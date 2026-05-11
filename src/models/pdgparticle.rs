use std::str::FromStr;

use rusqlite::OptionalExtension;

use crate::{DataEntry, DataType, LimitType, Pdg, PdgId, PdgResult, ValueType};

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
    pub fn query(&self, data_type: DataType) -> PdgResult<Option<DataEntry>> {
        let mut stmt = self.db.db().prepare("SELECT pdgdata.pdgid, edition, value_type, confidence_level, limit_type, comment, value, error_positive, error_negative, scale_factor, unit_text FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2")?;
        Ok(stmt
            .query_row([&data_type.to_string(), &self.pdg_id], |row| {
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
            })
            .optional()?)
    }
}
