use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::{PdgError, PdgResult};

const DATABASE_FILENAME: &str = "pdgall-2026.0.sqlite";
const DATABASE_URL: &str = "https://pdg.lbl.gov/2026/api/pdgall-2026.0.sqlite";
const DATABASE_SHA256: &str = "c82dab0c0a7c5264f23508066cabb8df9eacc1dbfc83c8d20fade6053394ba8a";
const DATABASE_SIZE: u64 = 67_457_024;
const ENV_DATABASE_PATH: &str = "PDG_RS_DB_PATH";
const ENV_CACHE_DIR: &str = "PDG_RS_CACHE_DIR";
const ENV_OFFLINE: &str = "PDG_RS_OFFLINE";

/// Release metadata for the opened PDG database file.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PdgDatabaseInfo {
    /// The file's declared edition, or its newest numeric edition if metadata is absent.
    pub edition: String,
    /// PDG database schema version, when recorded.
    pub schema_version: Option<String>,
    /// PDG release identifier, when recorded.
    pub data_release: Option<String>,
    /// PDG release timestamp as stored in the file.
    pub data_release_timestamp: Option<String>,
    /// Citation for the database release, when recorded.
    pub citation: Option<String>,
}

pub fn read_database_info(conn: &Connection) -> PdgResult<PdgDatabaseInfo> {
    let has_info: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'pdginfo')",
        [],
        |row| row.get(0),
    )?;
    let mut metadata = if has_info {
        let mut stmt = conn.prepare(
            "SELECT name, value FROM pdginfo WHERE value IS NOT NULL
             AND name IN ('edition', 'schema_version', 'data_release', 'data_release_timestamp', 'citation')",
        )?;
        stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?
    } else {
        HashMap::new()
    };
    let edition = match metadata
        .remove("edition")
        .filter(|value| !value.trim().is_empty())
    {
        Some(edition) => edition,
        None => available_editions(conn)?
            .into_iter()
            .next()
            .ok_or(PdgError::DatabaseEditionMissing)?,
    };
    Ok(PdgDatabaseInfo {
        edition,
        schema_version: metadata.remove("schema_version"),
        data_release: metadata.remove("data_release"),
        data_release_timestamp: metadata.remove("data_release_timestamp"),
        citation: metadata.remove("citation"),
    })
}

pub fn available_editions(conn: &Connection) -> PdgResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT edition FROM pdgdata WHERE edition IS NOT NULL AND trim(edition) <> '' ORDER BY edition DESC",
    )?;
    Ok(stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?)
}

pub fn open_read_only(path: &Path) -> PdgResult<Connection> {
    let metadata = path.metadata().map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            PdgError::DatabaseFileMissing(path.to_path_buf())
        } else {
            PdgError::DatabaseFileAccess {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    if !metadata.is_file() {
        return Err(PdgError::DatabaseFileAccess {
            path: path.to_path_buf(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "expected a SQLite file"),
        });
    }
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|source| {
        PdgError::DatabaseOpen {
            path: path.to_path_buf(),
            source,
        }
    })
}

pub fn ensure_database() -> PdgResult<PathBuf> {
    if offline() {
        return cached_database();
    }
    if let Some(path) = configured_database_path() {
        return Ok(path);
    }

    let path = cached_database_path()?;
    match validate_database(&path) {
        Ok(()) => Ok(path),
        Err(error) if should_download_after(&path, &error) => {
            download_database(&path)?;
            validate_database(&path)?;
            Ok(path)
        }
        Err(error) => Err(error),
    }
}

pub fn cached_database() -> PdgResult<PathBuf> {
    if let Some(path) = configured_database_path() {
        return Ok(path);
    }

    let path = cached_database_path()?;
    match validate_database(&path) {
        Ok(()) => Ok(path),
        Err(PdgError::Io(source)) if source.kind() == io::ErrorKind::NotFound => {
            Err(PdgError::OfflineDatabaseMissing(path))
        }
        Err(error) => Err(error),
    }
}

pub fn cached_database_path() -> PdgResult<PathBuf> {
    let cache_dir = env::var_os(ENV_CACHE_DIR)
        .map(PathBuf::from)
        .map_or_else(default_cache_dir, Ok)?;
    Ok(cache_dir.join(DATABASE_FILENAME))
}

fn configured_database_path() -> Option<PathBuf> {
    env::var_os(ENV_DATABASE_PATH).map(PathBuf::from)
}

fn default_cache_dir() -> PdgResult<PathBuf> {
    ProjectDirs::from("org", "pdg-rs", "pdg-rs")
        .map(|dirs| dirs.cache_dir().to_path_buf())
        .ok_or(PdgError::CacheDirectoryUnavailable)
}

fn offline() -> bool {
    env::var(ENV_OFFLINE).is_ok_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "t" | "yes" | "y" | "on"
        )
    })
}

fn should_download_after(path: &Path, error: &PdgError) -> bool {
    matches!(
        error,
        PdgError::Io(io_error) if io_error.kind() == std::io::ErrorKind::NotFound
    ) || matches!(
        error,
        PdgError::DatabaseSizeMismatch { path: error_path, .. }
            | PdgError::DatabaseChecksumMismatch {
                path: error_path, ..
            } if error_path == path
    )
}

fn download_database(path: &Path) -> PdgResult<()> {
    let Some(parent) = path.parent() else {
        return Err(PdgError::CacheDirectoryUnavailable);
    };
    fs::create_dir_all(parent)?;

    eprintln!("Downloading PDG database from {DATABASE_URL}");
    let response = ureq::get(DATABASE_URL)
        .call()
        .map_err(|error| PdgError::Download(error.to_string()))?;
    let mut reader = response.into_reader();
    let mut temp = NamedTempFile::new_in(parent)?;
    let temp_path = temp.path().to_path_buf();
    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        temp.write_all(&buffer[..bytes_read])?;
        hasher.update(&buffer[..bytes_read]);
        size += bytes_read as u64;
    }
    temp.flush()?;

    validate_size(&temp_path, size)?;
    validate_hash(&temp_path, &format!("{:x}", hasher.finalize()))?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn validate_database(path: &Path) -> PdgResult<()> {
    validate_size(path, path.metadata()?.len())?;
    validate_hash(path, &hash_file(path)?)?;
    Ok(())
}

fn validate_size(path: &Path, actual: u64) -> PdgResult<()> {
    if actual == DATABASE_SIZE {
        Ok(())
    } else {
        Err(PdgError::DatabaseSizeMismatch {
            path: path.to_path_buf(),
            expected: DATABASE_SIZE,
            actual,
        })
    }
}

fn validate_hash(path: &Path, actual: &str) -> PdgResult<()> {
    if actual == DATABASE_SHA256 {
        Ok(())
    } else {
        Err(PdgError::DatabaseChecksumMismatch {
            path: path.to_path_buf(),
            expected: DATABASE_SHA256,
            actual: actual.to_owned(),
        })
    }
}

fn hash_file(path: &Path) -> PdgResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
