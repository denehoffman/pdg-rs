use std::fmt::Display;

use rusqlite::Row;

use crate::{LimitType, Pdg, PdgFootnote, PdgId, PdgMeasurement, PdgResult, PdgText, ValueType};

/// Numeric data row for a PDG identifier.
///
/// Data entries retain a link to the originating [`Pdg`](crate::Pdg) handle so
/// related measurements, footnotes, and text blocks can be loaded on demand.
#[derive(Clone, Debug)]
pub struct DataEntry<'pdg> {
    pub(crate) db: &'pdg Pdg,
    /// PDG identifier for this data row.
    pub pdgid: PdgId,
    /// PDG edition for this value.
    pub edition: String,
    /// Classification of the value row.
    pub value_type: ValueType,
    /// Whether the row appears in the PDG summary table.
    pub in_summary_table: bool,
    /// Confidence level associated with the row.
    pub confidence_level: Option<f64>,
    /// Limit or range type, when this row is not a simple central value.
    pub limit_type: Option<LimitType>,
    /// PDG comment attached to the data row.
    pub comment: Option<String>,
    /// Parsed numeric central value.
    pub value: Option<f64>,
    /// Raw value text when the value is not represented solely by [`DataEntry::value`].
    pub value_text: Option<String>,
    /// Positive uncertainty on [`DataEntry::value`].
    pub error_positive: Option<f64>,
    /// Negative uncertainty on [`DataEntry::value`].
    pub error_negative: Option<f64>,
    /// PDG scale factor applied to this value.
    pub scale_factor: Option<f64>,
    /// Unit text from the PDG table.
    pub unit_text: String,
    /// Display-ready value text from the PDG table.
    pub display_value_text: String,
    /// Power-of-ten exponent used when displaying the value.
    pub display_power_of_ten: isize,
    /// Whether the display value should be interpreted as a percentage.
    pub display_in_percent: bool,
    /// Sort key used by the PDG tables.
    pub sort: Option<isize>,
}

impl DataEntry<'_> {
    pub(crate) const COLUMNS: &'static str = "pdgdata.pdgid, edition, value_type, in_summary_table, confidence_level, limit_type, comment, value, value_text, error_positive, error_negative, scale_factor, unit_text, display_value_text, display_power_of_ten, display_in_percent, pdgdata.sort";
    pub(crate) const COLUMN_COUNT: usize = 17;
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

    /// Loads measurements supporting this data entry.
    ///
    /// # Errors
    ///
    /// Returns a database error if the measurement query cannot be executed.
    pub fn measurements(&self) -> PdgResult<Vec<PdgMeasurement>> {
        self.db.measurements_for(&self.pdgid)
    }

    /// Loads footnotes attached to this data entry.
    ///
    /// # Errors
    ///
    /// Returns a database error if the footnote query cannot be executed.
    pub fn footnotes(&self) -> PdgResult<Vec<PdgFootnote>> {
        self.db.footnotes_for(&self.pdgid)
    }

    /// Loads text blocks attached to this data entry.
    ///
    /// # Errors
    ///
    /// Returns a database error if the text query cannot be executed.
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
