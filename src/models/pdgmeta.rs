use rusqlite::Row;

use crate::{DataType, PdgId};

/// Metadata row for a PDG identifier.
#[derive(Clone, Debug)]
pub struct PdgIdEntry {
    /// Internal row ID from the bundled database.
    pub id: isize,
    /// PDG identifier.
    pub pdgid: PdgId,
    /// Parent PDG identifier, when this row belongs to another entry.
    pub parent_pdgid: Option<PdgId>,
    /// Human-readable PDG description.
    pub description: String,
    /// Decay or mode number for mode-specific rows.
    pub mode_number: Option<isize>,
    /// Data type represented by this identifier.
    pub data_type: DataType,
    /// Raw PDG flags associated with this row.
    pub flags: String,
    /// Edition year in which the row was added, when known.
    pub year_added: Option<isize>,
    /// Sort key used by the PDG tables.
    pub sort: isize,
}

impl TryFrom<&Row<'_>> for PdgIdEntry {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.get(0)?,
            pdgid: row.get(1)?,
            parent_pdgid: row.get(2)?,
            description: row.get(3)?,
            mode_number: row.get(4)?,
            data_type: row.get(5)?,
            flags: row.get(6)?,
            year_added: row.get(7)?,
            sort: row.get(8)?,
        })
    }
}

/// Source table for a [`TextSearchResult`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextSearchSource {
    /// The result matched a [`PdgIdEntry::description`].
    Description,
    /// The result matched a text block.
    Text {
        /// PDG text category code.
        text_type: String,
        /// Sort key for the matched text row.
        sort: isize,
    },
    /// The result matched a footnote.
    Footnote {
        /// Footnote index for the matched footnote row.
        index: isize,
    },
}

/// A full-text search hit from [`crate::Pdg::search_text`].
#[derive(Clone, Debug)]
pub struct TextSearchResult {
    /// PDG identifier for the matching row.
    pub pdgid: PdgId,
    /// Source table and location for the match.
    pub source: TextSearchSource,
    /// Full source text that matched the query.
    pub text: String,
    /// SQLite-generated snippet with matched terms bracketed.
    pub snippet: String,
    /// FTS rank score, where lower values are better matches.
    pub score: f64,
    /// Text-block representation when [`TextSearchSource::Text`] matched.
    pub pdg_text: Option<PdgText>,
}

/// Free-form text block attached to a PDG identifier.
#[derive(Clone, Debug)]
pub struct PdgText {
    /// PDG identifier that owns this text block.
    pub pdgid: PdgId,
    /// PDG text category code.
    pub text_type: String,
    /// Text content, when present in the database.
    pub text: Option<String>,
    /// Sort key used by the PDG tables.
    pub sort: isize,
}

impl TryFrom<&Row<'_>> for PdgText {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            pdgid: row.get(0)?,
            text_type: row.get(1)?,
            text: row.get(2)?,
            sort: row.get(3)?,
        })
    }
}

/// Footnote attached to a PDG identifier or measurement.
#[derive(Clone, Debug)]
pub struct PdgFootnote {
    /// PDG identifier for entry-level footnotes.
    pub pdgid: Option<PdgId>,
    /// Footnote index within the owning entry.
    pub index: Option<isize>,
    /// Footnote text, when present in the database.
    pub text: Option<String>,
    /// Whether the footnote is marked by a PDG change bar.
    pub changebar: bool,
}

impl TryFrom<&Row<'_>> for PdgFootnote {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            pdgid: row.get(0)?,
            index: row.get(1)?,
            text: row.get(2)?,
            changebar: row.get(3)?,
        })
    }
}
