use std::collections::HashMap;

use crate::{
    DecayChannelMatch, DecayStateExpansion, OwnedDecayChannelMatch, ParticleFilterHit,
    ParticleSearchQuery, ParticleSummary, Pdg, PdgError, PdgItemType, PdgParticle, PdgResult,
};

/// Which side of a recorded decay supplies literal state names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecayStateSide {
    /// Incoming states used by the decays-from filter.
    Initial,
    /// Outgoing products used by the decays-to and decay-contains filters.
    Final,
}

/// How a particle's name or alias matched a ranked search query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ParticleMatchKind {
    /// The normalized query equals the particle's normalized canonical name.
    ExactName,
    /// The normalized query equals a normalized PDG or common alias.
    ExactAlias,
    /// The query uses resonance shorthand, with optional parentheses, index, or charge.
    Resonance,
    /// The normalized query matches the beginning of a name or alias.
    Prefix,
    /// The normalized query appears within the canonical name.
    Substring,
    /// A name or alias matches after correcting a single letter edit or transposition.
    Typo,
}

impl ParticleMatchKind {
    pub(super) const fn is_direct(self) -> bool {
        matches!(self, Self::ExactName | Self::ExactAlias | Self::Resonance)
    }
}

/// A particle and the spelling that produced its best search match.
#[derive(Debug)]
pub struct ParticleSearchHit<'pdg> {
    /// The matching particle, with its canonical name and identifiers.
    pub particle: PdgParticle<'pdg>,
    /// The reason this particle matched.
    pub kind: ParticleMatchKind,
    /// The original spelling of the canonical name or alias that matched.
    pub matched_name: String,
    /// Matching PDG channels, empty for a search without decay filters.
    pub matching_decays: Vec<DecayChannelMatch<'pdg>>,
}

impl ParticleSearchHit<'_> {
    /// Detaches this hit from the database so it can be stored or serialized.
    #[must_use]
    pub fn into_owned(self) -> OwnedParticleSearchHit {
        OwnedParticleSearchHit {
            particle: ParticleSummary::from(&self.particle),
            kind: self.kind,
            matched_name: self.matched_name,
            matching_decays: self
                .matching_decays
                .iter()
                .map(OwnedDecayChannelMatch::from)
                .collect(),
        }
    }
}

/// A ranked particle search hit that can outlive the database connection.
#[derive(Clone, Debug, serde::Serialize)]
pub struct OwnedParticleSearchHit {
    /// Particle identity and quantum numbers.
    pub particle: ParticleSummary,
    /// The reason the name matched.
    pub kind: ParticleMatchKind,
    /// The original spelling of the name or alias that matched.
    pub matched_name: String,
    /// Matching PDG channels, empty for a search without decay filters.
    pub matching_decays: Vec<OwnedDecayChannelMatch>,
}

impl Pdg {
    /// Finds particles by name or alias, subject to the supplied filters.
    ///
    /// Matching accepts common Greek letters and subscript and superscript spellings.
    /// Exact names rank first, followed by exact aliases, resonance shorthand,
    /// prefixes, substrings, and typo matches. For example, `f0980` and `f980`
    /// match `f_0(980)0`. Typo matching requires at least four letters in the query and
    /// preserves digits, signs, and antiparticle markers. Ties prefer shorter
    /// canonical names, then the database's PDG ID and name ordering. A query
    /// that is empty after normalization returns no hits.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pdg_rs::{Charge, ParticleSearchQuery, Pdg};
    ///
    /// # fn main() -> pdg_rs::PdgResult<()> {
    /// let pdg = Pdg::open()?;
    /// let hits = pdg.find_particles("K_S^0", ParticleSearchQuery::new().charge(Charge::Neutral))?;
    /// assert_eq!(hits.first().map(|hit| hit.particle.name.as_str()), Some("K(S)0"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns an error if a property range is invalid or a database query fails.
    pub fn find_particles(
        &self,
        text: &str,
        filters: ParticleSearchQuery,
    ) -> PdgResult<Vec<ParticleSearchHit<'_>>> {
        let query = normalize_name(text);
        if query.is_empty() {
            filters.validate_ranges()?;
            return Ok(Vec::new());
        }
        let filtered = self.search_particle_hits(filters)?;
        if filtered.is_empty() {
            return Ok(Vec::new());
        }

        let aliases = self.particle_aliases()?;
        let mut hits = filtered
            .into_iter()
            .filter_map(|hit| {
                let ParticleFilterHit {
                    particle,
                    matching_decays,
                } = hit;
                let (kind, matched_name) = best_name_match(&query, &particle.name, &aliases)?;
                let matched_name = matched_name.to_owned();
                Some(ParticleSearchHit {
                    particle,
                    kind,
                    matched_name,
                    matching_decays,
                })
            })
            .collect::<Vec<_>>();
        hits.sort_by_key(|hit| (hit.kind, hit.particle.name.chars().count()));
        Ok(hits)
    }

    pub(super) fn resolve_decay_names(&self, states: Vec<String>) -> PdgResult<Vec<Vec<String>>> {
        let names = self.decay_item_names()?;
        let aliases = self.particle_aliases()?;
        states
            .into_iter()
            .map(|state| resolve_decay_name(state.trim(), &names, &aliases))
            .collect()
    }

    /// Ranks names to insert into one entry of a decay filter.
    ///
    /// Inclusive mode suggests named particles and groups using the particle matcher.
    /// Literal mode suggests exact recorded spellings for the requested side of the decay.
    /// Empty queries return no suggestions.
    ///
    /// # Errors
    /// Returns a database error if the decay names or aliases cannot be queried.
    pub fn suggest_decay_states(
        &self,
        text: &str,
        side: DecayStateSide,
        expansion: DecayStateExpansion,
        limit: usize,
    ) -> PdgResult<Vec<String>> {
        let text = text.trim();
        let query = normalize_name(text);
        if query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let (names, aliases) = match expansion {
            DecayStateExpansion::Inclusive => {
                let names = self
                    .decay_item_names()?
                    .into_iter()
                    .filter(|(_, kind)| {
                        matches!(
                            kind,
                            PdgItemType::Particle
                                | PdgItemType::Group
                                | PdgItemType::ChargeMultiplet
                        )
                    })
                    .map(|(name, _)| name)
                    .collect();
                (names, Some(self.particle_aliases()?))
            }
            DecayStateExpansion::Literal => {
                let mut stmt = self
                    .conn
                    .prepare_cached("SELECT DISTINCT name FROM pdgdecay WHERE is_outgoing = ?1")?;
                let names = stmt
                    .query_map([side == DecayStateSide::Final], |row| row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                (names, None)
            }
        };
        let mut matches = names
            .iter()
            .filter(|name| !name.contains(',') && name.trim() == *name)
            .filter_map(|name| {
                let kind = aliases.as_ref().map_or_else(
                    || match_kind(&query, &normalize_name(name), false),
                    |aliases| best_name_match(&query, name, aliases).map(|(kind, _)| kind),
                )?;
                Some((kind, name != text, name.chars().count(), name))
            })
            .collect::<Vec<_>>();
        matches.sort_unstable();
        matches.dedup();
        Ok(matches
            .into_iter()
            .take(limit)
            .map(|(_, _, _, name)| name.clone())
            .collect())
    }

    fn decay_item_names(&self) -> PdgResult<Vec<(String, PdgItemType)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT DISTINCT name, item_type FROM pdgitem WHERE item_type NOT IN ('A', 'S', 'W') ORDER BY name",
        )?;
        Ok(stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?)
    }

    fn particle_aliases(&self) -> PdgResult<HashMap<String, Vec<String>>> {
        // Follow alias/synonym/redirect targets through groups and multiplets.
        // A particle target is terminal. UNION also stops cycles in the map.
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE alias_targets(alias, target_id) AS (
                SELECT item.name, mapping.target_id
                FROM pdgitem item JOIN pdgitem_map mapping ON mapping.pdgitem_id = item.id
                WHERE item.item_type IN ('A', 'S', 'W')
                UNION
                SELECT alias_targets.alias, mapping.target_id
                FROM alias_targets
                JOIN pdgitem item ON item.id = alias_targets.target_id
                JOIN pdgitem_map mapping ON mapping.pdgitem_id = item.id
                WHERE item.item_type IN ('A', 'S', 'W', 'G', 'B')
            )
            SELECT item.name, alias_targets.alias
            FROM alias_targets JOIN pdgitem item ON item.id = alias_targets.target_id
            WHERE item.item_type = 'P'",
        )?;
        let mut aliases = HashMap::<String, Vec<String>>::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            aliases.entry(row.get(0)?).or_default().push(row.get(1)?);
        }
        Ok(aliases)
    }
}

fn resolve_decay_name(
    spelling: &str,
    names: &[(String, PdgItemType)],
    aliases: &HashMap<String, Vec<String>>,
) -> PdgResult<Vec<String>> {
    // Exact PDG decay labels include groups, multiplets, and descriptive final states.
    let query = normalize_name(spelling);
    if query.is_empty() || names.iter().any(|(name, _)| name == spelling) {
        return Ok(vec![spelling.to_owned()]);
    }
    let mut exact = names
        .iter()
        .filter(|(name, _)| normalize_name(name) == query)
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    exact.dedup();
    if !exact.is_empty() {
        return Ok(exact);
    }

    let mut matches = names
        .iter()
        .filter(|(_, item_type)| *item_type == PdgItemType::Particle)
        .filter_map(|(name, _)| {
            best_name_match(&query, name, aliases).map(|(kind, _)| (kind, name))
        })
        .collect::<Vec<_>>();
    matches.sort_unstable();
    let Some(&(kind, _)) = matches.first() else {
        return Ok(vec![spelling.to_owned()]);
    };
    if kind.is_direct() {
        matches.retain(|(candidate_kind, _)| *candidate_kind == kind);
    }
    let candidates = matches
        .into_iter()
        .map(|(_, name)| name.clone())
        .collect::<Vec<_>>();
    if kind == ParticleMatchKind::ExactAlias {
        return Ok(candidates);
    }
    if kind == ParticleMatchKind::Resonance
        && let Some((stem, _)) = candidates[0].rsplit_once(')')
        && let Some(group) = candidates[0].get(..=stem.len())
        && matches_resonance(&query, &normalize_name(group))
        && candidates.iter().all(|candidate| {
            candidate
                .rsplit_once(')')
                .is_some_and(|(prefix, _)| prefix == stem)
        })
        && names
            .iter()
            .any(|(name, item_type)| *item_type == PdgItemType::Group && name == group)
    {
        return Ok(vec![group.to_owned()]);
    }
    if candidates.len() == 1 {
        Ok(candidates)
    } else {
        Err(PdgError::AmbiguousDecayState {
            name: spelling.to_owned(),
            candidates,
        })
    }
}

fn best_name_match<'a>(
    query: &str,
    name: &'a str,
    aliases: &'a HashMap<String, Vec<String>>,
) -> Option<(ParticleMatchKind, &'a str)> {
    std::iter::once((name, false))
        .chain(aliases_for_name(aliases, name).map(|alias| (alias, true)))
        .filter_map(|(spelling, is_alias)| {
            match_kind(query, &normalize_name(spelling), is_alias).map(|kind| (kind, spelling))
        })
        .min_by_key(|(kind, spelling)| (*kind, spelling.len(), *spelling))
}

fn aliases_for_name<'a>(
    aliases: &'a HashMap<String, Vec<String>>,
    name: &str,
) -> impl Iterator<Item = &'a str> {
    aliases
        .get(name)
        .into_iter()
        .flatten()
        .map(String::as_str)
        .chain(common_aliases(name).iter().copied())
}

fn common_aliases(name: &str) -> &'static [&'static str] {
    match name {
        "p" => &["proton"],
        "pbar" => &["antiproton"],
        "gamma" => &["photon"],
        "e-" => &["electron"],
        "e+" => &["positron"],
        "mu-" => &["muon"],
        "mu+" => &["antimuon"],
        "n" => &["neutron"],
        "nbar" => &["antineutron"],
        "pi+" => &["pion", "positive pion"],
        "pi-" => &["pion", "negative pion"],
        "pi0" => &["pion", "neutral pion"],
        _ => &[],
    }
}

fn match_kind(query: &str, spelling: &str, is_alias: bool) -> Option<ParticleMatchKind> {
    use ParticleMatchKind::{ExactAlias, ExactName, Prefix, Resonance, Substring, Typo};

    if query == spelling {
        return Some(if is_alias { ExactAlias } else { ExactName });
    }
    // A trailing sign is an explicit charge. A prefix match must not turn + into ++.
    let charge = |name: &str| name.trim_end_matches(['+', '-']).len();
    let query_charge = &query[charge(query)..];
    if !query_charge.is_empty() && query_charge != &spelling[charge(spelling)..] {
        return None;
    }
    if matches_resonance(query, spelling) {
        return Some(Resonance);
    }
    if spelling.starts_with(query) && (!is_alias || query.chars().count() >= 2) {
        return Some(Prefix);
    }
    if !is_alias && spelling.contains(query) {
        return Some(Substring);
    }
    (query
        .bytes()
        .filter(u8::is_ascii_alphabetic)
        .take(4)
        .count()
        == 4
        && has_antiparticle_marker(query) == has_antiparticle_marker(spelling)
        && single_letter_typo(query, spelling))
    .then_some(Typo)
}

fn matches_resonance(query: &str, spelling: &str) -> bool {
    let Some((stem, mass_and_charge)) = spelling.rsplit_once('(') else {
        return false;
    };
    let Some((mass, charge)) = mass_and_charge.split_once(')') else {
        return false;
    };
    // Only a numeric resonance label can be compacted. Preserve an explicit charge
    // or antiparticle marker, and require the entire numeric label to match.
    if stem.is_empty()
        || mass.is_empty()
        || !mass.bytes().all(|byte| byte.is_ascii_digit())
        || !matches!(charge, "" | "0" | "+" | "-" | "++" | "--")
    {
        return false;
    }
    let family = stem.trim_end_matches(|ch: char| ch.is_ascii_digit());
    [stem, family].into_iter().any(|prefix| {
        if prefix.is_empty() {
            return false;
        }
        query.strip_prefix(prefix).is_some_and(|rest| {
            let suffix = rest.strip_prefix(mass).or_else(|| {
                rest.strip_prefix('(')?
                    .strip_prefix(mass)?
                    .strip_prefix(')')
            });
            suffix.is_some_and(|suffix| suffix.is_empty() || suffix == charge)
        })
    })
}

fn has_antiparticle_marker(name: &str) -> bool {
    name.starts_with("anti") || name.contains("bar")
}

fn single_letter_typo(query: &str, spelling: &str) -> bool {
    let is_separator = |ch: char| !ch.is_ascii_alphabetic();
    // Compare letter runs separately so an edit cannot change a non-letter or
    // move a letter across one, such as swapping the s and 2 in "b_s2".
    query.len().abs_diff(spelling.len()) <= 1
        && query
            .chars()
            .filter(|&ch| is_separator(ch))
            .eq(spelling.chars().filter(|&ch| is_separator(ch)))
        && query
            .split(is_separator)
            .zip(spelling.split(is_separator))
            .filter(|(query_run, spelling_run)| query_run != spelling_run)
            .map(|(query_run, spelling_run)| strsim::osa_distance(query_run, spelling_run))
            .sum::<usize>()
            == 1
}

fn normalize_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for ch in name.chars().flat_map(char::to_lowercase) {
        normalized.push_str(match ch {
            ch if ch.is_whitespace() => continue,
            '_' | '^' => continue,
            'α' => "alpha",
            'β' => "beta",
            'γ' => "gamma",
            'δ' => "delta",
            'ε' => "epsilon",
            'ζ' => "zeta",
            'η' => "eta",
            'θ' => "theta",
            'ι' => "iota",
            'κ' => "kappa",
            'λ' => "lambda",
            'μ' | 'µ' => "mu",
            'ν' => "nu",
            'ξ' => "xi",
            'ο' => "omicron",
            'π' => "pi",
            'ρ' => "rho",
            'σ' | 'ς' => "sigma",
            'τ' => "tau",
            'υ' => "upsilon",
            'φ' | 'ϕ' => "phi",
            'χ' => "chi",
            'ψ' => "psi",
            'ω' => "omega",
            '₀' | '⁰' => "0",
            '₁' | '¹' => "1",
            '₂' | '²' => "2",
            '₃' | '³' => "3",
            '₄' | '⁴' => "4",
            '₅' | '⁵' => "5",
            '₆' | '⁶' => "6",
            '₇' | '⁷' => "7",
            '₈' | '⁸' => "8",
            '₉' | '⁹' => "9",
            '₊' | '⁺' => "+",
            '₋' | '⁻' | '−' => "-",
            'ₑ' => "e",
            'ₗ' | 'ᴸ' => "l",
            'ₛ' | 'ˢ' => "s",
            '\u{0304}' | '\u{0305}' => "bar",
            _ => {
                normalized.push(ch);
                continue;
            }
        });
    }
    if let Some(rest) = normalized.strip_prefix("anti-") {
        return format!("anti{rest}");
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::single_letter_typo;

    #[test]
    fn typo_matching_keeps_non_letters_in_place() {
        assert!(single_letter_typo("alpha2btea", "alpha2beta"));
        assert!(!single_letter_typo("alph2abeta", "alpha2beta"));
        assert!(!single_letter_typo("alpha3beta", "alpha2beta"));
        assert!(!single_letter_typo("alhpa2btea", "alpha2beta"));
    }
}
