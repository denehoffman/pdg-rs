use std::str::FromStr;

use rusqlite::{
    Row,
    types::{FromSql, FromSqlError, FromSqlResult, ValueRef},
};

use crate::{Pdg, PdgError, PdgParticle, PdgResult};

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

#[derive(Clone, Debug)]
pub struct PdgItem<'pdg> {
    pub(crate) db: &'pdg Pdg,
    pub name: String,
    pub item_type: PdgItemType,
}

impl<'pdg> PdgItem<'pdg> {
    pub(crate) fn from_row(db: &'pdg Pdg, row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            db,
            name: row.get(0)?,
            item_type: row.get(1)?,
        })
    }

    pub fn particle(&self) -> PdgResult<Option<PdgParticle<'pdg>>> {
        match &self.item_type {
            PdgItemType::Particle => self.db.particle(&self.name),
            _ => Ok(None),
        }
    }

    pub fn children(&self) -> PdgResult<Vec<PdgItemChild<'pdg>>> {
        self.db.item_children(&self.name)
    }

    pub fn parents(&self) -> PdgResult<Vec<PdgItem<'pdg>>> {
        self.db.item_parents(&self.name)
    }

    pub fn related_particles(&self) -> PdgResult<Vec<PdgParticle<'pdg>>> {
        if let Some(particle) = self.particle()? {
            return particle.related_particles();
        }

        Ok(self
            .children()?
            .into_iter()
            .filter_map(|child| child.particle)
            .collect())
    }
}

impl PartialEq for PdgItem<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.item_type == other.item_type
    }
}

impl Eq for PdgItem<'_> {}

#[derive(Debug, Clone)]
pub struct PdgItemChild<'pdg> {
    pub item: PdgItem<'pdg>,
    pub sort: isize,
    pub particle: Option<PdgParticle<'pdg>>,
}
