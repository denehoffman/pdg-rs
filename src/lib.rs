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
        Ok(Self { conn })
    }

    pub fn db(&self) -> &Connection {
        &self.conn
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

    pub fn search(&self, name: impl Into<String>) -> PdgResult<Vec<PdgParticle<'_>>> {
        let name = name.into();
        let sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE name LIKE '%' || ?1 || '%'",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_map([&name], |row| PdgParticle::from_row(self, row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn pdgid(&self, pdgid: isize) -> PdgResult<Option<PdgParticle<'_>>> {
        let sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE mcid = ?1",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_row([&pdgid], |row| PdgParticle::from_row(self, row))
            .optional()?)
    }

    pub fn particles_by_class(
        &self,
        particle_class: ParticleClass,
    ) -> PdgResult<Vec<PdgParticle<'_>>> {
        let sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE pdgid.flags = ?1 ORDER BY pdgparticle.pdgid, name",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_map([particle_class.flag()], |row| {
                PdgParticle::from_row(self, row)
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn search_by_class(
        &self,
        name: impl Into<String>,
        particle_class: ParticleClass,
    ) -> PdgResult<Vec<PdgParticle<'_>>> {
        let name = name.into();
        let sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE name LIKE '%' || ?1 || '%' AND pdgid.flags = ?2 ORDER BY pdgparticle.pdgid, name",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_map([&name, particle_class.flag()], |row| {
                PdgParticle::from_row(self, row)
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn search_particles(&self, query: ParticleSearch) -> PdgResult<Vec<PdgParticle<'_>>> {
        let mut sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE 1 = 1",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut params = Vec::new();

        if let Some(name_contains) = query.name_contains {
            sql.push_str(" AND name LIKE '%' || ? || '%'");
            params.push(Value::Text(name_contains));
        }

        if let Some(particle_class) = query.particle_class {
            sql.push_str(" AND pdgid.flags = ?");
            params.push(Value::Text(particle_class.flag().to_string()));
        }

        if let Some(angular_momentum) = query.angular_momentum {
            sql.push_str(" AND quantum_j = ?");
            params.push(Value::Text(angular_momentum.to_string()));
        }

        if let Some((min, max)) = query.mass_range_mev {
            sql.push_str(
                " AND pdgparticle.pdgid IN (
                    SELECT mass_pdgid.parent_pdgid
                    FROM pdgid mass_pdgid
                    JOIN pdgdata mass_data ON mass_data.pdgid_id = mass_pdgid.id
                    WHERE mass_pdgid.data_type = 'M'
                        AND mass_data.edition = ?
                        AND mass_data.value IS NOT NULL
                        AND CASE mass_data.unit_text
                            WHEN 'MeV' THEN mass_data.value
                            WHEN 'GeV' THEN mass_data.value * 1000.0
                            WHEN 'eV' THEN mass_data.value / 1000000.0
                            WHEN 'u' THEN mass_data.value * 931.49410242
                        END BETWEEN ? AND ?
                        AND (
                            mass_data.in_summary_table = 1
                            OR NOT EXISTS (
                                SELECT 1
                                FROM pdgid summary_pdgid
                                JOIN pdgdata summary_data ON summary_data.pdgid_id = summary_pdgid.id
                                WHERE summary_pdgid.parent_pdgid = mass_pdgid.parent_pdgid
                                    AND summary_pdgid.data_type = 'M'
                                    AND summary_data.edition = ?
                                    AND summary_data.value IS NOT NULL
                                    AND summary_data.in_summary_table = 1
                            )
                        )
                )",
            );
            params.push(Value::Text(LATEST_EDITION.to_string()));
            params.push(Value::Real(min));
            params.push(Value::Real(max));
            params.push(Value::Text(LATEST_EDITION.to_string()));
        }

        self.push_decay_filters(&mut sql, &mut params, query.decays_to, true)?;
        self.push_decay_filters(&mut sql, &mut params, query.decays_from, false)?;

        sql.push_str(" ORDER BY pdgparticle.pdgid, name");
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_map(params_from_iter(params), |row| {
                PdgParticle::from_row(self, row)
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn item(&self, name: impl Into<String>) -> PdgResult<Option<PdgItem>> {
        let name = name.into();
        let mut stmt = self
            .conn
            .prepare("SELECT name, item_type FROM pdgitem WHERE name = ?1")?;
        Ok(stmt
            .query_row([&name], |row| PdgItem::try_from(row))
            .optional()?)
    }

    pub fn item_children(&self, name: impl Into<String>) -> PdgResult<Vec<PdgItemChild<'_>>> {
        let name = name.into();
        let child_items = {
            let mut stmt = self.conn.prepare(
                "SELECT child.name, child.item_type, pdgitem_map.sort FROM pdgitem_map JOIN pdgitem parent ON parent.id = pdgitem_map.pdgitem_id JOIN pdgitem child ON child.id = pdgitem_map.target_id WHERE parent.name = ?1 ORDER BY pdgitem_map.sort",
            )?;
            stmt.query_map([&name], |row| {
                Ok((PdgItem::try_from(row)?, row.get::<_, isize>(2)?))
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

    pub fn item_parents(&self, name: impl Into<String>) -> PdgResult<Vec<PdgItem>> {
        let name = name.into();
        let mut stmt = self.conn.prepare(
            "SELECT parent.name, parent.item_type FROM pdgitem_map JOIN pdgitem parent ON parent.id = pdgitem_map.pdgitem_id JOIN pdgitem child ON child.id = pdgitem_map.target_id WHERE child.name = ?1 ORDER BY parent.item_type, parent.name",
        )?;
        Ok(stmt
            .query_map([&name], |row| PdgItem::try_from(row))?
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
            let names = self.expand_decay_state_names(state)?;
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

    fn expand_decay_state_names(&self, name: String) -> PdgResult<Vec<String>> {
        let mut names = vec![name.clone()];
        let mut seen = std::collections::HashSet::from([name.clone()]);
        let mut stmt = self.conn.prepare(
            "SELECT child.name FROM pdgitem_map JOIN pdgitem parent ON parent.id = pdgitem_map.pdgitem_id JOIN pdgitem child ON child.id = pdgitem_map.target_id WHERE parent.name = ?1 ORDER BY pdgitem_map.sort",
        )?;
        for child in stmt
            .query_map([&name], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
        {
            if seen.insert(child.clone()) {
                names.push(child);
            }
        }
        Ok(names)
    }
}
