use std::fmt::Display;

use rusqlite::Row;

use crate::{LimitType, PdgFootnote, PdgId};

#[derive(Clone, Debug)]
pub struct PdgReference {
    pub document_id: String,
    pub publication_name: Option<String>,
    pub publication_year: Option<isize>,
    pub doi: Option<String>,
    pub inspire_id: Option<String>,
    pub title: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PdgMeasurement {
    pub pdg_id: PdgId,
    pub reference: PdgReference,
    pub event_count: Option<String>,
    pub confidence_level: Option<f64>,
    pub place: Option<String>,
    pub technique: Option<String>,
    pub charge: Option<String>,
    pub changebar: bool,
    pub comment: Option<String>,
    pub sort: isize,
    pub values: Vec<PdgMeasurementValue>,
    pub footnotes: Vec<PdgFootnote>,
    pub(crate) id: isize,
}

impl TryFrom<&Row<'_>> for PdgMeasurement {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.get(0)?,
            pdg_id: row.get(1)?,
            event_count: row.get(2)?,
            confidence_level: row.get(3)?,
            place: row.get(4)?,
            technique: row.get(5)?,
            charge: row.get(6)?,
            changebar: row.get(7)?,
            comment: row.get(8)?,
            sort: row.get(9)?,
            reference: PdgReference {
                document_id: row.get(10)?,
                publication_name: row.get(11)?,
                publication_year: row.get(12)?,
                doi: row.get(13)?,
                inspire_id: row.get(14)?,
                title: row.get(15)?,
            },
            values: Vec::new(),
            footnotes: Vec::new(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct PdgMeasurementValue {
    pub column_name: Option<String>,
    pub value_text: Option<String>,
    pub unit_text: Option<String>,
    pub display_value_text: Option<String>,
    pub display_power_of_ten: Option<isize>,
    pub display_in_percent: Option<bool>,
    pub limit_type: Option<LimitType>,
    pub used_in_average: bool,
    pub used_in_fit: bool,
    pub value: Option<f64>,
    pub error_positive: Option<f64>,
    pub error_negative: Option<f64>,
    pub stat_error_positive: Option<f64>,
    pub stat_error_negative: Option<f64>,
    pub syst_error_positive: Option<f64>,
    pub syst_error_negative: Option<f64>,
    pub sort: isize,
}

impl TryFrom<&Row<'_>> for PdgMeasurementValue {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            column_name: row.get(0)?,
            value_text: row.get(1)?,
            unit_text: row.get(2)?,
            display_value_text: row.get(3)?,
            display_power_of_ten: row.get(4)?,
            display_in_percent: row.get(5)?,
            limit_type: row.get(6)?,
            used_in_average: row.get(7)?,
            used_in_fit: row.get(8)?,
            value: row.get(9)?,
            error_positive: row.get(10)?,
            error_negative: row.get(11)?,
            stat_error_positive: row.get(12)?,
            stat_error_negative: row.get(13)?,
            syst_error_positive: row.get(14)?,
            syst_error_negative: row.get(15)?,
            sort: row.get(16)?,
        })
    }
}

impl Display for PdgMeasurementValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.display_value_text
                .clone()
                .unwrap_or("NULL".to_string())
        )?;
        if self.display_in_percent.unwrap_or_default() {
            write!(f, "%")?;
        } else if self.display_power_of_ten.unwrap_or_default() != 0 {
            write!(f, "E{}", self.display_power_of_ten.unwrap_or_default())?;
        }
        if let Some(unit_text) = &self.unit_text {
            if !unit_text.is_empty() {
                write!(f, " {}", unit_text)?;
            }
        }
        Ok(())
    }
}
