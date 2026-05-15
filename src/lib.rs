#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![allow(clippy::empty_docs)]
#![doc = ""]
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};
use thiserror::Error;

mod database;
mod models;
pub use models::*;

/// The default PDG edition used by this crate.
pub const LATEST_EDITION: &str = "2025";

/// Result type returned by fallible `pdg-rs` operations.
pub type PdgResult<T> = Result<T, PdgError>;

/// Errors returned by database access, code parsing, and quantum number conversion.
#[derive(Error, Debug)]
pub enum PdgError {
    /// A `SQLite` error from the PDG database.
    #[error(transparent)]
    SqliteError(#[from] rusqlite::Error),
    /// An I/O error occurred while reading, writing, or caching the PDG database.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// No OS-specific cache directory could be found.
    #[error("PDG database cache directory is unavailable")]
    CacheDirectoryUnavailable,
    /// The PDG database is not cached and network access is disabled.
    #[error("PDG database is not cached at {0:?} and downloads are disabled")]
    OfflineDatabaseMissing(PathBuf),
    /// Downloading the PDG database failed.
    #[error("failed to download PDG database: {0}")]
    Download(String),
    /// A cached or downloaded PDG database has the wrong byte length.
    #[error(
        "PDG database size mismatch for {path:?}: expected {expected} bytes, got {actual} bytes"
    )]
    DatabaseSizeMismatch {
        /// Path to the database file that was checked.
        path: PathBuf,
        /// Expected byte length.
        expected: u64,
        /// Actual byte length.
        actual: u64,
    },
    /// A cached or downloaded PDG database has the wrong SHA-256 digest.
    #[error("PDG database checksum mismatch for {path:?}: expected {expected}, got {actual}")]
    DatabaseChecksumMismatch {
        /// Path to the database file that was checked.
        path: PathBuf,
        /// Expected SHA-256 digest.
        expected: &'static str,
        /// Actual SHA-256 digest.
        actual: String,
    },
    /// A value type code was not recognized.
    #[error("Failed to parse ValueType: {0}")]
    ParseValueType(String),
    /// A limit type code was not recognized.
    #[error("Failed to parse LimitType: {0}")]
    ParseLimitType(String),
    /// A data type code was not recognized.
    #[error("Failed to parse DataType: {0}")]
    ParseDataType(String),
    /// A quantum number could not be converted into the requested numeric representation.
    #[error(transparent)]
    QuantumNumberConversion(#[from] QuantumNumberConversionError),
    /// An application-specific error message.
    #[error("Custom error: {0}")]
    Custom(String),
}

/// Handle for querying the Particle Data Group database.
///
/// Create a handle with [`Pdg::open`], then use lookup methods such as
/// [`Pdg::particle`], [`Pdg::mcid`], [`Pdg::search_particles`], and
/// [`Pdg::search_text`] to retrieve typed records.
///
/// # Examples
///
/// ```no_run
/// use pdg_rs::Pdg;
///
/// # fn main() -> pdg_rs::PdgResult<()> {
/// let pdg = Pdg::open()?;
/// let pion = pdg.particle("pi+")?.expect("pi+ is in the PDG database");
///
/// assert_eq!(pion.name, "pi+");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Pdg {
    conn: Connection,
}

impl Pdg {
    const PARTICLE_COLUMNS: &'static str = "pdgparticle.pdgid, name, pdgid.description, cc_type, pdgid.flags, mcid, charge, quantum_i, quantum_g, quantum_j, quantum_p, quantum_c";
    const PARTICLE_JOIN: &'static str =
        "JOIN pdgid ON pdgid.pdgid = pdgparticle.pdgid AND pdgid.data_type = 'PART'";

    /// Opens the default PDG `SQLite` database.
    ///
    /// If `PDG_RS_DB_PATH` is set, that exact database file is opened. Otherwise,
    /// the default database is loaded from the local cache, downloading and
    /// verifying it first when needed.
    ///
    /// This also initializes the temporary full-text search index used by
    /// [`Pdg::search_text`].
    ///
    /// # Errors
    ///
    /// Returns an error if the configured database cannot be opened, the cached
    /// database cannot be verified, or the default database cannot be
    /// downloaded.
    pub fn open() -> PdgResult<Self> {
        Self::open_path(database::ensure_database()?)
    }

    /// Opens the default PDG `SQLite` database without downloading it.
    ///
    /// If `PDG_RS_DB_PATH` is set, that exact database file is opened. Otherwise,
    /// this opens the verified cached copy of the default database.
    ///
    /// # Errors
    ///
    /// Returns [`PdgError::OfflineDatabaseMissing`] if the default database is
    /// not cached. Returns another error if the configured database cannot be
    /// opened or the cached database cannot be verified.
    pub fn open_cached() -> PdgResult<Self> {
        Self::open_path(database::cached_database()?)
    }

    /// Opens a PDG `SQLite` database at `path`.
    ///
    /// This is useful for applications that manage their own database file or
    /// want to use a different PDG edition. Use [`Pdg::open`] for the default
    /// cache-or-download behavior.
    ///
    /// # Errors
    ///
    /// Returns a database error if `path` cannot be opened or initialized.
    pub fn open_path(path: impl AsRef<Path>) -> PdgResult<Self> {
        let conn = Connection::open(path)?;
        let pdg = Self { conn };
        pdg.initialize_text_search()?;
        Ok(pdg)
    }

    /// Ensures the default database exists in the local cache and returns its path.
    ///
    /// If `PDG_RS_DB_PATH` is set, this returns that path without downloading or
    /// validating it. Otherwise, this downloads the default database when it is
    /// missing or invalid, unless `PDG_RS_OFFLINE` disables network access.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache directory is unavailable, the database
    /// cannot be downloaded, or the downloaded file fails verification.
    pub fn ensure_database() -> PdgResult<PathBuf> {
        database::ensure_database()
    }

    /// Returns the cache path for the default database.
    ///
    /// This does not check whether the database exists and does not consider
    /// `PDG_RS_DB_PATH`, which is an explicit override rather than a cache path.
    ///
    /// # Errors
    ///
    /// Returns [`PdgError::CacheDirectoryUnavailable`] if no cache directory can
    /// be found.
    pub fn cached_database_path() -> PdgResult<PathBuf> {
        database::cached_database_path()
    }

    /// Returns the underlying `SQLite` connection.
    ///
    /// This is useful for advanced queries that are not covered by the typed
    /// API. Prefer the typed methods where possible because they preserve links
    /// back to this [`Pdg`] handle.
    #[must_use]
    pub const fn db(&self) -> &Connection {
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
                WHERE text IS NOT NULL AND text != '';
            INSERT INTO pdg_text_search(body, source, pdgid, text_type, sort)
                SELECT text, 'footnote', pdgid, NULL, footnote_index
                FROM pdgfootnote
                WHERE text IS NOT NULL AND text != '';",
        )?;
        Ok(())
    }

    /// Looks up a particle by its PDG item name.
    ///
    /// Use [`Pdg::particle_by_pdgid`] when you already have a PDG identifier,
    /// or [`Pdg::mcid`] when you have a Monte Carlo particle ID.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
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

    /// Looks up a particle by PDG identifier, case-insensitively.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
    pub fn particle_by_pdgid(
        &self,
        pdgid: impl Into<String>,
    ) -> PdgResult<Option<PdgParticle<'_>>> {
        let pdgid = pdgid.into();
        let sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE upper(pdgparticle.pdgid) = upper(?1)",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_row([&pdgid], |row| PdgParticle::from_row(self, row))
            .optional()?)
    }

    /// Looks up raw metadata for a PDG identifier.
    ///
    /// This returns a [`PdgIdEntry`] for any PDG row type, not just particle
    /// rows.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
    pub fn pdgid(&self, pdgid: impl Into<String>) -> PdgResult<Option<PdgIdEntry>> {
        let pdgid = pdgid.into();
        let mut stmt = self.conn.prepare(
            "SELECT id, pdgid, parent_pdgid, description, mode_number, data_type, flags, year_added, sort
            FROM pdgid
            WHERE upper(pdgid) = upper(?1)",
        )?;
        Ok(stmt
            .query_row([&pdgid], |row| PdgIdEntry::try_from(row))
            .optional()?)
    }

    /// Searches descriptions, text blocks, and footnotes with `SQLite` FTS5.
    ///
    /// Non-alphanumeric separators are normalized into individual quoted search
    /// terms before querying the index.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pdg_rs::{Pdg, TextSearchSource};
    ///
    /// # fn main() -> pdg_rs::PdgResult<()> {
    /// let pdg = Pdg::open()?;
    /// let results = pdg.search_text("K(S)0 mean life")?;
    ///
    /// assert!(results.iter().any(|result| {
    ///     result.source == TextSearchSource::Description
    /// }));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a database error if the search query cannot be executed.
    pub fn search_text(&self, query: impl Into<String>) -> PdgResult<Vec<TextSearchResult>> {
        let Some(query) = fts_query(&query.into()) else {
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
                let pdgid = row.get::<_, PdgId>(0)?;
                let source = row.get::<_, String>(1)?;
                let text_type = row.get::<_, Option<String>>(2)?;
                let sort = row.get::<_, Option<isize>>(3)?;
                let text = row.get::<_, String>(4)?;
                let snippet = row.get::<_, String>(5)?;
                let score = row.get::<_, f64>(6)?;
                let (source, pdg_text) = match source.as_str() {
                    "text" => {
                        let text_type = text_type.unwrap_or_default();
                        let sort = sort.unwrap_or_default();
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
                    "footnote" => (
                        TextSearchSource::Footnote {
                            index: sort.unwrap_or_default(),
                        },
                        None,
                    ),
                    _ => (TextSearchSource::Description, None),
                };
                Ok(TextSearchResult {
                    pdgid,
                    source,
                    text,
                    snippet,
                    score,
                    pdg_text,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Looks up a particle by its Monte Carlo particle ID.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
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

    #[allow(clippy::too_many_lines)]
    /// Searches particles using a [`ParticleSearchQuery`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pdg_rs::{Charge, ParticleClass, ParticleSearchQuery, Pdg};
    ///
    /// # fn main() -> pdg_rs::PdgResult<()> {
    /// let pdg = Pdg::open()?;
    /// let charged_mesons = pdg.search_particles(
    ///     ParticleSearchQuery::new()
    ///         .class(ParticleClass::Meson)
    ///         .charge(Charge::Plus),
    /// )?;
    ///
    /// assert!(charged_mesons.iter().any(|particle| particle.name == "pi+"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a database error if any particle, property, or decay filter query
    /// cannot be executed.
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
            params.push(Value::Text(particle_class.to_code().to_string()));
        }

        if let Some(particle_type) = query.particle_type {
            sql.push_str(" AND cc_type = ?");
            params.push(Value::Text(particle_type.to_code().to_string()));
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
            Some(self.property_entries_by_parent(DataType::Mass)?)
        } else {
            None
        };
        let width_entries = if width_range.is_some() {
            Some(self.property_entries_by_parent(DataType::FullWidth)?)
        } else {
            None
        };
        let lifetime_entries = if lifetime_range.is_some() {
            Some(self.property_entries_by_parent(DataType::Lifetime)?)
        } else {
            None
        };

        let mut filtered_particles = Vec::new();
        for particle in particles {
            if !matches_data_range(
                mass_entries.as_ref(),
                &particle.pdgid,
                mass_range,
                Unit::Mev,
            ) || !matches_data_range(
                width_entries.as_ref(),
                &particle.pdgid,
                width_range,
                Unit::Mev,
            ) || !matches_data_range(
                lifetime_entries.as_ref(),
                &particle.pdgid,
                lifetime_range,
                Unit::Seconds,
            ) {
                continue;
            }

            if decays_to.mode == DecayMatchMode::Exact
                && !decays_to.states.is_empty()
                && !self.particle_matches_exact_decay(
                    &particle.pdgid,
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

    /// Looks up a PDG item by name.
    ///
    /// Items include particles, groups, aliases, charge multiplets, and other
    /// names used to organize decays.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
    pub fn item(&self, name: impl Into<String>) -> PdgResult<Option<PdgItem<'_>>> {
        let name = name.into();
        let mut stmt = self
            .conn
            .prepare("SELECT name, item_type FROM pdgitem WHERE name = ?1")?;
        Ok(stmt
            .query_row([&name], |row| PdgItem::from_row(self, row))
            .optional()?)
    }

    /// Returns child items for a PDG item name.
    ///
    /// # Errors
    ///
    /// Returns a database error if the item map or particle lookup cannot be
    /// queried.
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

    /// Returns parent items for a PDG item name.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
    pub fn item_parents(&self, name: impl Into<String>) -> PdgResult<Vec<PdgItem<'_>>> {
        let name = name.into();
        let mut stmt = self.conn.prepare(
            "SELECT parent.name, parent.item_type FROM pdgitem_map JOIN pdgitem parent ON parent.id = pdgitem_map.pdgitem_id JOIN pdgitem child ON child.id = pdgitem_map.target_id WHERE child.name = ?1 ORDER BY parent.item_type, parent.name",
        )?;
        Ok(stmt
            .query_map([&name], |row| PdgItem::from_row(self, row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Returns PDG identifier rows whose parent is `pdgid`.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
    pub fn children_for_pdgid(&self, pdgid: impl Into<String>) -> PdgResult<Vec<PdgIdEntry>> {
        let pdgid = pdgid.into();
        let mut stmt = self.conn.prepare(
            "SELECT id, pdgid, parent_pdgid, description, mode_number, data_type, flags, year_added, sort
            FROM pdgid
            WHERE upper(parent_pdgid) = upper(?1)
            ORDER BY sort, pdgid",
        )?;
        Ok(stmt
            .query_map([&pdgid], |row| PdgIdEntry::try_from(row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Returns PDG identifier rows linked from `pdgid` through the mapping table.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
    pub fn mapped_entries_for_pdgid(&self, pdgid: impl Into<String>) -> PdgResult<Vec<PdgIdEntry>> {
        let pdgid = pdgid.into();
        let mut stmt = self.conn.prepare(
            "SELECT target.id, target.pdgid, target.parent_pdgid, target.description, target.mode_number, target.data_type, target.flags, target.year_added, target.sort
            FROM pdgid_map
            JOIN pdgid target ON target.id = pdgid_map.target_id
            WHERE upper(pdgid_map.source) = upper(?1)
            ORDER BY pdgid_map.sort, target.pdgid",
        )?;
        Ok(stmt
            .query_map([&pdgid], |row| PdgIdEntry::try_from(row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Returns latest-edition numeric data rows for a PDG identifier.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
    pub fn data_for(&self, pdgid: impl Into<String>) -> PdgResult<Vec<DataEntry<'_>>> {
        let pdgid = pdgid.into();
        let sql = format!(
            "SELECT {} FROM pdgdata WHERE upper(pdgid) = upper(?1) AND edition = ?2 ORDER BY sort",
            DataEntry::COLUMNS
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_map([&pdgid, LATEST_EDITION], |row| {
                DataEntry::from_row(self, row)
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Returns text blocks attached to a PDG identifier.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
    pub fn texts_for(&self, pdgid: impl Into<String>) -> PdgResult<Vec<PdgText>> {
        let pdgid = pdgid.into();
        let mut stmt = self.conn.prepare(
            "SELECT pdgid, type, text, sort FROM pdgtext WHERE pdgid = ?1 ORDER BY sort",
        )?;
        Ok(stmt
            .query_map([&pdgid], |row| PdgText::try_from(row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Returns footnotes attached to a PDG identifier.
    ///
    /// # Errors
    ///
    /// Returns a database error if the query cannot be executed.
    pub fn footnotes_for(&self, pdgid: impl Into<String>) -> PdgResult<Vec<PdgFootnote>> {
        let pdgid = pdgid.into();
        let mut stmt = self.conn.prepare(
            "SELECT pdgid, footnote_index, text, changebar FROM pdgfootnote WHERE pdgid = ?1 ORDER BY footnote_index",
        )?;
        Ok(stmt
            .query_map([&pdgid], |row| PdgFootnote::try_from(row))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Returns measurement rows, values, references, and footnotes for a PDG identifier.
    ///
    /// # Errors
    ///
    /// Returns a database error if measurement, value, or footnote queries cannot
    /// be executed.
    pub fn measurements_for(&self, pdgid: impl Into<String>) -> PdgResult<Vec<PdgMeasurement>> {
        let pdgid = pdgid.into();
        let mut stmt = self.conn.prepare(
            "SELECT pdgmeasurement.id, pdgmeasurement.pdgid, event_count, confidence_level, place, technique, charge, changebar, comment, sort, document_id, publication_name, publication_year, doi, inspire_id, title FROM pdgmeasurement JOIN pdgreference ON pdgreference.id = pdgmeasurement.pdgreference_id WHERE pdgmeasurement.pdgid = ?1 ORDER BY sort",
        )?;
        let mut measurements = stmt
            .query_map([&pdgid], |row| PdgMeasurement::try_from(row))?
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
            params.push(Value::Integer(i64::from(is_outgoing)));
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
        pdgid: &str,
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
            .query_map([pdgid], |row| {
                Ok((
                    row.get::<_, PdgId>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut modes = std::collections::HashMap::<PdgId, Vec<String>>::new();
        for (mode_pdgid, name, multiplier) in rows {
            let products = modes.entry(mode_pdgid).or_default();
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

    fn property_entries_by_parent(
        &self,
        data_type: DataType,
    ) -> PdgResult<std::collections::HashMap<PdgId, Vec<DataEntry<'_>>>> {
        let data_type_code = data_type.to_code();
        let direct_sql = format!(
            "SELECT {}, pdgid.parent_pdgid FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgdata.edition = ?2",
            DataEntry::COLUMNS
        );
        let mut direct_stmt = self.conn.prepare(&direct_sql)?;
        let direct_rows = direct_stmt
            .query_map([data_type_code, LATEST_EDITION], |row| {
                Ok((
                    row.get::<_, PdgId>(DataEntry::COLUMN_COUNT)?,
                    DataEntry::from_row(self, row)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let section_sql = format!(
            "SELECT {}, section.parent_pdgid FROM pdgdata
            JOIN pdgid child ON child.id = pdgdata.pdgid_id
            JOIN pdgid section ON section.pdgid = child.parent_pdgid
            WHERE child.data_type = ?1
                AND section.data_type = ?2
                AND pdgdata.edition = ?3",
            DataEntry::COLUMNS
        );
        let mut section_stmt = self.conn.prepare(&section_sql)?;
        let section_rows = section_stmt
            .query_map(
                [data_type_code, DataType::Section.to_code(), LATEST_EDITION],
                |row| {
                    Ok((
                        row.get::<_, PdgId>(DataEntry::COLUMN_COUNT)?,
                        DataEntry::from_row(self, row)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        let direct_entries = group_property_entries(direct_rows);
        let section_entries = group_property_entries(section_rows);

        Ok(section_entries.into_iter().chain(direct_entries).collect())
    }
}

fn group_property_entries<'pdg>(
    rows: Vec<(PdgId, DataEntry<'pdg>)>,
) -> std::collections::HashMap<PdgId, Vec<DataEntry<'pdg>>> {
    let mut grouped =
        std::collections::HashMap::<PdgId, (Vec<DataEntry<'pdg>>, Vec<DataEntry<'pdg>>)>::new();
    for (parent_pdgid, entry) in rows {
        let (all_entries, summary_entries) = grouped.entry(parent_pdgid).or_default();
        all_entries.push(entry.clone());
        if entry.in_summary_table {
            summary_entries.push(entry);
        }
    }

    grouped
        .into_iter()
        .map(|(pdgid, (all_entries, summary_entries))| {
            let entries = if summary_entries.is_empty() {
                all_entries
            } else {
                summary_entries
            };
            (pdgid, entries)
        })
        .collect()
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
    pdgid: &str,
    range: Option<(f64, f64)>,
    unit: Unit,
) -> bool {
    let Some((min, max)) = range else {
        return true;
    };
    let Some(entries) = entries_by_parent.and_then(|entries| entries.get(pdgid)) else {
        return true;
    };
    if entries.is_empty() {
        return true;
    }

    entries
        .iter()
        .any(|entry| data_interval(entry, unit).is_none_or(|interval| interval.overlaps(min, max)))
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
            .map_or(text.len(), |(char_index, _)| *char_index);
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
            "eV" => Some(0.000_001),
            "u" => Some(931.494_102_42),
            _ => None,
        },
        Unit::Seconds => match unit_text {
            "s" => Some(1.0),
            "yr" | "years" => Some(31_557_600.0),
            _ => None,
        },
    }
}

fn fts_query(query: &str) -> Option<String> {
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

    fn test_pdg() -> Pdg {
        Pdg::open_path(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/pdgall-2025-v0.2.2.sqlite"
        ))
        .unwrap()
    }

    #[test]
    fn charged_decay_states_do_not_expand_to_antiparticle_siblings() {
        let db = test_pdg();
        let names = db
            .expand_decay_state_names("pi+".to_string(), DecayStateExpansion::Inclusive)
            .unwrap();

        assert!(names.contains(&"pi+".to_string()));
        assert!(names.contains(&"pi".to_string()));
        assert!(!names.contains(&"pi-".to_string()));
    }

    #[test]
    fn text_search_finds_pdgid_descriptions() {
        let db = test_pdg();
        let results = db.search_text("K(S)0 MEAN LIFE").unwrap();

        let result = results
            .iter()
            .find(|result| {
                result.pdgid == "S012205" && result.source == TextSearchSource::Description
            })
            .unwrap();

        assert!(result.text.contains("K(S)0 MEAN LIFE"));
        assert!(!result.snippet.is_empty());
        assert!(result.pdg_text.is_none());
    }

    #[test]
    fn text_search_finds_pdgtext_rows() {
        let db = test_pdg();
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
    fn text_search_finds_footnote_rows() {
        let db = test_pdg();
        let results = db.search_text("normalisation decay").unwrap();

        let result = results
            .iter()
            .find(|result| matches!(result.source, TextSearchSource::Footnote { .. }))
            .unwrap();

        assert_eq!(result.pdgid, "S042P86");
        assert!(result.text.contains("normalisation decay"));
        assert!(!result.snippet.is_empty());
        assert!(result.pdg_text.is_none());
    }

    #[test]
    fn text_search_handles_punctuation_heavy_queries() {
        let db = test_pdg();
        let results = db.search_text("K(S)0").unwrap();

        assert!(!results.is_empty());
        assert!(results.iter().any(|result| result.text.contains("K(S)0")));
    }

    #[test]
    fn text_search_orders_by_score() {
        let db = test_pdg();
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
        let db = test_pdg();

        assert!(db.search_text(".,()").unwrap().is_empty());
        assert!(db.search_text("zzzzzznotapdgterm").unwrap().is_empty());
    }
}
