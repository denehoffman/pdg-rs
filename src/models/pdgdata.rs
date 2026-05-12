use rusqlite::Row;

use crate::{LimitType, PdgId, ValueType};

#[derive(Clone, Debug)]
pub struct DataEntry {
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

impl DataEntry {
    pub const COLUMNS: &'static str = "pdgdata.pdgid, edition, value_type, in_summary_table, confidence_level, limit_type, comment, value, value_text, error_positive, error_negative, scale_factor, unit_text, display_value_text, display_power_of_ten, display_in_percent, pdgdata.sort";
    pub const COLUMN_COUNT: usize = 17;
}

impl TryFrom<&Row<'_>> for DataEntry {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
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
}
