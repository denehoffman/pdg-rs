use std::str::FromStr;

use rusqlite::{
    Row,
    types::{FromSql, FromSqlError, FromSqlResult, ValueRef},
};

use crate::{Pdg, PdgError, PdgParticle, PdgResult};

/// Kind of named item in the PDG item hierarchy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PdgItemType {
    /// A concrete particle item.
    Particle,
    /// A group of related items.
    Group,
    /// A charge multiplet item.
    ChargeMultiplet,
    /// An alias for another item.
    Alias,
    /// A synonymous item name.
    Synonym,
    /// A wildcard item used in decay descriptions.
    Wildcard,
    /// A table term item.
    TableTerm,
    /// A redirecting item name.
    Redirect,
    /// An item type code not recognized by this crate.
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
                Self::from_str(s).map_err(|err| FromSqlError::Other(Box::new(err)))
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

/// Named item used to organize particles and decay products.
#[derive(Clone, Debug)]
pub struct PdgItem<'pdg> {
    pub(crate) db: &'pdg Pdg,
    /// Item name as stored in the PDG database.
    pub name: String,
    /// Kind of PDG item.
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

    /// Returns the particle represented by this item, when this item is a particle.
    ///
    /// # Errors
    ///
    /// Returns a database error if the particle lookup cannot be executed.
    pub fn particle(&self) -> PdgResult<Option<PdgParticle<'pdg>>> {
        match &self.item_type {
            PdgItemType::Particle => self.db.particle(&self.name),
            _ => Ok(None),
        }
    }

    /// Returns child items in the PDG item hierarchy.
    ///
    /// # Errors
    ///
    /// Returns a database error if the hierarchy query cannot be executed.
    pub fn children(&self) -> PdgResult<Vec<PdgItemChild<'pdg>>> {
        self.db.item_children(&self.name)
    }

    /// Returns parent items in the PDG item hierarchy.
    ///
    /// # Errors
    ///
    /// Returns a database error if the hierarchy query cannot be executed.
    pub fn parents(&self) -> PdgResult<Vec<Self>> {
        self.db.item_parents(&self.name)
    }

    /// Returns particles related to this item through the item hierarchy.
    ///
    /// # Errors
    ///
    /// Returns a database error if the hierarchy or particle queries cannot be
    /// executed.
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

/// Child item in a [`PdgItem`] hierarchy query.
#[derive(Debug, Clone)]
pub struct PdgItemChild<'pdg> {
    /// Child item.
    pub item: PdgItem<'pdg>,
    /// Sort key from the item mapping table.
    pub sort: isize,
    /// Particle for this child when [`PdgItemChild::item`] is a particle.
    pub particle: Option<PdgParticle<'pdg>>,
}
