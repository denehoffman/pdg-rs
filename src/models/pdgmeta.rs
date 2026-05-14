use rusqlite::Row;

use crate::{DataType, PdgId};

#[derive(Clone, Debug)]
pub struct PdgIdEntry {
    pub id: isize,
    pub pdg_id: PdgId,
    pub parent_pdg_id: Option<PdgId>,
    pub description: String,
    pub mode_number: Option<isize>,
    pub data_type: DataType,
    pub flags: String,
    pub year_added: Option<isize>,
    pub sort: isize,
}

impl TryFrom<&Row<'_>> for PdgIdEntry {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.get(0)?,
            pdg_id: row.get(1)?,
            parent_pdg_id: row.get(2)?,
            description: row.get(3)?,
            mode_number: row.get(4)?,
            data_type: row.get(5)?,
            flags: row.get(6)?,
            year_added: row.get(7)?,
            sort: row.get(8)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextSearchSource {
    Description,
    Text { text_type: String, sort: isize },
}

#[derive(Clone, Debug)]
pub struct TextSearchResult {
    pub pdg_id: PdgId,
    pub source: TextSearchSource,
    pub text: String,
    pub snippet: String,
    pub score: f64,
    pub pdg_text: Option<PdgText>,
}

#[derive(Clone, Debug)]
pub struct PdgText {
    pub pdg_id: PdgId,
    pub text_type: String,
    pub text: Option<String>,
    pub sort: isize,
}

impl TryFrom<&Row<'_>> for PdgText {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            pdg_id: row.get(0)?,
            text_type: row.get(1)?,
            text: row.get(2)?,
            sort: row.get(3)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct PdgFootnote {
    pub pdg_id: Option<PdgId>,
    pub index: Option<isize>,
    pub text: Option<String>,
    pub changebar: bool,
}

impl TryFrom<&Row<'_>> for PdgFootnote {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            pdg_id: row.get(0)?,
            index: row.get(1)?,
            text: row.get(2)?,
            changebar: row.get(3)?,
        })
    }
}
