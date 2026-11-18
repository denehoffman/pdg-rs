#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![allow(clippy::empty_docs)]
#![doc = ""]
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value};
use thiserror::Error;

mod database;
mod lookup;
mod models;
mod particle_search;
pub use database::PdgDatabaseInfo;
pub use lookup::{Lookup, LookupSelector};
pub use models::*;
pub use particle_search::{ParticleMatchKind, ParticleSearchHit};

/// The edition of the pinned default download. Use [`Pdg::edition`] for an opened database.
pub const LATEST_EDITION: &str = "2026";

/// Result type returned by fallible `pdg-rs` operations.
pub type PdgResult<T> = Result<T, PdgError>;

/// Errors returned by database access, query validation, code parsing, and quantum number conversion.
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
    /// The specified local database file does not exist.
    #[error("PDG database file does not exist at {0:?}")]
    DatabaseFileMissing(PathBuf),
    /// A local database path cannot be accessed as a regular file.
    #[error("cannot access PDG database file at {path:?}: {source}")]
    DatabaseFileAccess {
        /// Path supplied for the database.
        path: PathBuf,
        /// Underlying file error.
        source: std::io::Error,
    },
    /// The `SQLite` connection for a local file could not be opened read-only.
    #[error("cannot open PDG database at {path:?}: {source}")]
    DatabaseOpen {
        /// Path supplied for the database.
        path: PathBuf,
        /// Underlying database error.
        source: rusqlite::Error,
    },
    /// The opened file does not have the required PDG database schema.
    #[error("invalid or unsupported PDG database schema at {path:?}: {source}")]
    DatabaseSchema {
        /// Path supplied for the database.
        path: PathBuf,
        /// The query error that identified the incompatible file.
        source: rusqlite::Error,
    },
    /// Downloading the PDG database failed.
    #[error("failed to download PDG database: {0}")]
    Download(String),
    /// The database does not declare an edition or contain editioned numeric data.
    #[error("database has no declared edition or editioned numeric data")]
    DatabaseEditionMissing,
    /// The requested numeric edition has no rows in the opened database.
    #[error("PDG edition {requested:?} is unavailable. Available editions: {}", available.join(", "))]
    EditionUnavailable {
        /// The edition that was requested.
        requested: String,
        /// Numeric editions available in the opened file, newest first.
        available: Vec<String>,
    },
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
    /// A property range had unordered bounds or a `NaN` bound.
    #[error("invalid {property} range {min}..{max}. Bounds must be ordered and cannot be NaN")]
    InvalidPropertyRange {
        /// Name of the filtered property.
        property: &'static str,
        /// Minimum of the requested range.
        min: f64,
        /// Maximum of the requested range.
        max: f64,
    },
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
/// [`Pdg::lookup`], [`Pdg::find_particles`], and
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
    info: PdgDatabaseInfo,
    edition_override: Option<String>,
}

impl Pdg {
    const PARTICLE_COLUMNS: &'static str = "pdgparticle.pdgid, name, pdgid.description, cc_type, pdgid.flags, mcid, charge, quantum_i, quantum_g, quantum_j, quantum_p, quantum_c";
    const PARTICLE_JOIN: &'static str =
        "JOIN pdgid ON pdgid.pdgid = pdgparticle.pdgid AND pdgid.data_type = 'PART'";

    /// Opens the default PDG `SQLite` database read-only.
    ///
    /// If `PDG_RS_DB_PATH` is set, that exact database file is opened. Otherwise,
    /// the default database is loaded from the local cache, downloading and
    /// verifying it first when needed.
    ///
    /// # Errors
    ///
    /// Returns an error if the configured database cannot be opened, the cached
    /// database cannot be verified, or the default database cannot be
    /// downloaded.
    pub fn open() -> PdgResult<Self> {
        Self::open_path(database::ensure_database()?)
    }

    /// Opens the default PDG `SQLite` database read-only without downloading it.
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

    /// Opens an existing PDG `SQLite` database at `path` read-only.
    ///
    /// This is useful for applications that manage their own database file or
    /// want to use a different PDG release. Numeric queries default to the file's
    /// edition. Use [`Pdg::with_edition`] to select another edition in the file.
    ///
    /// # Errors
    ///
    /// Returns an error if `path` cannot be opened or initialized or has no edition.
    pub fn open_path(path: impl AsRef<Path>) -> PdgResult<Self> {
        let path = path.as_ref();
        let conn = database::open_read_only(path)?;
        let schema_error = |source| PdgError::DatabaseSchema {
            path: path.to_path_buf(),
            source,
        };
        conn.execute_batch(&format!(
            "SELECT {} FROM pdgparticle {} LIMIT 0;
             SELECT {}, pdgid_id FROM pdgdata LIMIT 0;
             SELECT id, pdgid, parent_pdgid, description, mode_number, data_type, flags, year_added, sort FROM pdgid LIMIT 0;
             SELECT pdgid, text, type, sort FROM pdgtext LIMIT 0;
             SELECT pdgid, text, footnote_index FROM pdgfootnote LIMIT 0;",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN,
            DataEntry::COLUMNS,
        )).map_err(schema_error)?;
        let info = database::read_database_info(&conn).map_err(|error| match error {
            PdgError::SqliteError(source) => schema_error(source),
            other => other,
        })?;
        Ok(Self {
            conn,
            info,
            edition_override: None,
        })
    }

    /// Returns the edition selected for numeric data, property filters, and branching values.
    #[must_use]
    pub fn edition(&self) -> &str {
        self.edition_override
            .as_deref()
            .unwrap_or(&self.info.edition)
    }

    /// Returns metadata for the opened database release.
    #[must_use]
    pub const fn database_info(&self) -> &PdgDatabaseInfo {
        &self.info
    }

    /// Lists the editions with numeric data in this database, newest first.
    ///
    /// # Errors
    /// Returns a database error if the query fails.
    pub fn available_editions(&self) -> PdgResult<Vec<String>> {
        database::available_editions(&self.conn)
    }

    /// Selects an edition for numeric data, property filters, and branching values.
    ///
    /// The particle catalogue, decay listings, text, and measurements come from the file's release.
    ///
    /// # Errors
    /// Returns [`PdgError::EditionUnavailable`] if the edition has no numeric rows,
    /// or a database error if the query fails.
    pub fn with_edition(mut self, edition: impl AsRef<str>) -> PdgResult<Self> {
        let edition = edition.as_ref().trim();
        let available: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM pdgdata WHERE edition = ?1 AND trim(edition) <> '')",
            [edition],
            |row| row.get(0),
        )?;
        if !available {
            return Err(PdgError::EditionUnavailable {
                requested: edition.to_owned(),
                available: self.available_editions()?,
            });
        }
        self.edition_override = Some(edition.to_owned());
        Ok(self)
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

    /// Returns the underlying read-only `SQLite` connection.
    ///
    /// This is useful for advanced queries that are not covered by the typed
    /// API. Prefer the typed methods where possible because they preserve links
    /// back to this [`Pdg`] handle.
    #[must_use]
    pub const fn db(&self) -> &Connection {
        &self.conn
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

    /// Looks up a particle by its PDG item name.
    ///
    /// Use [`Pdg::lookup`] when you already have a PDG identifier,
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
    /// Use [`Pdg::lookup`] with [`LookupSelector::PdgId`] to retrieve every variant.
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
    /// terms before querying the index. The temporary index is built on demand.
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
        let Some(query) = fts_query(&query.into()) else {
            return Ok(Vec::new());
        };
        self.initialize_text_search()?;
        let limit = limit.map_or(-1, |limit| i64::try_from(limit).unwrap_or(i64::MAX));

        let mut stmt = self.conn.prepare(
            "SELECT
                pdgid,
                source,
                text_type,
                sort,
                body,
                snippet(pdg_text_search, 0, '[', ']', '...', 24),
                bm25(pdg_text_search) AS score
            FROM temp.pdg_text_search
            WHERE pdg_text_search MATCH ?1
            ORDER BY score, source, sort, rowid
            LIMIT ?2",
        )?;
        Ok(stmt
            .query_map(rusqlite::params![query, limit], |row| {
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
    /// Returns an error if a property range is invalid or a database query fails.
    pub fn search_particles(&self, query: ParticleSearchQuery) -> PdgResult<Vec<PdgParticle<'_>>> {
        query.validate_ranges()?;
        let mut sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE 1 = 1",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut params = Vec::new();
        let mass_range = query.mass_range_mev;
        let width_range = query.width_range_mev;
        let lifetime_range = query.lifetime_range_seconds;
        let decay_mode = query.decays_to.mode;
        let decay_states =
            self.expand_decay_states(query.decays_to.states, query.decay_state_expansion)?;
        let incoming_states =
            self.expand_decay_states(query.decays_from, query.decay_state_expansion)?;

        if let Some(name_contains) = query.name_contains {
            sql.push_str(" AND name LIKE '%' || ? || '%'");
            params.push(Value::Text(name_contains));
        }

        if let Some(mcid) = query.mcid {
            sql.push_str(" AND mcid = ?");
            params.push(mcid.into());
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

        match query.isospin {
            QuantumFilter::Value(value @ (Isospin::I0 | Isospin::I2)) => {
                sql.push_str(" AND quantum_i IN (?, ?)");
                params.extend([
                    Value::Text(value.to_code().to_owned()),
                    Value::Text(Isospin::Photon.to_code().to_owned()),
                ]);
            }
            QuantumFilter::Value(value) => {
                sql.push_str(" AND quantum_i = ?");
                params.push(Value::Text(value.to_code().to_owned()));
            }
            QuantumFilter::Missing => sql.push_str(" AND quantum_i IS NULL"),
            QuantumFilter::Any => {}
        }
        Self::push_quantum_filter(&mut sql, &mut params, "quantum_g", query.g_parity);
        Self::push_quantum_filter(&mut sql, &mut params, "quantum_j", query.angular_momentum);
        Self::push_quantum_filter(&mut sql, &mut params, "quantum_p", query.parity);
        Self::push_quantum_filter(&mut sql, &mut params, "quantum_c", query.charge_conjugation);

        Self::push_decay_filters(&mut sql, &mut params, &decay_states, true);
        Self::push_decay_filters(&mut sql, &mut params, &incoming_states, false);

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
                query.include_unknown_properties,
            ) || !matches_data_range(
                width_entries.as_ref(),
                &particle.pdgid,
                width_range,
                Unit::Mev,
                query.include_unknown_properties,
            ) || !matches_data_range(
                lifetime_entries.as_ref(),
                &particle.pdgid,
                lifetime_range,
                Unit::Seconds,
                query.include_unknown_properties,
            ) {
                continue;
            }

            if !decay_states.is_empty()
                && !self.particle_matches_decay(&particle.pdgid, &decay_states, decay_mode)?
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

    /// Returns numeric data rows for a PDG identifier in the selected [`Pdg::edition`].
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
            .query_map([pdgid.as_str(), self.edition()], |row| {
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

    fn expand_decay_states(
        &self,
        states: Vec<String>,
        expansion: DecayStateExpansion,
    ) -> PdgResult<Vec<Vec<String>>> {
        states
            .into_iter()
            .map(|state| self.expand_decay_state_names(state, expansion))
            .collect()
    }

    fn push_decay_filters(
        sql: &mut String,
        params: &mut Vec<Value>,
        states: &[Vec<String>],
        is_outgoing: bool,
    ) {
        if states.is_empty() {
            return;
        }

        sql.push_str(
            " AND pdgparticle.pdgid IN (
                SELECT decay_pdgid.parent_pdgid
                FROM pdgid decay_pdgid
                WHERE decay_pdgid.data_type IN ('BFX', 'BFX1', 'BFX2', 'BFX3', 'BFX4', 'BFX5', 'BFI', 'BFI1', 'BFI2', 'BFI3', 'BFI4', 'BFI5')",
        );

        for names in states {
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
            params.extend(names.iter().cloned().map(Value::Text));
        }

        sql.push(')');
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

    fn particle_matches_decay(
        &self,
        pdgid: &str,
        requested: &[Vec<String>],
        mode: DecayMatchMode,
    ) -> PdgResult<bool> {
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
            .any(|products| decay_products_match(requested, products, mode)))
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
            .query_map([data_type_code, self.edition()], |row| {
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
                [data_type_code, DataType::Section.to_code(), self.edition()],
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
    include_unknown: bool,
) -> bool {
    let Some((min, max)) = range else {
        return true;
    };
    let mut intervals = entries_by_parent
        .and_then(|entries| entries.get(pdgid))
        .into_iter()
        .flatten()
        .filter_map(|entry| data_interval(entry, unit))
        .filter(|interval| interval.min <= interval.max)
        .peekable();
    if intervals.peek().is_none() {
        return include_unknown;
    }

    intervals.any(|interval| interval.overlaps(min, max))
}

fn decay_products_match(
    requested: &[Vec<String>],
    products: &[String],
    mode: DecayMatchMode,
) -> bool {
    if requested.len() > products.len()
        || (mode == DecayMatchMode::Exact && requested.len() != products.len())
    {
        return false;
    }

    let mut claims = vec![None; products.len()];
    let mut visited = vec![false; products.len()];
    (0..requested.len()).all(|index| {
        visited.fill(false);
        assign_decay_product(requested, products, &mut claims, &mut visited, index)
    })
}

fn assign_decay_product(
    requested: &[Vec<String>],
    products: &[String],
    claims: &mut [Option<usize>],
    visited: &mut [bool],
    index: usize,
) -> bool {
    for (product_index, product) in products.iter().enumerate() {
        if visited[product_index] || !requested[index].contains(product) {
            continue;
        }

        // An earlier alias match can move to another product to free this one.
        visited[product_index] = true;
        let previous = claims[product_index];
        if previous.is_none_or(|previous_index| {
            assign_decay_product(requested, products, claims, visited, previous_index)
        }) {
            claims[product_index] = Some(index);
            return true;
        }
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

    const TEST_DB_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/pdgall-2025-v0.2.2.sqlite"
    );

    fn test_pdg() -> Pdg {
        Pdg::open_path(TEST_DB_PATH).unwrap()
    }

    #[test]
    fn text_index_builds_on_demand_and_rebuilds_after_rollback() {
        let db = test_pdg();
        assert!(db.db().is_readonly("main").unwrap());
        let index_exists = || {
            db.db().query_row(
                "SELECT EXISTS(SELECT 1 FROM temp.sqlite_schema WHERE name = 'pdg_text_search')",
                [],
                |row| row.get::<_, bool>(0),
            ).unwrap()
        };
        assert!(db.particle("pi+").unwrap().is_some());
        assert!(db.search_text("").unwrap().is_empty());
        assert!(
            db.search_text_with_limit("form factors", Some(0))
                .unwrap()
                .is_empty()
        );
        assert!(!index_exists());

        let transaction = db.db().unchecked_transaction().unwrap();
        assert!(!db.search_text("form factors").unwrap().is_empty());
        assert!(index_exists());
        transaction.rollback().unwrap();

        assert!(!index_exists());
        assert!(!db.search_text("form factors").unwrap().is_empty());
        assert!(index_exists());
    }

    #[test]
    fn selected_edition_applies_to_properties_and_branching_values() {
        let db = test_pdg().with_edition("2024").unwrap();
        assert_eq!(db.edition(), "2024");
        assert_eq!(db.database_info().edition, "2025");

        let pion = db.particle("pi+").unwrap().unwrap();
        assert_eq!(pion.mass().unwrap().unwrap().edition, "2024");
        let resonance = db.particle("a_0(980)0").unwrap().unwrap();
        assert_eq!(resonance.mass().unwrap().unwrap().edition, "2024");

        let fractions = pion.branching_fractions().unwrap();
        assert!(!fractions.is_empty());
        assert!(
            fractions
                .iter()
                .all(|branch| branch.value.edition == "2024")
        );
        let related = fractions
            .iter()
            .flat_map(|branch| &branch.related_data)
            .collect::<Vec<_>>();
        assert!(!related.is_empty());
        assert!(related.iter().all(|branch| branch.value.edition == "2024"));
        let ratios = pion.branching_ratios().unwrap();
        assert!(!ratios.is_empty());
        assert!(ratios.iter().all(|branch| branch.value.edition == "2024"));
    }

    #[test]
    fn numeric_search_and_default_edition_follow_the_opened_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("historical.sqlite");
        std::fs::copy(TEST_DB_PATH, &path).unwrap();
        let conn = Connection::open(&path).unwrap();
        // Separate the mass intervals in the fixture so the filter can distinguish the editions.
        conn.execute_batch(
            "UPDATE pdgdata SET value = 160, error_positive = 0, error_negative = 0
             WHERE pdgid = 'S008M' AND edition = '2024';",
        )
        .unwrap();

        let query = ParticleSearchQuery::new()
            .mcid(211)
            .mass_range_mev(160.0, 160.0);
        let db = Pdg::open_path(&path).unwrap();
        assert!(db.search_particles(query.clone()).unwrap().is_empty());
        let db = db.with_edition("2024").unwrap();
        assert_eq!(db.search_particles(query.clone()).unwrap()[0].name, "pi+");
        drop(db);

        conn.execute_batch(
            "UPDATE pdginfo SET value = '2024' WHERE name = 'edition';
             DELETE FROM pdgdata WHERE edition > '2024';",
        )
        .unwrap();
        let db = Pdg::open_path(&path).unwrap();
        assert_eq!(db.edition(), "2024");
        assert_eq!(db.search_particles(query).unwrap()[0].name, "pi+");
        let data = db.data_for("S008M").unwrap();
        assert!(!data.is_empty());
        assert!(data.iter().all(|row| row.edition == "2024"));
        assert!(
            matches!(db.with_edition("2025"), Err(PdgError::EditionUnavailable { requested, .. }) if requested == "2025")
        );
    }

    #[test]
    fn database_metadata_can_fall_back_to_the_newest_numeric_edition() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE pdgdata (edition TEXT);
             INSERT INTO pdgdata VALUES ('2024'), ('2025'), ('2025'), (NULL), ('');",
        )
        .unwrap();
        let info = database::read_database_info(&conn).unwrap();
        assert_eq!(info.edition, "2025");
        assert_eq!(info.data_release, None);
        assert_eq!(
            database::available_editions(&conn).unwrap(),
            ["2025", "2024"]
        );
    }

    #[test]
    fn lookup_resolves_particle_names_aliases_and_mcids() {
        let db = test_pdg();
        for (selector, pdgid, name) in [
            (LookupSelector::Name("π⁺"), "S008", "pi+"),
            (LookupSelector::Name("X(3872)"), "M176", "chi_c1(3872)"),
            (LookupSelector::Mcid(-211), "S008", "pi-"),
        ] {
            let Lookup::Found { entry, particles } = db.lookup(selector).unwrap() else {
                panic!("{selector:?} should resolve");
            };
            assert_eq!(entry.pdgid, pdgid);
            assert_eq!(particles.len(), 1);
            assert_eq!(particles[0].name, name);
        }
    }

    #[test]
    fn lookup_returns_all_variants_and_supports_non_particle_entries() {
        let db = test_pdg();
        let Lookup::Found { entry, particles } =
            db.lookup(LookupSelector::PdgId(" m009 ")).unwrap()
        else {
            panic!("M009 should resolve");
        };
        assert_eq!(entry.pdgid, "M009");
        let names = particles
            .iter()
            .map(|particle| particle.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["rho(770)+", "rho(770)-", "rho(770)0"]);

        let Lookup::Found { entry, particles } =
            db.lookup(LookupSelector::PdgId("S008245")).unwrap()
        else {
            panic!("S008245 should resolve");
        };
        assert_eq!(entry.data_type, DataType::Section);
        assert!(particles.is_empty());
    }

    #[test]
    fn lookup_surfaces_ambiguity_and_leaves_typos_as_suggestions() {
        let db = test_pdg();
        let Lookup::Ambiguous(choices) =
            db.lookup(LookupSelector::Name("P_c_cbar(4312)+")).unwrap()
        else {
            panic!("multiple entries share this name");
        };
        let ids = choices
            .iter()
            .map(|particle| particle.pdgid.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["B185", "B205"]);

        let Lookup::Ambiguous(choices) = db.lookup(LookupSelector::Name("pion")).unwrap() else {
            panic!("pion refers to several charge states");
        };
        assert_eq!(choices.len(), 3);

        let Lookup::Missing { suggestions } = db.lookup(LookupSelector::Name("prtoon")).unwrap()
        else {
            panic!("a typo should not select a particle");
        };
        assert_eq!(suggestions[0].particle.name, "p");
    }

    #[test]
    fn ranked_search_resolves_database_aliases_and_particle_notation() {
        use ParticleMatchKind::{ExactAlias, ExactName, Typo};

        let db = test_pdg();
        for (query, name, kind) in [
            ("X(3872)", "chi_c1(3872)", ExactAlias),
            ("KS0", "K(S)0", ExactAlias),
            ("K_S^0", "K(S)0", ExactAlias),
            ("Kₛ⁰", "K(S)0", ExactAlias),
            ("σ", "f_0(500)0", ExactAlias),
            ("H0", "H", ExactAlias),
            ("π⁺", "pi+", ExactName),
            ("K̄⁰", "Kbar0", ExactName),
            ("proton", "p", ExactAlias),
            ("anti-proton", "pbar", ExactAlias),
            ("prtoon", "p", Typo),
            ("Lamda(1520)0", "Lambda(1520)0", Typo),
        ] {
            let hits = db
                .find_particles(query, ParticleSearchQuery::new())
                .unwrap();
            assert_eq!(
                hits.first()
                    .map(|hit| (hit.particle.name.as_str(), hit.kind)),
                Some((name, kind)),
                "{query}"
            );
            if query == "X(3872)" {
                assert_eq!(hits[0].matched_name, query);
            }
        }
    }

    #[test]
    fn ranked_search_composes_with_filters_and_keeps_stable_ties() {
        let db = test_pdg();
        let hits = db.find_particles("pi", ParticleSearchQuery::new()).unwrap();
        assert_eq!(hits[0].particle.name, "pi+");
        assert_eq!(hits[1].particle.name, "pi-");

        let hits = db
            .find_particles("pi", ParticleSearchQuery::new().charge(Charge::Minus))
            .unwrap();
        assert_eq!(hits[0].particle.name, "pi-");
        assert!(hits.iter().all(|hit| hit.particle.charge == Charge::Minus));

        let hits = db.find_particles("p", ParticleSearchQuery::new()).unwrap();
        assert_eq!(hits[0].particle.name, "p");
        assert_eq!(hits[0].kind, ParticleMatchKind::ExactName);
        assert!(hits.windows(2).all(|hits| hits[0].kind <= hits[1].kind));
    }

    #[test]
    fn ranked_search_preserves_charge_antiparticles_and_resonance_numbers() {
        let db = test_pdg();
        let charged = db
            .find_particles("Delta(1232)+", ParticleSearchQuery::new())
            .unwrap();
        assert_eq!(charged.len(), 1);
        assert_eq!(charged[0].particle.name, "Delta(1232)+");

        for query in ["lambda(1521)0", "Sigma(1385)2", "\u{2003}"] {
            let hits = db
                .find_particles(query, ParticleSearchQuery::new())
                .unwrap();
            assert!(hits.is_empty(), "{query}");
        }
    }

    #[test]
    fn search_ranges_require_comparable_values_unless_opted_in() {
        let db = test_pdg();
        let missing_width = ParticleSearchQuery::new()
            .mcid(211)
            .width_range_mev(0.0, 1.0);
        assert!(
            db.search_particles(missing_width.clone())
                .unwrap()
                .is_empty()
        );

        let hits = db
            .search_particles(missing_width.include_unknown_properties(true))
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "pi+");

        let known_mass = ParticleSearchQuery::new()
            .mcid(211)
            .mass_range_mev(0.0, 1.0)
            .include_unknown_properties(true);
        assert!(db.search_particles(known_mass).unwrap().is_empty());
    }

    #[test]
    fn search_validates_property_range_bounds() {
        let db = test_pdg();
        for (expected, query) in [
            ("mass", ParticleSearchQuery::new().mass_range_mev(2.0, 1.0)),
            (
                "width",
                ParticleSearchQuery::new().width_range_mev(f64::NAN, 1.0),
            ),
            (
                "lifetime",
                ParticleSearchQuery::new().lifetime_range_seconds(0.0, f64::NAN),
            ),
        ] {
            assert!(matches!(
                db.search_particles(query.clone()),
                Err(PdgError::InvalidPropertyRange { property, .. }) if property == expected
            ));
            assert!(matches!(
                db.find_particles("", query),
                Err(PdgError::InvalidPropertyRange { property, .. }) if property == expected
            ));
        }
    }

    #[test]
    fn decay_search_uses_distinct_occurrences_from_one_mode() {
        let db = test_pdg();
        let literal =
            ParticleSearchQuery::new().decay_state_expansion(DecayStateExpansion::Literal);
        let pion = literal.clone().mcid(211);
        let d_plus = literal.mcid(411);

        for query in [
            pion.clone().decay_contains(["mu+", "mu+"]),
            d_plus.clone().decays_to(["pi+", "pi+"]),
        ] {
            assert!(db.search_particles(query).unwrap().is_empty());
        }

        for (query, name) in [
            (pion.decay_contains(["mu+", "nu_mu"]), "pi+"),
            (d_plus.clone().decay_contains(["pi+", "pi+"]), "D+"),
            (d_plus.decays_to(["K-", "pi+", "pi+"]), "D+"),
        ] {
            let hits = db.search_particles(query).unwrap();
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].name, name);
        }
    }

    #[test]
    fn decay_search_can_reassign_overlapping_alias_matches() {
        let requested = [vec!["pi+".into(), "pi-".into()], vec!["pi+".into()]];
        let products = ["pi+".into(), "pi-".into()];
        assert!(decay_products_match(
            &requested,
            &products,
            DecayMatchMode::Exact
        ));
        assert!(decay_products_match(
            &requested,
            &products,
            DecayMatchMode::Contains
        ));
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
    fn text_search_limit_returns_the_top_ranked_hits() {
        let db = test_pdg();
        let all = db.search_text("decay").unwrap();
        let top = db.search_text_with_limit("decay", Some(5)).unwrap();
        assert_eq!(top.len(), 5);
        for (limited, unlimited) in top.iter().zip(&all) {
            assert_eq!(limited.pdgid, unlimited.pdgid);
            assert_eq!(limited.source, unlimited.source);
            assert_eq!(limited.snippet, unlimited.snippet);
            assert_eq!(limited.score.to_bits(), unlimited.score.to_bits());
        }
    }

    #[test]
    fn text_search_returns_empty_results_for_empty_or_missing_queries() {
        let db = test_pdg();

        assert!(db.search_text(".,()").unwrap().is_empty());
        assert!(db.search_text("zzzzzznotapdgterm").unwrap().is_empty());
    }
}
