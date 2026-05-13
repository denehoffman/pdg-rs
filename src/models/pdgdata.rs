use std::fmt::Display;

use rusqlite::Row;

use crate::{LimitType, Pdg, PdgFootnote, PdgId, PdgMeasurement, PdgResult, PdgText, ValueType};

#[derive(Clone, Debug)]
pub struct DataEntry<'pdg> {
    pub(crate) db: &'pdg Pdg,
    pub pdgid: PdgId,
    pub edition: String,
    pub value_type: ValueType,
    pub in_summary_table: bool,
    pub confidence_level: Option<f64>,
    pub limit_type: Option<LimitType>,
    pub comment: Option<String>,
    pub value: Option<f64>,
    pub value_text: Option<String>,
    pub error_positive: Option<f64>,
    pub error_negative: Option<f64>,
    pub scale_factor: Option<f64>,
    pub unit_text: String,
    pub display_value_text: String,
    pub display_power_of_ten: isize,
    pub display_in_percent: bool,
    pub sort: Option<isize>,
}

impl DataEntry<'_> {
    pub const COLUMNS: &'static str = "pdgdata.pdgid, edition, value_type, in_summary_table, confidence_level, limit_type, comment, value, value_text, error_positive, error_negative, scale_factor, unit_text, display_value_text, display_power_of_ten, display_in_percent, pdgdata.sort";
    pub const COLUMN_COUNT: usize = 17;
}

impl<'pdg> DataEntry<'pdg> {
    pub(crate) fn from_row(db: &'pdg Pdg, row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            db,
            pdgid: row.get(0)?,
            edition: row.get(1)?,
            value_type: row.get(2)?,
            in_summary_table: row.get(3)?,
            confidence_level: row.get(4)?,
            limit_type: row.get(5)?,
            comment: row.get(6)?,
            value: row.get(7)?,
            value_text: row.get(8)?,
            error_positive: row.get(9)?,
            error_negative: row.get(10)?,
            scale_factor: row.get(11)?,
            unit_text: row.get(12)?,
            display_value_text: row.get(13)?,
            display_power_of_ten: row.get(14)?,
            display_in_percent: row.get(15)?,
            sort: row.get(16)?,
        })
    }

    pub fn measurements(&self) -> PdgResult<Vec<PdgMeasurement>> {
        self.db.measurements_for(&self.pdgid)
    }

    pub fn footnotes(&self) -> PdgResult<Vec<PdgFootnote>> {
        self.db.footnotes_for(&self.pdgid)
    }

    pub fn texts(&self) -> PdgResult<Vec<PdgText>> {
        self.db.texts_for(&self.pdgid)
    }
}

impl Display for DataEntry<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_value_text)?;
        if self.display_in_percent {
            write!(f, "%")?;
        } else if self.display_power_of_ten != 0 {
            write!(f, "E{}", self.display_power_of_ten)?;
        }
        if !self.unit_text.is_empty() {
            write!(f, " {}", self.unit_text)?;
        }
        Ok(())
    }
}
