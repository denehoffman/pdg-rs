use rusqlite::{Connection, MAIN_DB, OptionalExtension, params_from_iter, types::Value};
use thiserror::Error;

mod models;
pub use models::*;

static PDG_BYTES: &[u8] = include_bytes!("../data/pdgall-2025-v0.2.2.sqlite");

pub const LATEST_EDITION: &'static str = "2025";

pub type PdgResult<T> = Result<T, PdgError>;

#[derive(Error, Debug)]
pub enum PdgError {
    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),
    #[error("Failed to parse ValueType: {0}")]
    ParseValueType(String),
    #[error("Failed to parse LimitType: {0}")]
    ParseLimitType(String),
    #[error("Failed to parse DataType: {0}")]
    ParseDataType(String),
    #[error(transparent)]
    QuantumNumberConversion(#[from] QuantumNumberConversionError),
    #[error("Custom error: {0}")]
    Custom(String),
}

#[derive(Debug)]
pub struct Pdg {
    conn: Connection,
}

impl Pdg {
    const PARTICLE_COLUMNS: &'static str = "pdgparticle.pdgid, name, pdgid.description, cc_type, pdgid.flags, mcid, charge, quantum_i, quantum_g, quantum_j, quantum_p, quantum_c";
    const PARTICLE_JOIN: &'static str =
        "JOIN pdgid ON pdgid.pdgid = pdgparticle.pdgid AND pdgid.data_type = 'PART'";

    pub fn open() -> PdgResult<Self> {
        let mut conn = Connection::open_in_memory()?;
        conn.deserialize_bytes(MAIN_DB, PDG_BYTES)?;
        let pdg = Self { conn };
        pdg.initialize_text_search()?;
        Ok(pdg)
    }

    pub fn db(&self) -> &Connection {
        &self.conn
    }

    fn initialize_text_search(&self) -> PdgResult<()> {
        self.conn.execute_batch(
            "CREATE VIRTUAL TABLE temp.pdg_text_search USING fts5(
                body,
                source UNINDEXED,
                pdgid UNINDEXED,
                text_type UNINDEXED,
                sort UNINDEXED,
                tokenize = 'unicode61'
            );
            INSERT INTO pdg_text_search(body, source, pdgid, text_type, sort)
                SELECT description, 'description', pdgid, NULL, sort
                FROM pdgid
                WHERE description != '';
            INSERT INTO pdg_text_search(body, source, pdgid, text_type, sort)
                SELECT text, 'text', pdgid, type, sort
                FROM pdgtext
                WHERE text IS NOT NULL AND text != '';",
        )?;
        Ok(())
    }

    pub fn particle(&self, name: impl Into<String>) -> PdgResult<Option<PdgParticle<'_>>> {
        let name = name.into();
        let sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE name = ?1",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_row([&name], |row| PdgParticle::from_row(self, row))
            .optional()?)
    }

    pub fn particle_by_pdg_id(
        &self,
        pdg_id: impl Into<String>,
    ) -> PdgResult<Option<PdgParticle<'_>>> {
        let pdg_id = pdg_id.into();
        let sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE upper(pdgparticle.pdgid) = upper(?1)",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_row([&pdg_id], |row| PdgParticle::from_row(self, row))
            .optional()?)
    }

    pub fn pdg_id(&self, pdg_id: impl Into<String>) -> PdgResult<Option<PdgIdEntry>> {
        let pdg_id = pdg_id.into();
        let mut stmt = self.conn.prepare(
            "SELECT id, pdgid, parent_pdgid, description, mode_number, data_type, flags, year_added, sort
            FROM pdgid
            WHERE upper(pdgid) = upper(?1)",
        )?;
        Ok(stmt
            .query_row([&pdg_id], |row| PdgIdEntry::try_from(row))
            .optional()?)
    }

    pub fn search_text(&self, query: impl Into<String>) -> PdgResult<Vec<TextSearchResult>> {
        let Some(query) = fts_query(query.into()) else {
            return Ok(Vec::new());
        };

        let mut stmt = self.conn.prepare(
            "SELECT
                pdgid,
                source,
                text_type,
                sort,
                body,
                snippet(pdg_text_search, 0, '[', ']', '...', 24),
                bm25(pdg_text_search)
            FROM pdg_text_search
            WHERE pdg_text_search MATCH ?1
            ORDER BY bm25(pdg_text_search), source, sort",
        )?;
        Ok(stmt
            .query_map([&query], |row| {
                let pdg_id = row.get::<_, PdgId>(0)?;
                let source = row.get::<_, String>(1)?;
                let text_type = row.get::<_, Option<String>>(2)?;
                let sort = row.get::<_, Option<isize>>(3)?;
                let text = row.get::<_, String>(4)?;
                let snippet = row.get::<_, String>(5)?;
                let score = row.get::<_, f64>(6)?;
                let (source, pdg_text) = if source == "text" {
                    let text_type = text_type.unwrap_or_default();
                    let sort = sort.unwrap_or_default();
                    (
                        TextSearchSource::Text {
                            text_type: text_type.clone(),
                            sort,
                        },
                        Some(PdgText {
                            pdg_id: pdg_id.clone(),
                            text_type,
                            text: Some(text.clone()),
                            sort,
                        }),
                    )
                } else {
                    (TextSearchSource::Description, None)
                };
                Ok(TextSearchResult {
                    pdg_id,
                    source,
                    text,
                    snippet,
                    score,
                    pdg_text,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn mcid(&self, mcid: isize) -> PdgResult<Option<PdgParticle<'_>>> {
        let sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE mcid = ?1",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_row([&mcid], |row| PdgParticle::from_row(self, row))
            .optional()?)
    }

    pub fn search_particles(&self, query: ParticleSearchQuery) -> PdgResult<Vec<PdgParticle<'_>>> {
        let mut sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE 1 = 1",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut params = Vec::new();
        let mass_range = query.mass_range_mev;
        let width_range = query.width_range_mev;
        let lifetime_range = query.lifetime_range_seconds;
        let decays_to = query.decays_to.clone();
        let decays_from = query.decays_from.clone();
        let decay_state_expansion = query.decay_state_expansion;

        if let Some(name_contains) = query.name_contains {
            sql.push_str(" AND name LIKE '%' || ? || '%'");
            params.push(Value::Text(name_contains));
        }

        if let Some(particle_class) = query.particle_class {
            sql.push_str(" AND pdgid.flags = ?");
            params.push(Value::Text(particle_class.flag().to_string()));
        }

        if let Some(particle_type) = query.particle_type {
            sql.push_str(" AND cc_type = ?");
            params.push(Value::Text(particle_type.code().to_string()));
        }

        if let Some(charge) = query.charge {
            sql.push_str(" AND ABS(charge - ?) < 1e-12");
            params.push(Value::Real(charge.as_f64()));
        }

        Self::push_quantum_filter(&mut sql, &mut params, "quantum_i", query.isospin);
        Self::push_quantum_filter(&mut sql, &mut params, "quantum_g", query.g_parity);
        Self::push_quantum_filter(&mut sql, &mut params, "quantum_j", query.angular_momentum);
        Self::push_quantum_filter(&mut sql, &mut params, "quantum_p", query.parity);
        Self::push_quantum_filter(&mut sql, &mut params, "quantum_c", query.charge_conjugation);

        self.push_decay_filters(
            &mut sql,
            &mut params,
            decays_to.states.clone(),
            true,
            decay_state_expansion,
        )?;
        self.push_decay_filters(
            &mut sql,
            &mut params,
            decays_from,
            false,
            decay_state_expansion,
        )?;

        sql.push_str(" ORDER BY pdgparticle.pdgid, name");
        let mut stmt = self.conn.prepare(&sql)?;
        let particles = stmt
            .query_map(params_from_iter(params), |row| {
                PdgParticle::from_row(self, row)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mass_entries = if mass_range.is_some() {
            Some(self.data_entries_by_parent(DataType::Mass)?)
        } else {
            None
        };
        let width_entries = if width_range.is_some() {
            Some(self.data_entries_by_parent(DataType::FullWidth)?)
        } else {
            None
        };
        let lifetime_entries = if lifetime_range.is_some() {
            Some(self.data_entries_by_parent(DataType::Lifetime)?)
        } else {
            None
        };

        let mut filtered_particles = Vec::new();
        for particle in particles {
            if !matches_data_range(
                mass_entries.as_ref(),
                &particle.pdg_id,
                mass_range,
                Unit::Mev,
            ) || !matches_data_range(
                width_entries.as_ref(),
                &particle.pdg_id,
                width_range,
                Unit::Mev,
            ) || !matches_data_range(
                lifetime_entries.as_ref(),
                &particle.pdg_id,
                lifetime_range,
                Unit::Seconds,
            ) {
                continue;
            }

            if decays_to.mode == DecayMatchMode::Exact
                && !decays_to.states.is_empty()
                && !self.particle_matches_exact_decay(
                    &particle.pdg_id,
                    &decays_to.states,
                    decay_state_expansion,
                )?
            {
                continue;
            }

            filtered_particles.push(particle);
        }

        Ok(filtered_particles)
    }

    pub fn item(&self, name: impl Into<String>) -> PdgResult<Option<PdgItem<'_>>> {
        let name = name.into();
        let mut stmt = self
            .conn
            .prepare("SELECT name, item_type FROM pdgitem WHERE name = ?1")?;
        Ok(stmt
            .query_row([&name], |row| PdgItem::from_row(self, row))
            .optional()?)
    }

    pub fn item_children(&self, name: impl Into<String>) -> PdgResult<Vec<PdgItemChild<'_>>> {
        let name = name.into();
        let child_items = {
            let mut stmt = self.conn.prepare(
                "SELECT child.name, child.item_type, pdgitem_map.sort FROM pdgitem_map JOIN pdgitem parent ON parent.id = pdgitem_map.pdgitem_id JOIN pdgitem child ON child.id = pdgitem_map.target_id WHERE parent.name = ?1 ORDER BY pdgitem_map.sort",
            )?;
            stmt.query_map([&name], |row| {
                Ok((PdgItem::from_row(self, row)?, row.get::<_, isize>(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
        };

        child_items
            .into_iter()
            .map(|(item, sort)| {
                let particle = match &item.item_type {
                    PdgItemType::Particle => self.particle(&item.name)?,
                    _ => None,
                };
                Ok(PdgItemChild {
                    item,
                    sort,
                    particle,
                })
            })
            .collect()
    }

    pub fn item_parents(&self, name: impl Into<String>) -> PdgResult<Vec<PdgItem<'_>>> {
        let name = name.into();
        let mut stmt = self.conn.prepare(
            "SELECT parent.name, parent.item_type FROM pdgitem_map JOIN pdgitem parent ON parent.id = pdgitem_map.pdgitem_id JOIN pdgitem child ON child.id = pdgitem_map.target_id WHERE child.name = ?1 ORDER BY parent.item_type, parent.name",
        )?;
        Ok(stmt
            .query_map([&name], |row| PdgItem::from_row(self, row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn children_for_pdg_id(&self, pdg_id: impl Into<String>) -> PdgResult<Vec<PdgIdEntry>> {
        let pdg_id = pdg_id.into();
        let mut stmt = self.conn.prepare(
            "SELECT id, pdgid, parent_pdgid, description, mode_number, data_type, flags, year_added, sort
            FROM pdgid
            WHERE upper(parent_pdgid) = upper(?1)
            ORDER BY sort, pdgid",
        )?;
        Ok(stmt
            .query_map([&pdg_id], |row| PdgIdEntry::try_from(row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn mapped_entries_for_pdg_id(
        &self,
        pdg_id: impl Into<String>,
    ) -> PdgResult<Vec<PdgIdEntry>> {
        let pdg_id = pdg_id.into();
        let mut stmt = self.conn.prepare(
            "SELECT target.id, target.pdgid, target.parent_pdgid, target.description, target.mode_number, target.data_type, target.flags, target.year_added, target.sort
            FROM pdgid_map
            JOIN pdgid target ON target.id = pdgid_map.target_id
            WHERE upper(pdgid_map.source) = upper(?1)
            ORDER BY pdgid_map.sort, target.pdgid",
        )?;
        Ok(stmt
            .query_map([&pdg_id], |row| PdgIdEntry::try_from(row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn data_for(&self, pdg_id: impl Into<String>) -> PdgResult<Vec<DataEntry<'_>>> {
        let pdg_id = pdg_id.into();
        let sql = format!(
            "SELECT {} FROM pdgdata WHERE upper(pdgid) = upper(?1) AND edition = ?2 ORDER BY sort",
            DataEntry::COLUMNS
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_map([&pdg_id, LATEST_EDITION], |row| {
                DataEntry::from_row(self, row)
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn texts_for(&self, pdg_id: impl Into<String>) -> PdgResult<Vec<PdgText>> {
        let pdg_id = pdg_id.into();
        let mut stmt = self.conn.prepare(
            "SELECT pdgid, type, text, sort FROM pdgtext WHERE pdgid = ?1 ORDER BY sort",
        )?;
        Ok(stmt
            .query_map([&pdg_id], |row| PdgText::try_from(row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn footnotes_for(&self, pdg_id: impl Into<String>) -> PdgResult<Vec<PdgFootnote>> {
        let pdg_id = pdg_id.into();
        let mut stmt = self.conn.prepare(
            "SELECT pdgid, footnote_index, text, changebar FROM pdgfootnote WHERE pdgid = ?1 ORDER BY footnote_index",
        )?;
        Ok(stmt
            .query_map([&pdg_id], |row| PdgFootnote::try_from(row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn measurements_for(&self, pdg_id: impl Into<String>) -> PdgResult<Vec<PdgMeasurement>> {
        let pdg_id = pdg_id.into();
        let mut stmt = self.conn.prepare(
            "SELECT pdgmeasurement.id, pdgmeasurement.pdgid, event_count, confidence_level, place, technique, charge, changebar, comment, sort, document_id, publication_name, publication_year, doi, inspire_id, title FROM pdgmeasurement JOIN pdgreference ON pdgreference.id = pdgmeasurement.pdgreference_id WHERE pdgmeasurement.pdgid = ?1 ORDER BY sort",
        )?;
        let mut measurements = stmt
            .query_map([&pdg_id], |row| PdgMeasurement::try_from(row))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut value_stmt = self.conn.prepare(
            "SELECT column_name, value_text, unit_text, display_value_text, display_power_of_ten, display_in_percent, limit_type, used_in_average, used_in_fit, value, error_positive, error_negative, stat_error_positive, stat_error_negative, syst_error_positive, syst_error_negative, sort FROM pdgmeasurement_values WHERE pdgmeasurement_id = ?1 ORDER BY sort",
        )?;
        let mut footnote_stmt = self.conn.prepare(
            "SELECT pdgfootnote.pdgid, footnote_index, text, changebar FROM pdgmeasurement_footnote JOIN pdgfootnote ON pdgfootnote.id = pdgmeasurement_footnote.pdgfootnote_id WHERE pdgmeasurement_id = ?1 ORDER BY footnote_index",
        )?;
        for measurement in &mut measurements {
            measurement.values = value_stmt
                .query_map([measurement.id], |row| PdgMeasurementValue::try_from(row))?
                .collect::<Result<Vec<_>, _>>()?;
            measurement.footnotes = footnote_stmt
                .query_map([measurement.id], |row| PdgFootnote::try_from(row))?
                .collect::<Result<Vec<_>, _>>()?;
        }

        Ok(measurements)
    }

    fn push_decay_filters(
        &self,
        sql: &mut String,
        params: &mut Vec<Value>,
        states: Vec<String>,
        is_outgoing: bool,
        expansion: DecayStateExpansion,
    ) -> PdgResult<()> {
        if states.is_empty() {
            return Ok(());
        }

        sql.push_str(
            " AND pdgparticle.pdgid IN (
                SELECT decay_pdgid.parent_pdgid
                FROM pdgid decay_pdgid
                WHERE decay_pdgid.data_type IN ('BFX', 'BFX1', 'BFX2', 'BFX3', 'BFX4', 'BFX5', 'BFI', 'BFI1', 'BFI2', 'BFI3', 'BFI4', 'BFI5')",
        );

        for state in states {
            let names = self.expand_decay_state_names(state, expansion)?;
            let placeholders = std::iter::repeat_n("?", names.len())
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(
                " AND EXISTS (
                    SELECT 1
                    FROM pdgdecay
                    WHERE pdgdecay.pdgid = decay_pdgid.pdgid
                        AND pdgdecay.is_outgoing = ?
                        AND pdgdecay.name IN ({placeholders})
                )"
            ));
            params.push(Value::Integer(if is_outgoing { 1 } else { 0 }));
            params.extend(names.into_iter().map(Value::Text));
        }

        sql.push(')');
        Ok(())
    }

    fn push_quantum_filter<T: ToString>(
        sql: &mut String,
        params: &mut Vec<Value>,
        column: &str,
        filter: QuantumFilter<T>,
    ) {
        match filter {
            QuantumFilter::Any => {}
            QuantumFilter::Missing => {
                sql.push_str(&format!(" AND {column} IS NULL"));
            }
            QuantumFilter::Value(value) => {
                sql.push_str(&format!(" AND {column} = ?"));
                params.push(Value::Text(value.to_string()));
            }
        }
    }

    fn particle_matches_exact_decay(
        &self,
        pdg_id: &str,
        states: &[String],
        expansion: DecayStateExpansion,
    ) -> PdgResult<bool> {
        let requested = states
            .iter()
            .map(|state| self.expand_decay_state_names(state.clone(), expansion))
            .collect::<PdgResult<Vec<_>>>()?;
        let mut stmt = self.conn.prepare(
            "SELECT decay_pdgid.pdgid, pdgdecay.name, pdgdecay.multiplier
            FROM pdgid decay_pdgid
            JOIN pdgdecay ON pdgdecay.pdgid = decay_pdgid.pdgid
            WHERE decay_pdgid.parent_pdgid = ?1
                AND decay_pdgid.data_type IN ('BFX', 'BFX1', 'BFX2', 'BFX3', 'BFX4', 'BFX5', 'BFI', 'BFI1', 'BFI2', 'BFI3', 'BFI4', 'BFI5')
                AND pdgdecay.is_outgoing = 1
            ORDER BY decay_pdgid.sort ASC, pdgdecay.sort ASC",
        )?;
        let rows = stmt
            .query_map([pdg_id], |row| {
                Ok((
                    row.get::<_, PdgId>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? as usize,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut modes = std::collections::HashMap::<PdgId, Vec<String>>::new();
        for (mode_pdg_id, name, multiplier) in rows {
            let products = modes.entry(mode_pdg_id).or_default();
            for _ in 0..multiplier {
                products.push(name.clone());
            }
        }

        Ok(modes
            .values()
            .any(|products| exact_decay_products_match(&requested, products)))
    }

    fn expand_decay_state_names(
        &self,
        name: String,
        expansion: DecayStateExpansion,
    ) -> PdgResult<Vec<String>> {
        if expansion == DecayStateExpansion::Literal {
            return Ok(vec![name]);
        }

        let mut names = vec![name.clone()];
        let mut seen = std::collections::HashSet::from([name.clone()]);
        let mut parents = Vec::new();

        let mut stmt = self.conn.prepare(
            "SELECT child.name, 0 AS is_parent, pdgitem_map.sort
            FROM pdgitem_map
            JOIN pdgitem parent ON parent.id = pdgitem_map.pdgitem_id
            JOIN pdgitem child ON child.id = pdgitem_map.target_id
            WHERE parent.name = ?1
            UNION ALL
            SELECT parent.name, 1 AS is_parent, pdgitem_map.sort
            FROM pdgitem_map
            JOIN pdgitem parent ON parent.id = pdgitem_map.pdgitem_id
            JOIN pdgitem child ON child.id = pdgitem_map.target_id
            WHERE child.name = ?1
            ORDER BY is_parent, sort",
        )?;
        for (relative, is_parent) in stmt
            .query_map([&name], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
        {
            if is_parent {
                parents.push(relative.clone());
            }
            if seen.insert(relative.clone()) {
                names.push(relative);
            }
        }

        if self.is_antiparticle_item(&name)? || self.is_neutral_meson_particle(&name)? {
            for parent in parents {
                let alias = format!("{parent}bar");
                if self.decay_state_exists(&alias)? && seen.insert(alias.clone()) {
                    names.push(alias);
                }
            }
        }

        Ok(names)
    }

    fn is_antiparticle_item(&self, name: &str) -> PdgResult<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM pdgparticle WHERE name = ?1 AND cc_type = 'A'",
                [name],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    fn is_neutral_meson_particle(&self, name: &str) -> PdgResult<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1
                FROM pdgparticle
                JOIN pdgid ON pdgid.pdgid = pdgparticle.pdgid AND pdgid.data_type = 'PART'
                WHERE pdgparticle.name = ?1
                    AND ABS(pdgparticle.charge) < 1e-12
                    AND pdgid.flags = 'M'",
                [name],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    fn decay_state_exists(&self, name: &str) -> PdgResult<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1
                WHERE EXISTS (SELECT 1 FROM pdgitem WHERE name = ?1)
                    OR EXISTS (SELECT 1 FROM pdgdecay WHERE name = ?1)",
                [name],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    fn data_entries_by_parent(
        &self,
        data_type: DataType,
    ) -> PdgResult<std::collections::HashMap<PdgId, Vec<DataEntry<'_>>>> {
        let data_type = data_type.to_string();
        let sql = format!(
            "SELECT {}, pdgid.parent_pdgid FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgdata.edition = ?2",
            DataEntry::COLUMNS
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map([&data_type, LATEST_EDITION], |row| {
                Ok((
                    row.get::<_, PdgId>(DataEntry::COLUMN_COUNT)?,
                    DataEntry::from_row(self, row)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut grouped =
            std::collections::HashMap::<PdgId, (Vec<DataEntry<'_>>, Vec<DataEntry<'_>>)>::new();
        for (parent_pdgid, entry) in rows {
            let (all_entries, summary_entries) = grouped.entry(parent_pdgid).or_default();
            all_entries.push(entry.clone());
            if entry.in_summary_table {
                summary_entries.push(entry);
            }
        }

        Ok(grouped
            .into_iter()
            .map(|(pdg_id, (all_entries, summary_entries))| {
                let entries = if summary_entries.is_empty() {
                    all_entries
                } else {
                    summary_entries
                };
                (pdg_id, entries)
            })
            .collect())
    }
}

#[derive(Copy, Clone)]
enum Unit {
    Mev,
    Seconds,
}

#[derive(Copy, Clone)]
struct Interval {
    min: f64,
    max: f64,
}

impl Interval {
    fn overlaps(self, min: f64, max: f64) -> bool {
        self.min <= max && self.max >= min
    }
}

fn matches_data_range(
    entries_by_parent: Option<&std::collections::HashMap<PdgId, Vec<DataEntry<'_>>>>,
    pdg_id: &str,
    range: Option<(f64, f64)>,
    unit: Unit,
) -> bool {
    let Some((min, max)) = range else {
        return true;
    };
    let Some(entries) = entries_by_parent.and_then(|entries| entries.get(pdg_id)) else {
        return true;
    };
    if entries.is_empty() {
        return true;
    }

    entries
        .iter()
        .any(|entry| match data_interval(entry, unit) {
            Some(interval) => interval.overlaps(min, max),
            None => true,
        })
}

fn exact_decay_products_match(requested: &[Vec<String>], products: &[String]) -> bool {
    if requested.len() != products.len() {
        return false;
    }

    let mut used = vec![false; products.len()];
    exact_decay_products_match_from(requested, products, &mut used, 0)
}

fn exact_decay_products_match_from(
    requested: &[Vec<String>],
    products: &[String],
    used: &mut [bool],
    index: usize,
) -> bool {
    if index == requested.len() {
        return true;
    }

    for (product_index, product) in products.iter().enumerate() {
        if used[product_index] || !requested[index].contains(product) {
            continue;
        }

        used[product_index] = true;
        if exact_decay_products_match_from(requested, products, used, index + 1) {
            return true;
        }
        used[product_index] = false;
    }

    false
}

fn data_interval(entry: &DataEntry, unit: Unit) -> Option<Interval> {
    let factor = unit_factor(&entry.unit_text, unit)?;

    if entry.limit_type == Some(LimitType::Range) {
        return parse_interval(entry).map(|interval| Interval {
            min: interval.min * factor,
            max: interval.max * factor,
        });
    }

    let value = entry.value?;
    let value = value * factor;
    match entry.limit_type {
        Some(LimitType::UpperLimit) => Some(Interval {
            min: f64::NEG_INFINITY,
            max: value,
        }),
        Some(LimitType::LowerLimit) => Some(Interval {
            min: value,
            max: f64::INFINITY,
        }),
        Some(LimitType::RangeExclusion) => None,
        Some(LimitType::Range) => unreachable!(),
        None => {
            let error_positive = entry.error_positive.unwrap_or(0.0) * factor;
            let error_negative = entry.error_negative.unwrap_or(0.0) * factor;
            Some(Interval {
                min: value - error_negative,
                max: value + error_positive,
            })
        }
    }
}

fn parse_interval(entry: &DataEntry) -> Option<Interval> {
    let text = entry
        .value_text
        .as_deref()
        .unwrap_or(entry.display_value_text.as_str());
    let values = parse_numbers(text);
    let min = values.iter().copied().reduce(f64::min)?;
    let max = values.iter().copied().reduce(f64::max)?;
    Some(Interval { min, max })
}

fn parse_numbers(text: &str) -> Vec<f64> {
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut numbers = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let (start, ch) = chars[index];
        let next = chars.get(index + 1).map(|(_, ch)| *ch);
        let starts_number = ch.is_ascii_digit()
            || (ch == '.' && next.is_some_and(|ch| ch.is_ascii_digit()))
            || ((ch == '+' || ch == '-')
                && next.is_some_and(|ch| ch.is_ascii_digit() || ch == '.'));
        if !starts_number {
            index += 1;
            continue;
        }

        let mut end_index = index + 1;
        let mut previous = ch;
        while end_index < chars.len() {
            let (_, current) = chars[end_index];
            if current.is_ascii_digit()
                || current == '.'
                || current == 'e'
                || current == 'E'
                || ((current == '+' || current == '-') && (previous == 'e' || previous == 'E'))
            {
                previous = current;
                end_index += 1;
            } else {
                break;
            }
        }

        let end = chars
            .get(end_index)
            .map(|(char_index, _)| *char_index)
            .unwrap_or(text.len());
        if let Ok(value) = text[start..end].parse::<f64>() {
            numbers.push(value);
        }
        index = end_index;
    }
    numbers
}

fn unit_factor(unit_text: &str, unit: Unit) -> Option<f64> {
    match unit {
        Unit::Mev => match unit_text {
            "MeV" => Some(1.0),
            "GeV" => Some(1000.0),
            "keV" => Some(0.001),
            "eV" => Some(0.000001),
            "u" => Some(931.49410242),
            _ => None,
        },
        Unit::Seconds => match unit_text {
            "s" => Some(1.0),
            "yr" | "years" => Some(31_557_600.0),
            _ => None,
        },
    }
}

fn fts_query(query: String) -> Option<String> {
    let mut terms = Vec::new();
    let mut term = String::new();
    for ch in query.chars() {
        if ch.is_alphanumeric() {
            term.push(ch);
        } else if !term.is_empty() {
            terms.push(std::mem::take(&mut term));
        }
    }
    if !term.is_empty() {
        terms.push(term);
    }

    (!terms.is_empty()).then(|| {
        terms
            .into_iter()
            .map(|term| format!("\"{term}\""))
            .collect::<Vec<_>>()
            .join(" ")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charged_decay_states_do_not_expand_to_antiparticle_siblings() {
        let db = Pdg::open().unwrap();
        let names = db
            .expand_decay_state_names("pi+".to_string(), DecayStateExpansion::Inclusive)
            .unwrap();

        assert!(names.contains(&"pi+".to_string()));
        assert!(names.contains(&"pi".to_string()));
        assert!(!names.contains(&"pi-".to_string()));
    }

    #[test]
    fn text_search_finds_pdgid_descriptions() {
        let db = Pdg::open().unwrap();
        let results = db.search_text("K(S)0 MEAN LIFE").unwrap();

        let result = results
            .iter()
            .find(|result| {
                result.pdg_id == "S012205" && result.source == TextSearchSource::Description
            })
            .unwrap();

        assert!(result.text.contains("K(S)0 MEAN LIFE"));
        assert!(!result.snippet.is_empty());
        assert!(result.pdg_text.is_none());
    }

    #[test]
    fn text_search_finds_pdgtext_rows() {
        let db = Pdg::open().unwrap();
        let results = db
            .search_text("Measurements Kbar0 divided convert")
            .unwrap();

        let result = results
            .iter()
            .find(|result| matches!(result.source, TextSearchSource::Text { .. }))
            .unwrap();

        assert!(result.text.contains("Measurements given as a Kbar0 ratio"));
        assert!(!result.snippet.is_empty());
        assert_eq!(
            result.pdg_text.as_ref().unwrap().text.as_deref(),
            Some(result.text.as_str())
        );
    }

    #[test]
    fn text_search_handles_punctuation_heavy_queries() {
        let db = Pdg::open().unwrap();
        let results = db.search_text("K(S)0").unwrap();

        assert!(!results.is_empty());
        assert!(results.iter().any(|result| result.text.contains("K(S)0")));
    }

    #[test]
    fn text_search_orders_by_score() {
        let db = Pdg::open().unwrap();
        let results = db.search_text("form factors").unwrap();

        assert!(results.len() > 1);
        assert!(
            results
                .windows(2)
                .all(|window| window[0].score <= window[1].score)
        );
    }

    #[test]
    fn text_search_returns_empty_results_for_empty_or_missing_queries() {
        let db = Pdg::open().unwrap();

        assert!(db.search_text(".,()").unwrap().is_empty());
        assert!(db.search_text("zzzzzznotapdgterm").unwrap().is_empty());
    }
}
