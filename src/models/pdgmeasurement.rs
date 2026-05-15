use std::fmt::Display;

use rusqlite::Row;

use crate::{LimitType, PdgFootnote, PdgId};

/// Bibliographic reference for a PDG measurement.
#[derive(Clone, Debug)]
pub struct PdgReference {
    /// PDG document identifier.
    pub document_id: String,
    /// Publication venue or collaboration name, when available.
    pub publication_name: Option<String>,
    /// Publication year, when available.
    pub publication_year: Option<isize>,
    /// Digital object identifier, when available.
    pub doi: Option<String>,
    /// INSPIRE record identifier, when available.
    pub inspire_id: Option<String>,
    /// Publication title, when available.
    pub title: Option<String>,
}

/// Experimental or observational measurement supporting a PDG data entry.
#[derive(Clone, Debug)]
pub struct PdgMeasurement {
    /// PDG identifier measured by this row.
    pub pdgid: PdgId,
    /// Bibliographic reference for the measurement.
    pub reference: PdgReference,
    /// Event count reported by the measurement.
    pub event_count: Option<String>,
    /// Confidence level associated with the measurement.
    pub confidence_level: Option<f64>,
    /// Location or experiment place label.
    pub place: Option<String>,
    /// Measurement technique label.
    pub technique: Option<String>,
    /// Charge selector or charge state label.
    pub charge: Option<String>,
    /// Whether the measurement is marked by a PDG change bar.
    pub changebar: bool,
    /// PDG comment attached to the measurement.
    pub comment: Option<String>,
    /// Sort key used by the PDG tables.
    pub sort: isize,
    /// Values reported by this measurement.
    pub values: Vec<PdgMeasurementValue>,
    /// Footnotes attached to this measurement.
    pub footnotes: Vec<PdgFootnote>,
    pub(crate) id: isize,
}

impl TryFrom<&Row<'_>> for PdgMeasurement {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.get(0)?,
            pdgid: row.get(1)?,
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

/// Individual value reported by a [`PdgMeasurement`].
#[derive(Clone, Debug)]
pub struct PdgMeasurementValue {
    /// Column name for multi-column measurement rows.
    pub column_name: Option<String>,
    /// Raw value text from the measurement table.
    pub value_text: Option<String>,
    /// Unit text from the measurement table.
    pub unit_text: Option<String>,
    /// Display-ready value text.
    pub display_value_text: Option<String>,
    /// Power-of-ten exponent used when displaying the value.
    pub display_power_of_ten: Option<isize>,
    /// Whether the display value should be interpreted as a percentage.
    pub display_in_percent: Option<bool>,
    /// Limit or range type for this value.
    pub limit_type: Option<LimitType>,
    /// Whether this value is used in the PDG average.
    pub used_in_average: bool,
    /// Whether this value is used in a PDG fit.
    pub used_in_fit: bool,
    /// Parsed numeric central value.
    pub value: Option<f64>,
    /// Positive total uncertainty.
    pub error_positive: Option<f64>,
    /// Negative total uncertainty.
    pub error_negative: Option<f64>,
    /// Positive statistical uncertainty.
    pub stat_error_positive: Option<f64>,
    /// Negative statistical uncertainty.
    pub stat_error_negative: Option<f64>,
    /// Positive systematic uncertainty.
    pub syst_error_positive: Option<f64>,
    /// Negative systematic uncertainty.
    pub syst_error_negative: Option<f64>,
    /// Sort key used by the PDG tables.
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
                .unwrap_or_else(|| "NULL".to_string())
        )?;
        if self.display_in_percent.unwrap_or_default() {
            write!(f, "%")?;
        } else if self.display_power_of_ten.unwrap_or_default() != 0 {
            write!(f, "E{}", self.display_power_of_ten.unwrap_or_default())?;
        }
        if let Some(unit_text) = &self.unit_text
            && !unit_text.is_empty()
        {
            write!(f, " {unit_text}")?;
        }

        Ok(())
    }
}
