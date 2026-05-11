use rusqlite::{Connection, MAIN_DB, OptionalExtension};
use thiserror::Error;

mod models;
pub use models::*;

static PDG_BYTES: &[u8] = include_bytes!("../data/pdgall-2025-v0.2.2.sqlite");

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
    #[error("Custom error: {0}")]
    Custom(String),
}

#[derive(Debug)]
pub struct Pdg {
    conn: Connection,
}

impl Pdg {
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
        let mut stmt = self.conn
            .prepare("SELECT pdgid, name, cc_type, mcid, charge, quantum_i, quantum_g, quantum_j, quantum_p, quantum_c FROM pdgparticle WHERE name = ?1")?;
        Ok(stmt
            .query_row([&name], |row| {
                Ok(PdgParticle {
                    db: self,
                    pdg_id: row.get(0)?,
                    name: row.get(1)?,
                    cc_type: row.get(2)?,
                    mcid: row.get(3)?,
                    charge: row.get(4)?,
                    quantum_i: row.get(5)?,
                    quantum_g: row.get(6)?,
                    quantum_j: row.get(7)?,
                    quantum_p: row.get(8)?,
                    quantum_c: row.get(9)?,
                })
            })
            .optional()?)
    }

    pub fn search(&self, name: impl Into<String>) -> PdgResult<Vec<PdgParticle<'_>>> {
        let name = name.into();
        let mut stmt = self.conn
            .prepare("SELECT pdgid, name, cc_type, mcid, charge, quantum_i, quantum_g, quantum_j, quantum_p, quantum_c FROM pdgparticle WHERE name LIKE '%' || ?1 || '%'")?;
        Ok(stmt
            .query_map([&name], |row| {
                Ok(PdgParticle {
                    db: self,
                    pdg_id: row.get(0)?,
                    name: row.get(1)?,
                    cc_type: row.get(2)?,
                    mcid: row.get(3)?,
                    charge: row.get(4)?,
                    quantum_i: row.get(5)?,
                    quantum_g: row.get(6)?,
                    quantum_j: row.get(7)?,
                    quantum_p: row.get(8)?,
                    quantum_c: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn pdgid(&self, pdgid: isize) -> PdgResult<Option<PdgParticle<'_>>> {
        let mut stmt = self.conn
            .prepare("SELECT pdgid, name, cc_type, mcid, charge, quantum_i, quantum_g, quantum_j, quantum_p, quantum_c FROM pdgparticle WHERE mcid = ?1")?;
        Ok(stmt
            .query_row([&pdgid], |row| {
                Ok(PdgParticle {
                    db: self,
                    pdg_id: row.get(0)?,
                    name: row.get(1)?,
                    cc_type: row.get(2)?,
                    mcid: row.get(3)?,
                    charge: row.get(4)?,
                    quantum_i: row.get(5)?,
                    quantum_g: row.get(6)?,
                    quantum_j: row.get(7)?,
                    quantum_p: row.get(8)?,
                    quantum_c: row.get(9)?,
                })
            })
            .optional()?)
    }
}
