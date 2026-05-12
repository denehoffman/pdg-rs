use std::str::FromStr;

use rusqlite::{
    Row,
    types::{FromSql, FromSqlError, FromSqlResult, ValueRef},
};

use crate::{PdgError, PdgParticle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PdgItemType {
    Particle,
    Group,
    ChargeMultiplet,
    Alias,
    Synonym,
    Wildcard,
    TableTerm,
    Redirect,
    Unknown(String),
}

impl FromStr for PdgItemType {
    type Err = PdgError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "P" => Self::Particle,
            "G" => Self::Group,
            "B" => Self::ChargeMultiplet,
            "A" => Self::Alias,
            "S" => Self::Synonym,
            "I" => Self::Wildcard,
            "T" => Self::TableTerm,
            "W" => Self::Redirect,
            other => Self::Unknown(other.to_string()),
        })
    }
}

impl FromSql for PdgItemType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|err| FromSqlError::Other(Box::new(err)))?;
                PdgItemType::from_str(s).map_err(|err| FromSqlError::Other(Box::new(err)))
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdgItem {
    pub name: String,
    pub item_type: PdgItemType,
}

impl TryFrom<&Row<'_>> for PdgItem {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            name: row.get(0)?,
            item_type: row.get(1)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PdgItemChild<'pdg> {
    pub item: PdgItem,
    pub sort: isize,
    pub particle: Option<PdgParticle<'pdg>>,
}
