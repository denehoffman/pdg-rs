use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    ops::Deref,
};

use rusqlite::{
    OptionalExtension, Row, params_from_iter,
    types::{FromSql, FromSqlError, FromSqlResult, ValueRef},
};

use crate::{
    DataEntry, DataType, LATEST_EDITION, LimitType, Pdg, PdgFootnote, PdgId, PdgItem,
    PdgMeasurement, PdgResult, PdgText,
};

#[derive(Copy, Clone, Debug)]
pub enum ParticleType {
    Particle,
    Antiparticle,
    SelfConjugate,
}

impl Display for ParticleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Particle => "Particle",
                Self::Antiparticle => "Antiparticle",
                Self::SelfConjugate => "Self-Conjugate",
            }
        )
    }
}

impl FromSql for ParticleType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|err| FromSqlError::Other(Box::new(err)))?;
                match s {
                    "P" => Ok(Self::Particle),
                    "A" => Ok(Self::Antiparticle),
                    "S" => Ok(Self::SelfConjugate),
                    _ => Err(FromSqlError::InvalidType),
                }
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ParticleClass {
    GaugeBoson,
    Lepton,
    Quark,
    Meson,
    Baryon,
}

impl ParticleClass {
    pub(crate) fn flag(self) -> &'static str {
        match self {
            Self::GaugeBoson => "G",
            Self::Lepton => "L",
            Self::Quark => "Q",
            Self::Meson => "M",
            Self::Baryon => "B",
        }
    }
}

impl Display for ParticleClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::GaugeBoson => "Gauge/Higgs Boson",
                Self::Lepton => "Lepton",
                Self::Quark => "Quark",
                Self::Meson => "Meson",
                Self::Baryon => "Baryon",
            }
        )
    }
}

impl FromSql for ParticleClass {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|err| FromSqlError::Other(Box::new(err)))?;
                match s {
                    "G" => Ok(Self::GaugeBoson),
                    "L" => Ok(Self::Lepton),
                    "Q" => Ok(Self::Quark),
                    "M" => Ok(Self::Meson),
                    "B" => Ok(Self::Baryon),
                    _ => Err(FromSqlError::InvalidType),
                }
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Charge {
    PlusPlus,
    Plus,
    Neutral,
    Minus,
    MinusMinus,
    PlusOneThird,
    PlusTwoThirds,
    MinusOneThird,
    MinusTwoThirds,
}

impl Display for Charge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::PlusPlus => "+2",
                Self::Plus => "+1",
                Self::Neutral => "0",
                Self::Minus => "-1",
                Self::MinusMinus => "-2",
                Self::PlusOneThird => "+1/3",
                Self::PlusTwoThirds => "+2/3",
                Self::MinusOneThird => "-1/3",
                Self::MinusTwoThirds => "-2/3",
            }
        )
    }
}

impl FromSql for Charge {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Real(v) => Charge::from_f64(v).ok_or(FromSqlError::InvalidType),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

impl Charge {
    fn from_f64(value: f64) -> Option<Self> {
        const EPSILON: f64 = 1e-12;
        [
            (2.0, Self::PlusPlus),
            (1.0, Self::Plus),
            (0.0, Self::Neutral),
            (-1.0, Self::Minus),
            (-2.0, Self::MinusMinus),
            (1.0 / 3.0, Self::PlusOneThird),
            (2.0 / 3.0, Self::PlusTwoThirds),
            (-1.0 / 3.0, Self::MinusOneThird),
            (-2.0 / 3.0, Self::MinusTwoThirds),
        ]
        .into_iter()
        .find_map(|(charge, variant)| (value - charge).abs().lt(&EPSILON).then_some(variant))
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Isospin {
    I0,
    I1,
    I2,
    I3,
    Photon,
    Unknown,
}

impl Display for Isospin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::I0 => "0",
                Self::I1 => "1/2",
                Self::I2 => "1",
                Self::I3 => "3/2",
                Self::Photon => "0 or 1",
                Self::Unknown => "Unknown",
            }
        )
    }
}

impl FromSql for Isospin {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|err| FromSqlError::Other(Box::new(err)))?;
                match s {
                    "0" => Ok(Self::I0),
                    "0,1" => Ok(Self::Photon),
                    "1/2" => Ok(Self::I1),
                    "1" => Ok(Self::I2),
                    "3/2" => Ok(Self::I3),
                    "?" => Ok(Self::Unknown),
                    _ => Err(FromSqlError::InvalidType),
                }
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Clone, Debug)]
pub enum AngularMomentum {
    J0,
    J1,
    J2,
    J3,
    J4,
    J5,
    J6,
    J7,
    J8,
    J9,
    J10,
    J11,
    J12,
    J13,
    J14,
    J15,
    Custom(String),
    Unknown,
}

impl Display for AngularMomentum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::J0 => "0",
                Self::J1 => "1/2",
                Self::J2 => "1",
                Self::J3 => "3/2",
                Self::J4 => "2",
                Self::J5 => "5/2",
                Self::J6 => "3",
                Self::J7 => "7/2",
                Self::J8 => "4",
                Self::J9 => "9/2",
                Self::J10 => "5",
                Self::J11 => "11/2",
                Self::J12 => "6",
                Self::J13 => "13/2",
                Self::J14 => "7",
                Self::J15 => "15/2",
                Self::Custom(s) => s,
                Self::Unknown => "Unknown",
            }
        )
    }
}

impl FromSql for AngularMomentum {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|err| FromSqlError::Other(Box::new(err)))?;
                match s {
                    "0" => Ok(Self::J0),
                    "1/2" => Ok(Self::J1),
                    "1" => Ok(Self::J2),
                    "3/2" => Ok(Self::J3),
                    "2" => Ok(Self::J4),
                    "5/2" => Ok(Self::J5),
                    "3" => Ok(Self::J6),
                    "7/2" => Ok(Self::J7),
                    "4" => Ok(Self::J8),
                    "9/2" => Ok(Self::J9),
                    "5" => Ok(Self::J10),
                    "11/2" => Ok(Self::J11),
                    "6" => Ok(Self::J12),
                    "13/2" => Ok(Self::J13),
                    "7" => Ok(Self::J14),
                    "15/2" => Ok(Self::J15),
                    "?" => Ok(Self::Unknown),
                    other => Ok(Self::Custom(other.to_string())),
                }
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Parity {
    Plus,
    Minus,
    Unknown,
}

impl Display for Parity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Plus => "+",
                Self::Minus => "-",
                Self::Unknown => "Unknown",
            }
        )
    }
}

impl FromSql for Parity {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|err| FromSqlError::Other(Box::new(err)))?;
                match s {
                    "+" => Ok(Self::Plus),
                    "-" => Ok(Self::Minus),
                    "?" => Ok(Self::Unknown),
                    _ => Err(FromSqlError::InvalidType),
                }
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PdgParticle<'pdg> {
    pub(crate) db: &'pdg Pdg,
    pub pdg_id: PdgId,
    pub name: String,
    pub description: String,
    pub particle_type: ParticleType,
    pub particle_class: ParticleClass,
    pub mcid: Option<isize>,
    pub charge: Charge,
    pub quantum_i: Option<Isospin>,
    pub quantum_g: Option<Parity>,
    pub quantum_j: Option<AngularMomentum>,
    pub quantum_p: Option<Parity>,
    pub quantum_c: Option<Parity>,
}

impl Display for PdgParticle<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}, {}, {}, charge {})",
            self.name, self.pdg_id, self.particle_class, self.particle_type, self.charge
        )?;

        if let Some(mcid) = self.mcid {
            write!(f, ", MCID {mcid}")?;
        }

        let mut quantum_numbers = Vec::new();
        if let Some(isospin) = &self.quantum_i {
            quantum_numbers.push(format!("I={isospin}"));
        }
        if let Some(g_parity) = &self.quantum_g {
            quantum_numbers.push(format!("G={g_parity}"));
        }
        if let Some(spin) = &self.quantum_j {
            quantum_numbers.push(format!("J={spin}"));
        }
        if let Some(parity) = &self.quantum_p {
            quantum_numbers.push(format!("P={parity}"));
        }
        if let Some(charge_conjugation) = &self.quantum_c {
            quantum_numbers.push(format!("C={charge_conjugation}"));
        }

        if !quantum_numbers.is_empty() {
            write!(f, ", {}", quantum_numbers.join(", "))?;
        }

        Ok(())
    }
}

impl<'pdg> PdgParticle<'pdg> {
    pub(crate) fn from_row(db: &'pdg Pdg, row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            db,
            pdg_id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            particle_type: row.get(3)?,
            particle_class: row.get(4)?,
            mcid: row.get(5)?,
            charge: row.get(6)?,
            quantum_i: row.get(7)?,
            quantum_g: row.get(8)?,
            quantum_j: row.get(9)?,
            quantum_p: row.get(10)?,
            quantum_c: row.get(11)?,
        })
    }

    pub fn texts(&self) -> PdgResult<Vec<PdgText>> {
        self.db.texts_for(&self.pdg_id)
    }

    pub fn footnotes(&self) -> PdgResult<Vec<PdgFootnote>> {
        self.db.footnotes_for(&self.pdg_id)
    }

    pub fn item(&self) -> PdgResult<Option<PdgItem>> {
        self.db.item(&self.name)
    }

    pub fn item_children(&self) -> PdgResult<Vec<crate::PdgItemChild<'_>>> {
        self.db.item_children(&self.name)
    }

    pub fn parent_items(&self) -> PdgResult<Vec<PdgItem>> {
        self.db.item_parents(&self.name)
    }

    pub fn related_particles(&self) -> PdgResult<Vec<PdgParticle<'_>>> {
        let mut related_particles = Vec::new();
        let mut seen = HashSet::new();
        seen.insert(self.name.clone());

        for parent in self.parent_items()? {
            for child in self.db.item_children(parent.name)? {
                let Some(particle) = child.particle else {
                    continue;
                };
                if seen.insert(particle.name.clone()) {
                    related_particles.push(particle);
                }
            }
        }

        Ok(related_particles)
    }

    pub fn measurements_for(&self, data_type: DataType) -> PdgResult<Vec<PdgMeasurement>> {
        Ok(match self.query(data_type, LATEST_EDITION)? {
            Some(data) => self.db.measurements_for(data.pdgid)?,
            None => Vec::new(),
        })
    }

    pub fn mass(&self) -> PdgResult<Option<Mass>> {
        Ok(self.query_map(DataType::Mass, LATEST_EDITION, |data| {
            ParticleData::try_from(data).ok().map(|data| Mass { data })
        })?)
    }
    pub fn lifetime(&self) -> PdgResult<Option<Lifetime>> {
        Ok(self.query_map(DataType::Lifetime, LATEST_EDITION, |data| {
            ParticleData::try_from(data)
                .ok()
                .map(|data| Lifetime { data })
        })?)
    }
    pub fn branching_fractions(&self) -> PdgResult<Vec<BranchingFraction>> {
        let mut branching_fractions = self.branching_fractions_for(&[
            (
                DataType::ExclusiveBranchingFraction,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction1,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction2,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction3,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction4,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction5,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::InclusiveBranchingFraction,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction1,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction2,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction3,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction4,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction5,
                BranchingFractionKind::Inclusive,
            ),
        ])?;
        self.attach_decay_products(&mut branching_fractions)?;
        self.attach_related_data(&mut branching_fractions)?;
        Ok(branching_fractions
            .into_iter()
            .map(|branching_fraction| branching_fraction.data)
            .collect())
    }
    pub fn exclusive_branching_fractions(&self) -> PdgResult<Vec<BranchingFraction>> {
        let mut branching_fractions = self.branching_fractions_for(&[
            (
                DataType::ExclusiveBranchingFraction,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction1,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction2,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction3,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction4,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction5,
                BranchingFractionKind::Exclusive,
            ),
        ])?;
        self.attach_decay_products(&mut branching_fractions)?;
        self.attach_related_data(&mut branching_fractions)?;
        Ok(branching_fractions
            .into_iter()
            .map(|branching_fraction| branching_fraction.data)
            .collect())
    }
    pub fn inclusive_branching_fractions(&self) -> PdgResult<Vec<BranchingFraction>> {
        let mut branching_fractions = self.branching_fractions_for(&[
            (
                DataType::InclusiveBranchingFraction,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction1,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction2,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction3,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction4,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction5,
                BranchingFractionKind::Inclusive,
            ),
        ])?;
        self.attach_decay_products(&mut branching_fractions)?;
        self.attach_related_data(&mut branching_fractions)?;
        Ok(branching_fractions
            .into_iter()
            .map(|branching_fraction| branching_fraction.data)
            .collect())
    }
    pub fn branching_ratios(&self) -> PdgResult<Vec<BranchingRatio>> {
        Ok(self
            .decay_data(DataType::BranchingRatio, LATEST_EDITION)?
            .into_iter()
            .filter_map(|data| {
                let value = ParticleData::try_from(data.data).ok()?;
                Some(BranchingRatio {
                    pdg_id: data.pdg_id,
                    description: data.description,
                    mode_number: data.mode_number,
                    value,
                })
            })
            .collect())
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
        let sql = format!(
            "SELECT {} FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY edition DESC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        Ok(stmt
            .query_map(
                [&data_type.to_string(), &self.pdg_id, &edition.into()],
                |row| DataEntry::try_from(row),
            )?
            .collect::<Result<Vec<_>, _>>()?)
    }
    pub fn query(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
    ) -> PdgResult<Option<DataEntry>> {
        let sql = format!(
            "SELECT {} FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY edition DESC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        Ok(stmt
            .query_row(
                [&data_type.to_string(), &self.pdg_id, &edition.into()],
                |row| DataEntry::try_from(row),
            )
            .optional()?)
    }

    fn branching_fractions_for(
        &self,
        data_types: &[(DataType, BranchingFractionKind)],
    ) -> PdgResult<Vec<BranchingFractionWithSort>> {
        let mut branching_fractions = Vec::new();
        for (data_type, kind) in data_types {
            branching_fractions.extend(
                self.decay_data(*data_type, LATEST_EDITION)?
                    .into_iter()
                    .filter_map(|data| {
                        let value = ParticleData::try_from(data.data).ok()?;
                        Some(BranchingFractionWithSort {
                            data: BranchingFraction {
                                pdg_id: data.pdg_id,
                                description: data.description,
                                mode_number: data.mode_number,
                                value,
                                kind: *kind,
                                products: Vec::new(),
                                related_data: Vec::new(),
                            },
                            sort: data.sort,
                        })
                    }),
            );
        }
        branching_fractions.sort_by_key(|branching_fraction| branching_fraction.sort);
        Ok(branching_fractions)
    }

    fn decay_data(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
    ) -> PdgResult<Vec<DecayData>> {
        let data_type = data_type.to_string();
        let edition = edition.into();
        let sql = format!(
            "SELECT {}, pdgid.description, pdgid.mode_number, pdgid.sort FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY pdgid.sort ASC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        Ok(stmt
            .query_map([&data_type, &self.pdg_id, &edition], |row| {
                DecayData::try_from(row)
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn attach_decay_products(
        &self,
        branching_fractions: &mut [BranchingFractionWithSort],
    ) -> PdgResult<()> {
        if branching_fractions.is_empty() {
            return Ok(());
        }

        let pdg_ids = branching_fractions
            .iter()
            .map(|branching_fraction| branching_fraction.data.pdg_id.as_str())
            .collect::<Vec<_>>();
        let placeholders = std::iter::repeat_n("?", pdg_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT pdgid, name, is_outgoing, multiplier FROM pdgdecay WHERE pdgid IN ({placeholders}) ORDER BY pdgid ASC, sort ASC"
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        let mut products_by_pdg_id: HashMap<PdgId, Vec<DecayProduct>> = HashMap::new();
        let rows = stmt.query_map(params_from_iter(pdg_ids), |row| {
            Ok((
                row.get::<_, PdgId>(0)?,
                DecayProduct {
                    name: row.get(1)?,
                    is_outgoing: row.get(2)?,
                    multiplier: row.get::<_, i64>(3)? as usize,
                },
            ))
        })?;

        for row in rows {
            let (pdg_id, product) = row?;
            products_by_pdg_id.entry(pdg_id).or_default().push(product);
        }

        for branching_fraction in branching_fractions {
            branching_fraction.data.products = products_by_pdg_id
                .remove(&branching_fraction.data.pdg_id)
                .unwrap_or_default();
        }
        Ok(())
    }

    fn attach_related_data(
        &self,
        branching_fractions: &mut [BranchingFractionWithSort],
    ) -> PdgResult<()> {
        if branching_fractions.is_empty() {
            return Ok(());
        }

        let pdg_ids = branching_fractions
            .iter()
            .map(|branching_fraction| branching_fraction.data.pdg_id.as_str())
            .collect::<Vec<_>>();
        let placeholders = std::iter::repeat_n("?", pdg_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {}, target.description, target.mode_number, target.data_type, pdgid_map.source FROM pdgid_map JOIN pdgid target ON target.id = pdgid_map.target_id JOIN pdgdata ON pdgdata.pdgid_id = target.id WHERE pdgid_map.source IN ({placeholders}) AND pdgdata.edition = ? ORDER BY pdgid_map.source ASC, pdgid_map.sort ASC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );

        let mut params = pdg_ids;
        params.push(LATEST_EDITION);
        let mut stmt = self.db.db().prepare(&sql)?;
        let mut related_by_pdg_id: HashMap<PdgId, Vec<RelatedDataEntry>> = HashMap::new();
        let rows = stmt.query_map(params_from_iter(params), |row| {
            Ok((
                row.get::<_, PdgId>(DataEntry::COLUMN_COUNT + 3)?,
                RelatedDataEntry::try_from(row),
            ))
        })?;

        for row in rows {
            let (pdg_id, related_data) = row?;
            if let Ok(related_data) = related_data {
                related_by_pdg_id
                    .entry(pdg_id)
                    .or_default()
                    .push(related_data);
            }
        }

        for branching_fraction in branching_fractions {
            branching_fraction.data.related_data = related_by_pdg_id
                .remove(&branching_fraction.data.pdg_id)
                .unwrap_or_default();
        }
        Ok(())
    }
}

#[derive(Debug)]
struct DecayData {
    pdg_id: PdgId,
    description: String,
    mode_number: Option<usize>,
    data: DataEntry,
    sort: usize,
}

impl TryFrom<&Row<'_>> for DecayData {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        let data = DataEntry::try_from(row)?;
        Ok(Self {
            pdg_id: data.pdgid.clone(),
            description: row.get(DataEntry::COLUMN_COUNT)?,
            mode_number: row
                .get::<_, Option<isize>>(DataEntry::COLUMN_COUNT + 1)?
                .map(|mode_number| mode_number as usize),
            data,
            sort: row.get::<_, isize>(DataEntry::COLUMN_COUNT + 2)? as usize,
        })
    }
}

#[derive(Debug)]
struct BranchingFractionWithSort {
    data: BranchingFraction,
    sort: usize,
}

#[derive(Debug, Clone)]
pub struct ParticleData {
    pub pdg_id: PdgId,
    pub edition: String,
    pub value_type: crate::ValueType,
    pub in_summary_table: bool,
    pub confidence_level: Option<f64>,
    pub limit_type: Option<LimitType>,
    pub comment: Option<String>,
    pub value: f64,
    pub error: Option<(f64, f64)>,
    pub scale_factor: Option<f64>,
    pub unit_text: String,
    pub display_value_text: String,
    pub display_power_of_ten: isize,
    pub display_in_percent: bool,
    pub sort: Option<isize>,
}

impl TryFrom<DataEntry> for ParticleData {
    type Error = DataEntry;

    fn try_from(data: DataEntry) -> Result<Self, Self::Error> {
        let value = match data.value {
            Some(value) => value,
            None => return Err(data),
        };
        Ok(Self {
            pdg_id: data.pdgid,
            edition: data.edition,
            value_type: data.value_type,
            in_summary_table: data.in_summary_table,
            confidence_level: data.confidence_level,
            limit_type: data.limit_type,
            comment: data.comment,
            value,
            error: match (data.error_positive, data.error_negative) {
                (Some(error_positive), Some(error_negative)) => {
                    Some((error_positive, error_negative))
                }
                _ => None,
            },
            scale_factor: data.scale_factor,
            unit_text: data.unit_text,
            display_value_text: data.display_value_text,
            display_power_of_ten: data.display_power_of_ten,
            display_in_percent: data.display_in_percent,
            sort: data.sort,
        })
    }
}
impl Display for ParticleData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let error = self.error.unwrap_or_default();
        write!(
            f,
            "{}+{}-{} {}",
            self.value, error.0, error.1, self.unit_text
        )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchingFractionKind {
    Exclusive,
    Inclusive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecayProduct {
    pub name: String,
    pub is_outgoing: bool,
    pub multiplier: usize,
}

#[derive(Debug, Clone)]
pub struct BranchingFraction {
    pub pdg_id: PdgId,
    pub description: String,
    pub mode_number: Option<usize>,
    pub value: ParticleData,
    pub kind: BranchingFractionKind,
    pub products: Vec<DecayProduct>,
    pub related_data: Vec<RelatedDataEntry>,
}

#[derive(Debug, Clone)]
pub struct RelatedDataEntry {
    pub pdg_id: PdgId,
    pub description: String,
    pub data_type: DataType,
    pub mode_number: Option<usize>,
    pub value: ParticleData,
}

impl TryFrom<&Row<'_>> for RelatedDataEntry {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        let data = DataEntry::try_from(row)?;
        Ok(Self {
            pdg_id: data.pdgid.clone(),
            description: row.get(DataEntry::COLUMN_COUNT)?,
            mode_number: row
                .get::<_, Option<isize>>(DataEntry::COLUMN_COUNT + 1)?
                .map(|mode_number| mode_number as usize),
            data_type: row.get(DataEntry::COLUMN_COUNT + 2)?,
            value: ParticleData::try_from(data).map_err(|_| rusqlite::Error::InvalidQuery)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BranchingRatio {
    pub pdg_id: PdgId,
    pub description: String,
    pub mode_number: Option<usize>,
    pub value: ParticleData,
}

#[cfg(test)]
mod tests {
    use crate::{BranchingFractionKind, DataType, LimitType, ParticleClass, Pdg, PdgItemType};

    #[test]
    fn displays_particle_identity_and_quantum_numbers() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        assert_eq!(
            pion.to_string(),
            "pi+ (S008, Meson, Particle, charge +1), MCID 211, I=1, G=-, J=0, P=-"
        );
    }

    #[test]
    fn displays_self_conjugate_particles() {
        let db = Pdg::open().unwrap();
        let photon = db.particle("gamma").unwrap().unwrap();

        assert_eq!(
            photon.to_string(),
            "gamma (S000, Gauge/Higgs Boson, Self-Conjugate, charge 0), MCID 22, I=0 or 1, J=1, P=-, C=-"
        );
    }

    #[test]
    fn displays_fractional_charges() {
        let db = Pdg::open().unwrap();

        let down_quark = db.particle("d").unwrap().unwrap();
        assert_eq!(
            down_quark.to_string(),
            "d (Q001, Quark, Particle, charge -1/3), MCID 1, I=1/2, J=1/2, P=+"
        );

        let up_quark = db.particle("u").unwrap().unwrap();
        assert_eq!(
            up_quark.to_string(),
            "u (Q002, Quark, Particle, charge +2/3), MCID 2, I=1/2, J=1/2, P=+"
        );

        let antidown_quark = db.particle("dbar").unwrap().unwrap();
        assert_eq!(
            antidown_quark.to_string(),
            "dbar (Q001, Quark, Antiparticle, charge +1/3), MCID -1, I=1/2, J=1/2, P=-"
        );

        let antiup_quark = db.particle("ubar").unwrap().unwrap();
        assert_eq!(
            antiup_quark.to_string(),
            "ubar (Q002, Quark, Antiparticle, charge -2/3), MCID -2, I=1/2, J=1/2, P=-"
        );
    }

    #[test]
    fn classifies_representative_particles() {
        let db = Pdg::open().unwrap();

        let pion = db.particle("pi+").unwrap().unwrap();
        assert_eq!(pion.description, "pi+-");
        assert_eq!(pion.particle_class, ParticleClass::Meson);
        assert_eq!(
            db.particle("p").unwrap().unwrap().particle_class,
            ParticleClass::Baryon
        );
        assert_eq!(
            db.particle("e-").unwrap().unwrap().particle_class,
            ParticleClass::Lepton
        );
        assert_eq!(
            db.particle("d").unwrap().unwrap().particle_class,
            ParticleClass::Quark
        );
        assert_eq!(
            db.particle("gamma").unwrap().unwrap().particle_class,
            ParticleClass::GaugeBoson
        );
    }

    #[test]
    fn checks_particle_classes() {
        let db = Pdg::open().unwrap();

        let pion = db.particle("pi+").unwrap().unwrap();
        assert!(pion.particle_class == ParticleClass::Meson);
        assert!(db.particle("p").unwrap().unwrap().particle_class == ParticleClass::Baryon);
        assert!(db.particle("e-").unwrap().unwrap().particle_class == ParticleClass::Lepton);
        assert!(db.particle("d").unwrap().unwrap().particle_class == ParticleClass::Quark);
        assert!(db.particle("gamma").unwrap().unwrap().particle_class == ParticleClass::GaugeBoson);
    }

    #[test]
    fn queries_particles_by_class() {
        let db = Pdg::open().unwrap();
        let leptons = db.particles_by_class(ParticleClass::Lepton).unwrap();

        assert!(
            leptons
                .iter()
                .all(|particle| particle.particle_class == ParticleClass::Lepton)
        );
        assert!(leptons.iter().any(|particle| particle.name == "e-"));
        assert!(leptons.iter().any(|particle| particle.name == "mu-"));
        assert!(leptons.iter().any(|particle| particle.name == "nu_e"));
        assert!(!leptons.iter().any(|particle| particle.name == "pi+"));
    }

    #[test]
    fn searches_particles_by_class() {
        let db = Pdg::open().unwrap();
        let pion_mesons = db.search_by_class("pi", ParticleClass::Meson).unwrap();
        let pion_baryons = db.search_by_class("pi", ParticleClass::Baryon).unwrap();

        assert!(!pion_mesons.is_empty());
        assert!(
            pion_mesons
                .iter()
                .all(|particle| particle.particle_class == ParticleClass::Meson)
        );
        assert!(pion_mesons.iter().any(|particle| particle.name == "pi+"));
        assert!(!pion_baryons.iter().any(|particle| particle.name == "pi+"));
    }

    #[test]
    fn loads_items_by_name() {
        let db = Pdg::open().unwrap();
        let pion_pair = db.item("pi+-").unwrap().unwrap();

        assert_eq!(pion_pair.name, "pi+-");
        assert_eq!(pion_pair.item_type, PdgItemType::ChargeMultiplet);
        assert!(db.item("NO_SUCH_ITEM").unwrap().is_none());
    }

    #[test]
    fn loads_item_children_with_particles() {
        let db = Pdg::open().unwrap();
        let pion_children = db.item_children("pi+-").unwrap();

        assert_eq!(pion_children.len(), 2);
        assert_eq!(pion_children[0].item.name, "pi+");
        assert_eq!(pion_children[0].sort, 0);
        assert_eq!(pion_children[0].item.item_type, PdgItemType::Particle);
        assert_eq!(pion_children[0].particle.as_ref().unwrap().name, "pi+");
        assert_eq!(pion_children[1].item.name, "pi-");
        assert_eq!(pion_children[1].sort, 1);

        let w_children = db.item_children("W").unwrap();
        assert_eq!(
            w_children
                .iter()
                .map(|child| child.item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["W+", "W-"]
        );
        assert!(db.item_children("NO_SUCH_ITEM").unwrap().is_empty());
    }

    #[test]
    fn particle_exposes_item_context() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        assert_eq!(pion.item().unwrap().unwrap().name, "pi+");
        let parent_items = pion.parent_items().unwrap();

        assert!(
            parent_items
                .iter()
                .any(|item| item.name == "pi+-" && item.item_type == PdgItemType::ChargeMultiplet)
        );
        assert!(
            parent_items
                .iter()
                .any(|item| item.name == "pi" && item.item_type == PdgItemType::Group)
        );
    }

    #[test]
    fn particle_exposes_related_particles() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();
        let related_particles = pion.related_particles().unwrap();

        assert!(
            related_particles
                .iter()
                .any(|particle| particle.name == "pi-")
        );
        assert!(
            related_particles
                .iter()
                .any(|particle| particle.name == "pi0")
        );
        assert!(
            !related_particles
                .iter()
                .any(|particle| particle.name == "pi+")
        );
    }

    #[test]
    fn loads_texts_for_data_entries() {
        let db = Pdg::open().unwrap();
        let texts = db.texts_for("S008M").unwrap();

        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].pdg_id, "S008M");
        assert_eq!(texts[0].text_type, "h");
        assert_eq!(texts[0].sort, 1);
        assert!(
            texts[0]
                .text
                .as_ref()
                .unwrap()
                .contains("charged pion mass measurements")
        );
    }

    #[test]
    fn loads_footnotes_for_data_entries() {
        let db = Pdg::open().unwrap();
        let footnotes = db.footnotes_for("S008M").unwrap();

        assert!(footnotes.len() >= 10);
        assert_eq!(footnotes[0].pdg_id.as_deref(), Some("S008M"));
        assert_eq!(footnotes[0].index, Some(1));
        assert!(footnotes[0].text.as_ref().unwrap().contains("DAUM 2019"));
    }

    #[test]
    fn particle_forwards_text_and_footnote_lookups() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        assert!(pion.texts().unwrap().is_empty());
        assert!(pion.footnotes().unwrap().is_empty());
    }

    #[test]
    fn loads_measurements_for_data_entries() {
        let db = Pdg::open().unwrap();
        let measurements = db.measurements_for("S008M").unwrap();
        let first = measurements.first().unwrap();
        let first_value = first.values.first().unwrap();
        let first_footnote = first.footnotes.first().unwrap();

        assert_eq!(first.pdg_id, "S008M");
        assert_eq!(first.reference.document_id.trim(), "DAUM 2019");
        assert_eq!(first.reference.publication_year, Some(2019));
        assert_eq!(
            first.reference.doi.as_deref(),
            Some("10.1016/j.physletb.2019.07.027")
        );
        assert!(
            first
                .reference
                .title
                .as_ref()
                .unwrap()
                .contains("charged and neutral pion masses")
        );
        assert_eq!(first_value.column_name.as_deref(), Some("VALUE"));
        assert_eq!(
            first_value.display_value_text.as_deref(),
            Some("139.57021 +-0.00014")
        );
        assert_eq!(first_value.unit_text.as_deref(), Some("MeV"));
        assert!(first_value.used_in_average);
        assert!(first_value.used_in_fit);
        assert_eq!(first_footnote.pdg_id.as_deref(), Some("S008M"));
        assert_eq!(first_footnote.index, Some(1));
        assert!(first_footnote.text.as_ref().unwrap().contains("DAUM 2019"));
        assert!(!first_footnote.changebar);
    }

    #[test]
    fn particle_loads_measurements_for_data_type() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();
        let particle_measurements = pion.measurements_for(DataType::Mass).unwrap();
        let direct_measurements = db.measurements_for("S008M").unwrap();

        assert_eq!(particle_measurements.len(), direct_measurements.len());
        assert_eq!(
            particle_measurements[0].reference.document_id,
            direct_measurements[0].reference.document_id
        );
    }

    #[test]
    fn missing_measurements_return_empty_vec() {
        let db = Pdg::open().unwrap();

        assert!(db.measurements_for("NO_SUCH_PDGID").unwrap().is_empty());
    }

    #[test]
    fn lifetime_uses_lifetime_data_type() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        let mass = pion.mass().unwrap().unwrap();
        let lifetime = pion.lifetime().unwrap().unwrap();

        assert!(mass.value > 100.0);
        assert!(lifetime.value < 0.000001);
    }

    #[test]
    fn exclusive_branching_fractions_include_decay_products() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_fractions = pion.exclusive_branching_fractions().unwrap();
        let muon_mode = branching_fractions
            .iter()
            .find(|branching_fraction| branching_fraction.pdg_id == "S008.1")
            .unwrap();

        assert_eq!(muon_mode.description, "pi+ --> mu+ nu_mu");
        assert_eq!(muon_mode.mode_number, Some(1));
        assert_eq!(muon_mode.kind, BranchingFractionKind::Exclusive);
        assert_eq!(muon_mode.products.len(), 3);
        assert_eq!(muon_mode.products[0].name, "pi+");
        assert!(!muon_mode.products[0].is_outgoing);
        assert_eq!(muon_mode.products[1].name, "mu+");
        assert!(muon_mode.products[1].is_outgoing);
    }

    #[test]
    fn branching_fractions_include_related_ratios() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_fractions = pion.exclusive_branching_fractions().unwrap();
        let muon_mode = branching_fractions
            .iter()
            .find(|branching_fraction| branching_fraction.pdg_id == "S008.1")
            .unwrap();
        let related_ratio = muon_mode
            .related_data
            .iter()
            .find(|related_data| related_data.pdg_id == "S008R10")
            .unwrap();

        assert_eq!(related_ratio.data_type, DataType::BranchingRatio);
        assert!(related_ratio.description.contains("G(pi+ --> e+ nu_e)"));
        assert!(related_ratio.value.value > 0.0);
        assert!(!related_ratio.value.display_value_text.is_empty());
    }

    #[test]
    fn branching_fractions_preserve_non_ratio_related_data() {
        let db = Pdg::open().unwrap();
        let kaon = db.particle("K+").unwrap().unwrap();

        let branching_fractions = kaon.exclusive_branching_fractions().unwrap();
        let muon_mode = branching_fractions
            .iter()
            .find(|branching_fraction| branching_fraction.pdg_id == "S010.1")
            .unwrap();

        assert!(
            muon_mode
                .related_data
                .iter()
                .any(|related_data| related_data.pdg_id == "S010T"
                    && related_data.data_type == DataType::Lifetime
                    && related_data.description == "K+- MEAN LIFE"
                    && related_data.value.value > 0.0)
        );
    }

    #[test]
    fn inclusive_and_exclusive_branching_fractions_are_grouped() {
        let db = Pdg::open().unwrap();
        let b0 = db.particle("B0").unwrap().unwrap();

        let inclusive = b0.inclusive_branching_fractions().unwrap();
        let exclusive = b0.exclusive_branching_fractions().unwrap();
        let all = b0.branching_fractions().unwrap();

        assert!(inclusive.iter().any(|mode| mode.pdg_id == "S042.94"));
        assert!(exclusive.iter().any(|mode| mode.pdg_id == "S042.30"));
        assert_eq!(all.len(), inclusive.len() + exclusive.len());
        let inclusive_position = all
            .iter()
            .position(|mode| mode.pdg_id == "S042.94")
            .unwrap();
        let exclusive_position = all
            .iter()
            .position(|mode| mode.pdg_id == "S042.30")
            .unwrap();

        assert!(inclusive_position < exclusive_position);
    }

    #[test]
    fn branching_ratios_include_descriptions() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_ratios = pion.branching_ratios().unwrap();
        let ratio = branching_ratios
            .iter()
            .find(|ratio| ratio.pdg_id == "S008R2")
            .unwrap();

        assert_eq!(ratio.description, "G(pi+ --> e+ nu_e)/G(total)");
        assert!(ratio.value.value > 0.0);
    }

    #[test]
    fn branching_ratios_include_errors() {
        let db = Pdg::open().unwrap();
        let sigma = db.search("Sigma(2010)").unwrap().remove(0);

        let branching_ratios = sigma.branching_ratios().unwrap();
        let ratio = branching_ratios
            .iter()
            .find(|ratio| ratio.pdg_id == "B002R1")
            .unwrap();

        assert_eq!(ratio.value.error, Some((0.03, 0.03)));
    }

    #[test]
    fn mass_includes_confidence_level_and_limit_type() {
        let db = Pdg::open().unwrap();
        let down_quark = db.particle("d").unwrap().unwrap();
        let mass = down_quark.mass().unwrap().unwrap();

        assert_eq!(mass.confidence_level, Some(90.0));

        let n1895 = db.particle("N(1895)0").unwrap().unwrap();
        let mass_limit = n1895.mass().unwrap().unwrap();

        assert_eq!(mass_limit.limit_type, Some(LimitType::Range));
    }

    #[test]
    fn lifetime_includes_confidence_level_and_limit_type() {
        let db = Pdg::open().unwrap();
        let proton = db.particle("p").unwrap().unwrap();
        let lifetime = proton.lifetime().unwrap().unwrap();

        assert_eq!(lifetime.confidence_level, Some(90.0));
        assert_eq!(lifetime.limit_type, Some(LimitType::LowerLimit));
    }

    #[test]
    fn upper_limit_branching_fractions_preserve_limit_type() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_fractions = pion.exclusive_branching_fractions().unwrap();
        let limit_mode = branching_fractions
            .iter()
            .find(|branching_fraction| branching_fraction.pdg_id == "S008.10")
            .unwrap();

        assert_eq!(limit_mode.value.limit_type, Some(LimitType::UpperLimit));
        assert_eq!(limit_mode.value.error, Some((0.0, 0.0)));
    }
}
