use std::{
    env,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::{PdgError, PdgResult};

const DATABASE_FILENAME: &str = "pdgall-2025-v0.2.2.sqlite";
const DATABASE_URL: &str = "https://pdg.lbl.gov/2025/api/pdgall-2025-v0.2.2.sqlite";
const DATABASE_SHA256: &str = "0126ff52a0a8de8d683d56ee9eb10f7366f5f3b9df7ae4a25e2c269026d8566d";
const DATABASE_SIZE: u64 = 63_967_232;
const ENV_DATABASE_PATH: &str = "PDG_RS_DB_PATH";
const ENV_CACHE_DIR: &str = "PDG_RS_CACHE_DIR";
const ENV_OFFLINE: &str = "PDG_RS_OFFLINE";

pub fn ensure_database() -> PdgResult<PathBuf> {
    if let Some(path) = configured_database_path() {
        return Ok(path);
    }

    let path = cached_database_path()?;
    match validate_database(&path) {
        Ok(()) => Ok(path),
        Err(error) if should_download_after(&path, &error) => {
            if offline() {
                return Err(PdgError::OfflineDatabaseMissing(path));
            }
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
    if !path.exists() {
        return Err(PdgError::OfflineDatabaseMissing(path));
    }
    validate_database(&path)?;
    Ok(path)
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
