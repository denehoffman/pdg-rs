use rusqlite::{Row, params};

use crate::{
    Pdg, PdgId, PdgResult, PdgText, TextSearchCorrection, TextSearchParent, TextSearchResult,
    TextSearchSource,
};

const MAX_CORRECTIONS: usize = 3;

impl Pdg {
    /// Searches descriptions, text blocks, and footnotes with `SQLite` FTS5.
    ///
    /// Search terms match whole tokens by default. Append `*` before whitespace
    /// or the end of the query to match a term's prefix. If no results match, the
    /// search tries the final term as a prefix, then tries spelling corrections
    /// for one unrecognized word. The temporary index is built on demand.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pdg_rs::{Pdg, TextSearchSource};
    ///
    /// # fn main() -> pdg_rs::PdgResult<()> {
    /// let pdg = Pdg::open()?;
    /// let results = pdg.search_text("K(S)0 mean life")?;
    /// assert!(results.iter().any(|result| result.source == TextSearchSource::Description));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns a database error if the index cannot be initialized or the search fails.
    pub fn search_text(&self, query: impl Into<String>) -> PdgResult<Vec<TextSearchResult>> {
        self.search_text_with_limit(query, None)
    }

    /// Searches the same text sources as [`Pdg::search_text`] with a result limit.
    ///
    /// `None` returns all hits. A limit of zero returns an empty list. Any positive
    /// limit is applied after ranking, before the rows are read into Rust.
    ///
    /// # Errors
    /// Returns a database error if the index cannot be initialized or the search fails.
    pub fn search_text_with_limit(
        &self,
        query: impl Into<String>,
        limit: Option<usize>,
    ) -> PdgResult<Vec<TextSearchResult>> {
        if limit == Some(0) {
            return Ok(Vec::new());
        }
        let mut terms = text_terms(&query.into());
        let Some(tail) = terms.len().checked_sub(1) else {
            return Ok(Vec::new());
        };
        self.initialize_text_search()?;
        let limit = limit.map_or(-1, |limit| i64::try_from(limit).unwrap_or(i64::MAX));

        let results = self.query_text(&terms, limit)?;
        if !results.is_empty() {
            return Ok(results);
        }

        if !terms[tail].prefix
            && terms[tail].value.len() >= 3
            && terms[tail]
                .value
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic())
        {
            terms[tail].prefix = true;
            let results = self.query_text(&terms, limit)?;
            if !results.is_empty() {
                return Ok(results);
            }
            terms[tail].prefix = false;
        }

        if let Some((index, replacements)) = self.text_correction_candidates(&terms)? {
            let original = terms[index].value.clone();
            for replacement in replacements {
                terms[index].value.clone_from(&replacement);
                let mut results = self.query_text(&terms, limit)?;
                if !results.is_empty() {
                    let correction = TextSearchCorrection {
                        original,
                        replacement,
                    };
                    for hit in &mut results {
                        hit.correction = Some(correction.clone());
                    }
                    return Ok(results);
                }
            }
        }
        Ok(Vec::new())
    }

    fn query_text(&self, terms: &[TextTerm], limit: i64) -> PdgResult<Vec<TextSearchResult>> {
        let expression = terms
            .iter()
            .map(|term| format!("\"{}\"{}", term.value, if term.prefix { "*" } else { "" }))
            .collect::<Vec<_>>()
            .join(" ");
        let mut stmt = self.conn.prepare_cached(
            "WITH ranked AS (
                SELECT rowid, pdgid, source, text_type, sort, body,
                       snippet(pdg_text_search, 0, '[', ']', '...', 24) AS snippet,
                       bm25(pdg_text_search) AS score
                FROM temp.pdg_text_search
                WHERE pdg_text_search MATCH ?1
                ORDER BY score, source, sort, rowid
                LIMIT ?2
            )
            SELECT ranked.pdgid, ranked.source, ranked.text_type, ranked.sort,
                   ranked.body, ranked.snippet, ranked.score, entry.description,
                   parent.pdgid, parent.description
            FROM ranked
            LEFT JOIN main.pdgid entry ON entry.pdgid = ranked.pdgid
            LEFT JOIN main.pdgid parent ON parent.pdgid = entry.parent_pdgid
            ORDER BY ranked.score, ranked.source, ranked.sort, ranked.rowid",
        )?;
        Ok(stmt
            .query_map(params![expression, limit], text_hit)?
            .collect::<Result<_, _>>()?)
    }

    fn text_correction_candidates(
        &self,
        terms: &[TextTerm],
    ) -> PdgResult<Option<(usize, Vec<String>)>> {
        let mut distinguishable_words = terms
            .iter()
            .enumerate()
            .filter(|(_, term)| {
                !term.prefix
                    && (4..=32).contains(&term.value.len())
                    && term.value.bytes().all(|byte| byte.is_ascii_alphabetic())
            })
            .peekable();
        if distinguishable_words.peek().is_none() {
            return Ok(None);
        }
        // The vocabulary is a view over the existing index, created only on a failed search.
        self.conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS temp.pdg_text_vocabulary
             USING fts5vocab(pdg_text_search, 'row');",
        )?;
        let mut exists = self.conn.prepare_cached(
            "SELECT EXISTS(SELECT 1 FROM temp.pdg_text_vocabulary WHERE term = ?1)",
        )?;
        let mut unknown = None;
        for (index, term) in distinguishable_words {
            let word = term.value.to_ascii_lowercase();
            if !exists.query_row([&word], |row| row.get::<_, bool>(0))? {
                if unknown.is_some() {
                    return Ok(None);
                }
                unknown = Some((index, word));
            }
        }
        let Some((index, word)) = unknown else {
            return Ok(None);
        };
        let mut candidates = self.conn.prepare_cached(
            "SELECT term FROM temp.pdg_text_vocabulary
             WHERE length(term) BETWEEN length(?1) - 1 AND length(?1) + 1
               AND term NOT GLOB '*[^a-z]*'
             ORDER BY doc DESC, term",
        )?;
        let mut rows = candidates.query([&word])?;
        let mut corrections = Vec::new();
        while let Some(row) = rows.next()? {
            let term: String = row.get(0)?;
            if strsim::osa_distance(&word, &term) == 1 {
                corrections.push(term);
                if corrections.len() == MAX_CORRECTIONS {
                    break;
                }
            }
        }
        Ok(Some((index, corrections)))
    }

    fn initialize_text_search(&self) -> PdgResult<()> {
        // Check SQLite's schema so a caller rollback cannot leave a stale readiness flag.
        let initialized: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM temp.sqlite_schema WHERE name = 'pdg_text_search')",
            [],
            |row| row.get(0),
        )?;
        if initialized {
            return Ok(());
        }

        // The savepoint makes creation and population atomic, including inside a caller transaction.
        self.conn.execute_batch("SAVEPOINT pdg_rs_text_index")?;
        let result = self.conn.execute_batch(
            "CREATE VIRTUAL TABLE temp.pdg_text_search USING fts5(
                body,
                source UNINDEXED,
                pdgid UNINDEXED,
                text_type UNINDEXED,
                sort UNINDEXED,
                tokenize = 'unicode61'
            );
            INSERT INTO temp.pdg_text_search(body, source, pdgid, text_type, sort)
                SELECT description, 'description', pdgid, NULL, sort
                FROM main.pdgid
                WHERE description != '';
            INSERT INTO temp.pdg_text_search(body, source, pdgid, text_type, sort)
                SELECT text, 'text', pdgid, type, sort
                FROM main.pdgtext
                WHERE text IS NOT NULL AND text != '';
            INSERT INTO temp.pdg_text_search(body, source, pdgid, text_type, sort)
                SELECT text, 'footnote', pdgid, NULL, footnote_index
                FROM main.pdgfootnote
                WHERE text IS NOT NULL AND text != '';
            RELEASE pdg_rs_text_index;",
        );
        if result.is_err() {
            // SQLite may already have rolled back after a fatal error. Preserve the original error.
            let _ = self
                .conn
                .execute_batch("ROLLBACK TO pdg_rs_text_index; RELEASE pdg_rs_text_index;");
        }
        result?;
        Ok(())
    }
}

struct TextTerm {
    value: String,
    prefix: bool,
}

fn text_terms(query: &str) -> Vec<TextTerm> {
    let mut terms = Vec::new();
    let mut parts = query
        .split_inclusive(|ch: char| !ch.is_alphanumeric())
        .peekable();
    while let Some(part) = parts.next() {
        let value = part.trim_end_matches(|ch: char| !ch.is_alphanumeric());
        if !value.is_empty() {
            // An asterisk within a particle spelling like K*(892) is punctuation.
            let prefix = part.ends_with('*')
                && parts
                    .peek()
                    .is_none_or(|next| next.starts_with(char::is_whitespace));
            terms.push(TextTerm {
                value: value.to_owned(),
                prefix,
            });
        }
    }
    terms
}

fn text_hit(row: &Row<'_>) -> rusqlite::Result<TextSearchResult> {
    let pdgid = row.get::<_, PdgId>(0)?;
    let source = row.get::<_, String>(1)?;
    let text_type = row.get::<_, Option<String>>(2)?;
    let sort = row.get::<_, Option<isize>>(3)?.unwrap_or_default();
    let text = row.get::<_, String>(4)?;
    let (source, pdg_text) = match source.as_str() {
        "text" => {
            let text_type = text_type.unwrap_or_default();
            (
                TextSearchSource::Text {
                    text_type: text_type.clone(),
                    sort,
                },
                Some(PdgText {
                    pdgid: pdgid.clone(),
                    text_type,
                    text: Some(text.clone()),
                    sort,
                }),
            )
        }
        "footnote" => (TextSearchSource::Footnote { index: sort }, None),
        _ => (TextSearchSource::Description, None),
    };
    let parent = match row.get::<_, Option<PdgId>>(8)? {
        Some(pdgid) => Some(TextSearchParent {
            pdgid,
            title: row.get(9)?,
        }),
        None => None,
    };
    Ok(TextSearchResult {
        pdgid,
        source,
        text,
        pdg_text,
        snippet: row.get(5)?,
        score: row.get(6)?,
        title: row.get(7)?,
        parent,
        correction: None,
    })
}
