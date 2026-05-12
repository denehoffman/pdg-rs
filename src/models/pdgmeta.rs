use rusqlite::Row;

use crate::PdgId;

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
