use std::{fmt::Display, ops::Deref};

use rusqlite::OptionalExtension;

use crate::{DataEntry, DataType, LATEST_EDITION, Pdg, PdgId, PdgResult};

#[derive(Debug, Clone)]
pub struct PdgParticle<'pdg> {
    pub(crate) db: &'pdg Pdg,
    pub pdg_id: PdgId,
    pub name: String,
    pub cc_type: String,
    pub mcid: Option<isize>,
    pub charge: f64,
    pub quantum_i: Option<String>,
    pub quantum_g: Option<String>,
    pub quantum_j: Option<String>,
    pub quantum_p: Option<String>,
    pub quantum_c: Option<String>,
}

impl<'pdg> PdgParticle<'pdg> {
    pub fn mass(&self) -> PdgResult<Option<Mass>> {
        self.query_map(DataType::Mass, LATEST_EDITION, |data| {
            Some(Mass {
                data: ParticleData {
                    value: data.value?,
                    error: Some((data.error_positive?, data.error_negative?)),
                    unit: data.unit_text,
                },
            })
        })
    }
    pub fn lifetime(&self) -> PdgResult<Option<Lifetime>> {
        self.query_map(DataType::Mass, LATEST_EDITION, |data| {
            Some(Lifetime {
                data: ParticleData {
                    value: data.value?,
                    error: Some((data.error_positive?, data.error_negative?)),
                    unit: data.unit_text,
                },
            })
        })
    }
    pub fn query_all_map<P, T>(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
        predicate: P,
    ) -> PdgResult<Vec<T>>
    where
        P: Fn(DataEntry) -> Option<T>,
    {
        Ok(self
            .query_all(data_type, edition)?
            .into_iter()
            .filter_map(predicate)
            .collect())
    }
    pub fn query_map<P, T>(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
        predicate: P,
    ) -> PdgResult<Option<T>>
    where
        P: Fn(DataEntry) -> Option<T>,
    {
        Ok(self.query(data_type, edition)?.map(predicate).flatten())
    }
    pub fn query_all(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
    ) -> PdgResult<Vec<DataEntry>> {
        let mut stmt = self.db.db().prepare("SELECT pdgdata.pdgid, edition, value_type, confidence_level, limit_type, comment, value, error_positive, error_negative, scale_factor, unit_text FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY edition DESC, pdgdata.sort ASC")?;
        Ok(stmt
            .query_map(
                [&data_type.to_string(), &self.pdg_id, &edition.into()],
                |row| {
                    Ok(DataEntry {
                        pdgid: row.get(0)?,
                        edition: row.get(1)?,
                        value_type: row.get(2)?,
                        confidence_level: row.get(3)?,
                        limit_type: row.get(4)?,
                        comment: row.get(5)?,
                        value: row.get(6)?,
                        error_positive: row.get(7)?,
                        error_negative: row.get(8)?,
                        scale_factor: row.get(9)?,
                        unit_text: row.get(10)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?)
    }
    pub fn query(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
    ) -> PdgResult<Option<DataEntry>> {
        let mut stmt = self.db.db().prepare("SELECT pdgdata.pdgid, edition, value_type, confidence_level, limit_type, comment, value, error_positive, error_negative, scale_factor, unit_text FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY edition DESC, pdgdata.sort ASC")?;
        Ok(stmt
            .query_row(
                [&data_type.to_string(), &self.pdg_id, &edition.into()],
                |row| {
                    Ok(DataEntry {
                        pdgid: row.get(0)?,
                        edition: row.get(1)?,
                        value_type: row.get(2)?,
                        confidence_level: row.get(3)?,
                        limit_type: row.get(4)?,
                        comment: row.get(5)?,
                        value: row.get(6)?,
                        error_positive: row.get(7)?,
                        error_negative: row.get(8)?,
                        scale_factor: row.get(9)?,
                        unit_text: row.get(10)?,
                    })
                },
            )
            .optional()?)
    }
}

#[derive(Debug, Clone)]
pub struct ParticleData {
    pub value: f64,
    pub error: Option<(f64, f64)>,
    pub unit: String,
}
impl Display for ParticleData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let error = self.error.unwrap_or_default();
        write!(f, "{}+{}-{} {}", self.value, error.0, error.1, self.unit)
    }
}

#[derive(Debug, Clone)]
pub struct Mass {
    pub data: ParticleData,
}
impl Deref for Mass {
    type Target = ParticleData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
impl Display for Mass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.data)
    }
}

#[derive(Debug, Clone)]
pub struct Lifetime {
    pub data: ParticleData,
}
impl Deref for Lifetime {
    type Target = ParticleData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
impl Display for Lifetime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.data)
    }
}
