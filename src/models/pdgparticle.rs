use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use rusqlite::{
    OptionalExtension, Row, params_from_iter,
    types::{FromSql, FromSqlError, FromSqlResult, ValueRef},
};

use crate::{
    DataEntry, DataType, LATEST_EDITION, Pdg, PdgFootnote, PdgId, PdgItem, PdgMeasurement,
    PdgResult, PdgText,
};

/// Particle/antiparticle relationship encoded by the PDG particle table.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ParticleType {
    /// Particle entry.
    Particle,
    /// Antiparticle entry.
    Antiparticle,
    /// Self-conjugate particle entry.
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

impl ParticleType {
    /// Returns the compact PDG database code for this particle type.
    #[must_use]
    pub const fn to_code(self) -> &'static str {
        match self {
            Self::Particle => "P",
            Self::Antiparticle => "A",
            Self::SelfConjugate => "S",
        }
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

/// Broad PDG particle class.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ParticleClass {
    /// Gauge or Higgs boson.
    GaugeBoson,
    /// Lepton.
    Lepton,
    /// Quark.
    Quark,
    /// Meson.
    Meson,
    /// Baryon.
    Baryon,
}

impl ParticleClass {
    /// Returns the compact PDG database code for this particle class.
    #[must_use]
    pub const fn to_code(self) -> &'static str {
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

/// Electric charge in units of the positron charge.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Charge {
    /// Charge `+2`.
    PlusPlus,
    /// Charge `+1`.
    Plus,
    /// Charge `0`.
    Neutral,
    /// Charge `-1`.
    Minus,
    /// Charge `-2`.
    MinusMinus,
    /// Charge `+1/3`.
    PlusOneThird,
    /// Charge `+2/3`.
    PlusTwoThirds,
    /// Charge `-1/3`.
    MinusOneThird,
    /// Charge `-2/3`.
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
            ValueRef::Real(v) => Self::from_f64(v).ok_or(FromSqlError::InvalidType),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

impl Charge {
    pub(crate) fn as_f64(self) -> f64 {
        match self {
            Self::PlusPlus => 2.0,
            Self::Plus => 1.0,
            Self::Neutral => 0.0,
            Self::Minus => -1.0,
            Self::MinusMinus => -2.0,
            Self::PlusOneThird => 1.0 / 3.0,
            Self::PlusTwoThirds => 2.0 / 3.0,
            Self::MinusOneThird => -1.0 / 3.0,
            Self::MinusTwoThirds => -2.0 / 3.0,
        }
    }

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

/// Isospin quantum number.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Isospin {
    /// Isospin `0`.
    I0,
    /// Isospin `1/2`.
    I1,
    /// Isospin `1`.
    I2,
    /// Isospin `3/2`.
    I3,
    /// Ambiguous photon-like value `0 or 1`.
    Photon,
    /// Unknown isospin.
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

/// Angular momentum quantum number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AngularMomentum {
    /// Angular momentum `0`.
    J0,
    /// Angular momentum `1/2`.
    J1,
    /// Angular momentum `1`.
    J2,
    /// Angular momentum `3/2`.
    J3,
    /// Angular momentum `2`.
    J4,
    /// Angular momentum `5/2`.
    J5,
    /// Angular momentum `3`.
    J6,
    /// Angular momentum `7/2`.
    J7,
    /// Angular momentum `4`.
    J8,
    /// Angular momentum `9/2`.
    J9,
    /// Angular momentum `5`.
    J10,
    /// Angular momentum `11/2`.
    J11,
    /// Angular momentum `6`.
    J12,
    /// Angular momentum `13/2`.
    J13,
    /// Angular momentum `7`.
    J14,
    /// Angular momentum `15/2`.
    J15,
    /// Custom angular momentum text from the PDG database.
    Custom(String),
    /// Unknown angular momentum.
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

/// Discrete parity-like quantum number.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Parity {
    /// Positive parity.
    Plus,
    /// Negative parity.
    Minus,
    /// Unknown parity.
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

/// Particle row from the PDG database.
#[derive(Debug, Clone)]
pub struct PdgParticle<'pdg> {
    pub(crate) db: &'pdg Pdg,
    /// PDG identifier for the particle row.
    pub pdgid: PdgId,
    /// PDG item name.
    pub name: String,
    /// Human-readable PDG description.
    pub description: String,
    /// Particle/antiparticle relationship.
    pub particle_type: ParticleType,
    /// Broad particle class.
    pub particle_class: ParticleClass,
    /// Monte Carlo particle ID, when assigned.
    pub mcid: Option<isize>,
    /// Electric charge.
    pub charge: Charge,
    /// Isospin quantum number.
    pub quantum_i: Option<Isospin>,
    /// G-parity quantum number.
    pub quantum_g: Option<Parity>,
    /// Angular momentum quantum number.
    pub quantum_j: Option<AngularMomentum>,
    /// Parity quantum number.
    pub quantum_p: Option<Parity>,
    /// Charge-conjugation parity quantum number.
    pub quantum_c: Option<Parity>,
}

/// Particle property value and its provenance.
#[derive(Clone, Debug)]
pub struct ParticleProperty<'pdg> {
    /// Data type for the property.
    pub data_type: DataType,
    /// Data value for the property.
    pub value: DataEntry<'pdg>,
    /// Where the property was found.
    pub source: PropertySource,
}

/// Source of a [`ParticleProperty`].
#[derive(Clone, Debug)]
pub enum PropertySource {
    /// The property is stored directly under the particle identifier.
    Direct,
    /// The property is stored under a child section.
    Section {
        /// PDG identifier of the section containing the property.
        section_pdgid: PdgId,
    },
}

impl Display for PropertySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => f.write_str("Direct"),
            Self::Section { section_pdgid } => write!(f, "Section {section_pdgid}"),
        }
    }
}

impl Display for PdgParticle<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}, {}, {}, charge {})",
            self.name, self.pdgid, self.particle_class, self.particle_type, self.charge
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
            pdgid: row.get(0)?,
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

    /// Returns a latest-edition property stored directly under this particle.
    ///
    /// # Errors
    ///
    /// Returns a database error if the data query fails.
    pub fn direct_property(&self, data_type: DataType) -> PdgResult<Option<DataEntry<'pdg>>> {
        self.query(data_type, LATEST_EDITION)
    }

    /// Returns a latest-edition property, checking direct and section data.
    ///
    /// # Errors
    ///
    /// Returns a database error if data or child identifier queries fail.
    pub fn property(&self, data_type: DataType) -> PdgResult<Option<ParticleProperty<'pdg>>> {
        if let Some(value) = self.direct_property(data_type)? {
            return Ok(Some(ParticleProperty {
                data_type,
                value,
                source: PropertySource::Direct,
            }));
        }

        for section in self.db.children_for_pdgid(&self.pdgid)? {
            if !matches!(section.data_type, DataType::Section) {
                continue;
            }
            for child in self.db.children_for_pdgid(&section.pdgid)? {
                if child.data_type == data_type {
                    return self.section_property(&section, &child);
                }
            }
        }

        Ok(None)
    }

    fn section_property(
        &self,
        section: &crate::PdgIdEntry,
        child: &crate::PdgIdEntry,
    ) -> PdgResult<Option<ParticleProperty<'pdg>>> {
        let data = self.db.data_for(&child.pdgid)?;
        Ok(data.into_iter().next().map(|value| ParticleProperty {
            data_type: child.data_type,
            value,
            source: PropertySource::Section {
                section_pdgid: section.pdgid.clone(),
            },
        }))
    }

    /// Returns a compact display string for charge and available quantum numbers.
    #[must_use]
    pub fn quantum_summary(&self) -> String {
        [
            Some(format!("Q={}", self.charge)),
            self.quantum_i.as_ref().map(|value| format!("I={value}")),
            self.quantum_g.as_ref().map(|value| format!("G={value}")),
            self.quantum_j.as_ref().map(|value| format!("J={value}")),
            self.quantum_p.as_ref().map(|value| format!("P={value}")),
            self.quantum_c.as_ref().map(|value| format!("C={value}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ")
    }

    /// Loads text blocks attached to this particle.
    ///
    /// # Errors
    ///
    /// Returns a database error if the text query fails.
    pub fn texts(&self) -> PdgResult<Vec<PdgText>> {
        self.db.texts_for(&self.pdgid)
    }

    /// Loads footnotes attached to this particle.
    ///
    /// # Errors
    ///
    /// Returns a database error if the footnote query fails.
    pub fn footnotes(&self) -> PdgResult<Vec<PdgFootnote>> {
        self.db.footnotes_for(&self.pdgid)
    }

    /// Loads the item hierarchy node for this particle.
    ///
    /// # Errors
    ///
    /// Returns a database error if the item query fails.
    pub fn item(&self) -> PdgResult<Option<PdgItem<'pdg>>> {
        self.db.item(&self.name)
    }

    /// Loads child items for this particle's item name.
    ///
    /// # Errors
    ///
    /// Returns a database error if the item hierarchy query fails.
    pub fn item_children(&self) -> PdgResult<Vec<crate::PdgItemChild<'pdg>>> {
        self.db.item_children(&self.name)
    }

    /// Loads parent items for this particle's item name.
    ///
    /// # Errors
    ///
    /// Returns a database error if the item hierarchy query fails.
    pub fn parent_items(&self) -> PdgResult<Vec<PdgItem<'pdg>>> {
        self.db.item_parents(&self.name)
    }

    /// Returns sibling particles related through the item hierarchy.
    ///
    /// # Errors
    ///
    /// Returns a database error if hierarchy or particle queries fail.
    pub fn related_particles(&self) -> PdgResult<Vec<Self>> {
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

    /// Loads measurements supporting a property data type.
    ///
    /// # Errors
    ///
    /// Returns a database error if the property or measurement queries fail.
    pub fn measurements_for(&self, data_type: DataType) -> PdgResult<Vec<PdgMeasurement>> {
        Ok(match self.property(data_type)? {
            Some(property) => self.db.measurements_for(property.value.pdgid)?,
            None => Vec::new(),
        })
    }

    /// Returns the latest-edition mass entry, if available.
    ///
    /// # Errors
    ///
    /// Returns a database error if the property query fails.
    pub fn mass(&self) -> PdgResult<Option<DataEntry<'pdg>>> {
        Ok(self
            .property(DataType::Mass)?
            .map(|property| property.value))
    }
    /// Returns the latest-edition lifetime entry, if available.
    ///
    /// # Errors
    ///
    /// Returns a database error if the property query fails.
    pub fn lifetime(&self) -> PdgResult<Option<DataEntry<'pdg>>> {
        Ok(self
            .property(DataType::Lifetime)?
            .map(|property| property.value))
    }
    /// Returns the latest-edition full-width entry, if available.
    ///
    /// # Errors
    ///
    /// Returns a database error if the property query fails.
    pub fn width(&self) -> PdgResult<Option<DataEntry<'pdg>>> {
        Ok(self
            .property(DataType::FullWidth)?
            .map(|property| property.value))
    }
    /// Returns exclusive and inclusive branching fractions for this particle.
    ///
    /// # Errors
    ///
    /// Returns a database error if branching data, decay products, or related
    /// data cannot be loaded.
    pub fn branching_fractions(&self) -> PdgResult<Vec<BranchingFraction<'pdg>>> {
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
    /// Returns exclusive branching fractions for this particle.
    ///
    /// # Errors
    ///
    /// Returns a database error if branching data, decay products, or related
    /// data cannot be loaded.
    pub fn exclusive_branching_fractions(&self) -> PdgResult<Vec<BranchingFraction<'pdg>>> {
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
    /// Returns inclusive branching fractions for this particle.
    ///
    /// # Errors
    ///
    /// Returns a database error if branching data, decay products, or related
    /// data cannot be loaded.
    pub fn inclusive_branching_fractions(&self) -> PdgResult<Vec<BranchingFraction<'pdg>>> {
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
    /// Returns branching-ratio rows for this particle.
    ///
    /// # Errors
    ///
    /// Returns a database error if branching-ratio data cannot be loaded.
    pub fn branching_ratios(&self) -> PdgResult<Vec<BranchingRatio<'pdg>>> {
        Ok(self
            .decay_data(DataType::BranchingRatio, LATEST_EDITION)?
            .into_iter()
            .map(|data| BranchingRatio {
                pdgid: data.pdgid,
                description: data.description,
                mode_number: data.mode_number,
                value: data.data,
            })
            .collect())
    }
    /// Queries all latest matching data entries and maps them through `predicate`.
    ///
    /// Entries for which `predicate` returns `None` are omitted.
    ///
    /// # Errors
    ///
    /// Returns a database error if the data query fails.
    pub fn query_all_map<P, T>(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
        predicate: P,
    ) -> PdgResult<Vec<T>>
    where
        P: Fn(DataEntry<'pdg>) -> Option<T>,
    {
        Ok(self
            .query_all(data_type, edition)?
            .into_iter()
            .filter_map(predicate)
            .collect())
    }
    /// Queries the first matching data entry and maps it through `predicate`.
    ///
    /// # Errors
    ///
    /// Returns a database error if the data query fails.
    pub fn query_map<P, T>(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
        predicate: P,
    ) -> PdgResult<Option<T>>
    where
        P: Fn(DataEntry<'pdg>) -> Option<T>,
    {
        Ok(self.query(data_type, edition)?.and_then(predicate))
    }
    /// Returns all data entries of `data_type` for a PDG edition.
    ///
    /// # Errors
    ///
    /// Returns a database error if the data query fails.
    pub fn query_all(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
    ) -> PdgResult<Vec<DataEntry<'pdg>>> {
        let sql = format!(
            "SELECT {} FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY edition DESC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        Ok(stmt
            .query_map([data_type.to_code(), &self.pdgid, &edition.into()], |row| {
                DataEntry::from_row(self.db, row)
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }
    /// Returns the first data entry of `data_type` for a PDG edition.
    ///
    /// # Errors
    ///
    /// Returns a database error if the data query fails.
    pub fn query(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
    ) -> PdgResult<Option<DataEntry<'pdg>>> {
        let sql = format!(
            "SELECT {} FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY edition DESC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        Ok(stmt
            .query_row([data_type.to_code(), &self.pdgid, &edition.into()], |row| {
                DataEntry::from_row(self.db, row)
            })
            .optional()?)
    }

    fn branching_fractions_for(
        &self,
        data_types: &[(DataType, BranchingFractionKind)],
    ) -> PdgResult<Vec<BranchingFractionWithSort<'pdg>>> {
        let mut branching_fractions = Vec::new();
        for (data_type, kind) in data_types {
            branching_fractions.extend(
                self.decay_data(*data_type, LATEST_EDITION)?
                    .into_iter()
                    .map(|data| BranchingFractionWithSort {
                        data: BranchingFraction {
                            pdgid: data.pdgid,
                            description: data.description,
                            mode_number: data.mode_number,
                            value: data.data,
                            kind: *kind,
                            products: Vec::new(),
                            related_data: Vec::new(),
                        },
                        sort: data.sort,
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
    ) -> PdgResult<Vec<DecayData<'pdg>>> {
        let data_type = data_type.to_code();
        let edition = edition.into();
        let sql = format!(
            "SELECT {}, pdgid.description, pdgid.mode_number, pdgid.sort FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY pdgid.sort ASC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        Ok(stmt
            .query_map([data_type, &self.pdgid, &edition], |row| {
                DecayData::from_row(self.db, row)
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn attach_decay_products(
        &self,
        branching_fractions: &mut [BranchingFractionWithSort<'pdg>],
    ) -> PdgResult<()> {
        if branching_fractions.is_empty() {
            return Ok(());
        }

        let pdgids = branching_fractions
            .iter()
            .map(|branching_fraction| branching_fraction.data.pdgid.as_str())
            .collect::<Vec<_>>();
        let placeholders = std::iter::repeat_n("?", pdgids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT pdgid, name, is_outgoing, multiplier FROM pdgdecay WHERE pdgid IN ({placeholders}) ORDER BY pdgid ASC, sort ASC"
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        let mut products_by_pdgid: HashMap<PdgId, Vec<DecayProduct<'pdg>>> = HashMap::new();
        let rows = stmt.query_map(params_from_iter(pdgids), |row| {
            Ok((
                row.get::<_, PdgId>(0)?,
                DecayProduct {
                    db: self.db,
                    name: row.get(1)?,
                    is_outgoing: row.get(2)?,
                    multiplier: row.get::<_, i64>(3)?,
                },
            ))
        })?;

        for row in rows {
            let (pdgid, product) = row?;
            products_by_pdgid.entry(pdgid).or_default().push(product);
        }

        for branching_fraction in branching_fractions {
            branching_fraction.data.products = products_by_pdgid
                .remove(&branching_fraction.data.pdgid)
                .unwrap_or_default();
        }
        Ok(())
    }

    fn attach_related_data(
        &self,
        branching_fractions: &mut [BranchingFractionWithSort<'pdg>],
    ) -> PdgResult<()> {
        if branching_fractions.is_empty() {
            return Ok(());
        }

        let pdgids = branching_fractions
            .iter()
            .map(|branching_fraction| branching_fraction.data.pdgid.as_str())
            .collect::<Vec<_>>();
        let placeholders = std::iter::repeat_n("?", pdgids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {}, target.description, target.mode_number, target.data_type, pdgid_map.source FROM pdgid_map JOIN pdgid target ON target.id = pdgid_map.target_id JOIN pdgdata ON pdgdata.pdgid_id = target.id WHERE pdgid_map.source IN ({placeholders}) AND pdgdata.edition = ? ORDER BY pdgid_map.source ASC, pdgid_map.sort ASC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );

        let mut params = pdgids;
        params.push(LATEST_EDITION);
        let mut stmt = self.db.db().prepare(&sql)?;
        let mut related_by_pdgid: HashMap<PdgId, Vec<RelatedDataEntry<'pdg>>> = HashMap::new();
        let rows = stmt.query_map(params_from_iter(params), |row| {
            Ok((
                row.get::<_, PdgId>(DataEntry::COLUMN_COUNT + 3)?,
                RelatedDataEntry::from_row(self.db, row),
            ))
        })?;

        for row in rows {
            let (pdgid, related_data) = row?;
            if let Ok(related_data) = related_data {
                related_by_pdgid
                    .entry(pdgid)
                    .or_default()
                    .push(related_data);
            }
        }

        for branching_fraction in branching_fractions {
            branching_fraction.data.related_data = related_by_pdgid
                .remove(&branching_fraction.data.pdgid)
                .unwrap_or_default();
        }
        Ok(())
    }
}

#[derive(Debug)]
struct DecayData<'pdg> {
    pdgid: PdgId,
    description: String,
    mode_number: Option<u32>,
    data: DataEntry<'pdg>,
    sort: u32,
}

impl<'pdg> DecayData<'pdg> {
    fn from_row(db: &'pdg Pdg, row: &Row<'_>) -> rusqlite::Result<Self> {
        let data = DataEntry::from_row(db, row)?;
        Ok(Self {
            pdgid: data.pdgid.clone(),
            description: row.get(DataEntry::COLUMN_COUNT)?,
            mode_number: row.get::<_, Option<u32>>(DataEntry::COLUMN_COUNT + 1)?,
            data,
            sort: row.get::<_, u32>(DataEntry::COLUMN_COUNT + 2)?,
        })
    }
}

#[derive(Debug)]
struct BranchingFractionWithSort<'pdg> {
    data: BranchingFraction<'pdg>,
    sort: u32,
}

/// Whether a branching fraction is exclusive or inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchingFractionKind {
    /// Exclusive branching fraction.
    Exclusive,
    /// Inclusive branching fraction.
    Inclusive,
}

impl Display for BranchingFractionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Exclusive => "Exclusive",
            Self::Inclusive => "Inclusive",
        })
    }
}

/// Decay product listed for a branching-fraction row.
#[derive(Debug, Clone)]
pub struct DecayProduct<'pdg> {
    pub(crate) db: &'pdg Pdg,
    /// PDG item name for the product.
    pub name: String,
    /// Whether this product is outgoing from the decay.
    pub is_outgoing: bool,
    /// Multiplicity of this product in the decay row.
    pub multiplier: i64,
}

impl<'pdg> DecayProduct<'pdg> {
    /// Loads the PDG item for this decay product.
    ///
    /// # Errors
    ///
    /// Returns a database error if the item query fails.
    pub fn item(&self) -> PdgResult<Option<PdgItem<'pdg>>> {
        self.db.item(&self.name)
    }

    /// Loads the particle for this decay product, when it names a particle.
    ///
    /// # Errors
    ///
    /// Returns a database error if the particle query fails.
    pub fn particle(&self) -> PdgResult<Option<PdgParticle<'pdg>>> {
        self.db.particle(&self.name)
    }

    /// Loads child items for this decay product name.
    ///
    /// # Errors
    ///
    /// Returns a database error if the hierarchy query fails.
    pub fn children(&self) -> PdgResult<Vec<crate::PdgItemChild<'pdg>>> {
        self.db.item_children(&self.name)
    }

    /// Loads parent items for this decay product name.
    ///
    /// # Errors
    ///
    /// Returns a database error if the hierarchy query fails.
    pub fn parents(&self) -> PdgResult<Vec<PdgItem<'pdg>>> {
        self.db.item_parents(&self.name)
    }
}

/// Branching-fraction data for a particle decay mode.
#[derive(Debug, Clone)]
pub struct BranchingFraction<'pdg> {
    /// PDG identifier for the branching-fraction row.
    pub pdgid: PdgId,
    /// Human-readable decay description.
    pub description: String,
    /// PDG mode number, when available.
    pub mode_number: Option<u32>,
    /// Numeric value for the branching fraction.
    pub value: DataEntry<'pdg>,
    /// Exclusive or inclusive branching-fraction kind.
    pub kind: BranchingFractionKind,
    /// Decay products attached to the branching-fraction row.
    pub products: Vec<DecayProduct<'pdg>>,
    /// Other data rows mapped to this branching-fraction row.
    pub related_data: Vec<RelatedDataEntry<'pdg>>,
}

impl BranchingFraction<'_> {
    /// Loads measurements supporting this branching fraction.
    ///
    /// # Errors
    ///
    /// Returns a database error if the measurement query fails.
    pub fn measurements(&self) -> PdgResult<Vec<PdgMeasurement>> {
        self.value.measurements()
    }

    /// Loads footnotes attached to this branching fraction.
    ///
    /// # Errors
    ///
    /// Returns a database error if the footnote query fails.
    pub fn footnotes(&self) -> PdgResult<Vec<PdgFootnote>> {
        self.value.footnotes()
    }

    /// Loads text blocks attached to this branching fraction.
    ///
    /// # Errors
    ///
    /// Returns a database error if the text query fails.
    pub fn texts(&self) -> PdgResult<Vec<PdgText>> {
        self.value.texts()
    }
}

/// Data row related to a branching fraction through the PDG mapping table.
#[derive(Debug, Clone)]
pub struct RelatedDataEntry<'pdg> {
    /// PDG identifier for the related row.
    pub pdgid: PdgId,
    /// Human-readable PDG description.
    pub description: String,
    /// Data type for the related row.
    pub data_type: DataType,
    /// PDG mode number, when available.
    pub mode_number: Option<u32>,
    /// Numeric value for the related row.
    pub value: DataEntry<'pdg>,
}

impl<'pdg> RelatedDataEntry<'pdg> {
    fn from_row(db: &'pdg Pdg, row: &Row<'_>) -> rusqlite::Result<Self> {
        let data = DataEntry::from_row(db, row)?;
        Ok(Self {
            pdgid: data.pdgid.clone(),
            description: row.get(DataEntry::COLUMN_COUNT)?,
            mode_number: row.get::<_, Option<u32>>(DataEntry::COLUMN_COUNT + 1)?,
            data_type: row.get(DataEntry::COLUMN_COUNT + 2)?,
            value: data,
        })
    }

    /// Loads measurements supporting this related data row.
    ///
    /// # Errors
    ///
    /// Returns a database error if the measurement query fails.
    pub fn measurements(&self) -> PdgResult<Vec<PdgMeasurement>> {
        self.value.measurements()
    }

    /// Loads footnotes attached to this related data row.
    ///
    /// # Errors
    ///
    /// Returns a database error if the footnote query fails.
    pub fn footnotes(&self) -> PdgResult<Vec<PdgFootnote>> {
        self.value.footnotes()
    }

    /// Loads text blocks attached to this related data row.
    ///
    /// # Errors
    ///
    /// Returns a database error if the text query fails.
    pub fn texts(&self) -> PdgResult<Vec<PdgText>> {
        self.value.texts()
    }
}

/// Branching-ratio data for a particle.
#[derive(Debug, Clone)]
pub struct BranchingRatio<'pdg> {
    /// PDG identifier for the branching-ratio row.
    pub pdgid: PdgId,
    /// Human-readable PDG description.
    pub description: String,
    /// PDG mode number, when available.
    pub mode_number: Option<u32>,
    /// Numeric value for the branching ratio.
    pub value: DataEntry<'pdg>,
}

impl BranchingRatio<'_> {
    /// Loads measurements supporting this branching ratio.
    ///
    /// # Errors
    ///
    /// Returns a database error if the measurement query fails.
    pub fn measurements(&self) -> PdgResult<Vec<PdgMeasurement>> {
        self.value.measurements()
    }

    /// Loads footnotes attached to this branching ratio.
    ///
    /// # Errors
    ///
    /// Returns a database error if the footnote query fails.
    pub fn footnotes(&self) -> PdgResult<Vec<PdgFootnote>> {
        self.value.footnotes()
    }

    /// Loads text blocks attached to this branching ratio.
    ///
    /// # Errors
    ///
    /// Returns a database error if the text query fails.
    pub fn texts(&self) -> PdgResult<Vec<PdgText>> {
        self.value.texts()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AngularMomentum, BranchingFractionKind, Charge, DataType, DecayStateExpansion, Isospin,
        LimitType, Parity, ParticleClass, ParticleSearchQuery, ParticleType, Pdg, PdgItemType,
        PropertySource,
    };

    fn test_pdg() -> Pdg {
        Pdg::open_path(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/pdgall-2025-v0.2.2.sqlite"
        ))
        .unwrap()
    }

    #[test]
    fn displays_particle_identity_and_quantum_numbers() {
        let db = test_pdg();
        let pion = db.particle("pi+").unwrap().unwrap();

        assert_eq!(
            pion.to_string(),
            "pi+ (S008, Meson, Particle, charge +1), MCID 211, I=1, G=-, J=0, P=-"
        );
    }

    #[test]
    fn displays_self_conjugate_particles() {
        let db = test_pdg();
        let photon = db.particle("gamma").unwrap().unwrap();

        assert_eq!(
            photon.to_string(),
            "gamma (S000, Gauge/Higgs Boson, Self-Conjugate, charge 0), MCID 22, I=0 or 1, J=1, P=-, C=-"
        );
    }

    #[test]
    fn displays_fractional_charges() {
        let db = test_pdg();

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
        let db = test_pdg();

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
        let db = test_pdg();

        let pion = db.particle("pi+").unwrap().unwrap();
        assert!(pion.particle_class == ParticleClass::Meson);
        assert!(db.particle("p").unwrap().unwrap().particle_class == ParticleClass::Baryon);
        assert!(db.particle("e-").unwrap().unwrap().particle_class == ParticleClass::Lepton);
        assert!(db.particle("d").unwrap().unwrap().particle_class == ParticleClass::Quark);
        assert!(db.particle("gamma").unwrap().unwrap().particle_class == ParticleClass::GaugeBoson);
    }

    #[test]
    fn queries_particles_by_class() {
        let db = test_pdg();
        let leptons = db
            .search_particles(ParticleSearchQuery::new().class(ParticleClass::Lepton))
            .unwrap();

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
        let db = test_pdg();
        let pion_mesons = db
            .search_particles(
                ParticleSearchQuery::new()
                    .name_contains("pi")
                    .class(ParticleClass::Meson),
            )
            .unwrap();
        let pion_baryons = db
            .search_particles(
                ParticleSearchQuery::new()
                    .name_contains("pi")
                    .class(ParticleClass::Baryon),
            )
            .unwrap();

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
    fn searches_by_class_and_angular_momentum() {
        let db = test_pdg();
        let vector_mesons = db
            .search_particles(
                ParticleSearchQuery::new()
                    .class(ParticleClass::Meson)
                    .angular_momentum(AngularMomentum::J2),
            )
            .unwrap();

        assert!(!vector_mesons.is_empty());
        assert!(vector_mesons.iter().all(|particle| {
            particle.particle_class == ParticleClass::Meson
                && particle.quantum_j == Some(AngularMomentum::J2)
        }));
    }

    #[test]
    fn searches_by_particle_type_charge_and_quantum_numbers() {
        let db = test_pdg();
        let scalar_neutral_mesons = db
            .search_particles(
                ParticleSearchQuery::new()
                    .class(ParticleClass::Meson)
                    .particle_type(ParticleType::SelfConjugate)
                    .charge(Charge::Neutral)
                    .isospin(Isospin::I0)
                    .g_parity(Parity::Plus)
                    .angular_momentum(AngularMomentum::J0)
                    .parity(Parity::Plus)
                    .charge_conjugation(Parity::Plus),
            )
            .unwrap();

        assert!(
            scalar_neutral_mesons
                .iter()
                .any(|particle| particle.name == "f_0(980)0")
        );
        assert!(scalar_neutral_mesons.iter().all(|particle| {
            particle.particle_class == ParticleClass::Meson
                && particle.particle_type == ParticleType::SelfConjugate
                && particle.charge == Charge::Neutral
                && particle.quantum_i == Some(Isospin::I0)
                && particle.quantum_g == Some(Parity::Plus)
                && particle.quantum_j == Some(AngularMomentum::J0)
                && particle.quantum_p == Some(Parity::Plus)
                && particle.quantum_c == Some(Parity::Plus)
        }));
    }

    #[test]
    fn searches_for_missing_optional_quantum_numbers() {
        let db = test_pdg();
        let particles = db
            .search_particles(
                ParticleSearchQuery::new()
                    .name_contains("p")
                    .g_parity(None)
                    .charge_conjugation(None),
            )
            .unwrap();

        assert!(particles.iter().any(|particle| particle.name == "p"));
        assert!(
            particles
                .iter()
                .all(|particle| particle.quantum_g.is_none() && particle.quantum_c.is_none())
        );
    }

    #[test]
    fn searches_by_normalized_mass_range() {
        let db = test_pdg();
        let light_mesons = db
            .search_particles(
                ParticleSearchQuery::new()
                    .class(ParticleClass::Meson)
                    .mass_range_mev(100.0, 150.0),
            )
            .unwrap();

        assert!(light_mesons.iter().any(|particle| particle.name == "pi+"));
        assert!(light_mesons.iter().any(|particle| particle.name == "pi-"));
        assert!(light_mesons.iter().any(|particle| particle.name == "pi0"));
    }

    #[test]
    fn range_searches_use_section_derived_properties() {
        let db = test_pdg();
        let particles = db
            .search_particles(
                ParticleSearchQuery::new()
                    .name_contains("a_0(")
                    .mass_range_mev(1500.0, 2000.0),
            )
            .unwrap();

        assert!(
            particles
                .iter()
                .any(|particle| particle.name == "a_0(1710)0")
        );
        assert!(
            !particles
                .iter()
                .any(|particle| particle.name == "a_0(980)0")
        );
    }

    #[test]
    fn searches_by_ambiguous_width_range() {
        let db = test_pdg();
        let particles = db
            .search_particles(ParticleSearchQuery::new().width_range_mev(0.0, 50.0))
            .unwrap();

        assert!(
            particles
                .iter()
                .any(|particle| particle.name == "f_0(980)0")
        );
        assert!(
            !particles
                .iter()
                .any(|particle| particle.name == "D_1(2430)0")
        );
        assert!(particles.iter().any(|particle| particle.name == "e-"));
    }

    #[test]
    fn searches_by_lifetime_range() {
        let db = test_pdg();
        let particles = db
            .search_particles(ParticleSearchQuery::new().lifetime_range_seconds(1e-8, 1e-7))
            .unwrap();

        assert!(particles.iter().any(|particle| particle.name == "pi+"));
        assert!(!particles.iter().any(|particle| particle.name == "p"));
    }

    #[test]
    fn searches_by_decay_final_states() {
        let db = test_pdg();
        let sigma_modes = db
            .search_particles(
                ParticleSearchQuery::new()
                    .class(ParticleClass::Baryon)
                    .decays_to(["p", "K-"])
                    .mass_range_mev(0.0, 2000.0),
            )
            .unwrap();

        assert!(
            sigma_modes
                .iter()
                .any(|particle| particle.name == "Lambda(1520)0")
        );
        assert!(
            sigma_modes
                .iter()
                .any(|particle| particle.name == "Sigma(1385)0")
        );
        assert!(
            !sigma_modes
                .iter()
                .any(|particle| particle.name == "Xi_b()-")
        );
    }

    #[test]
    fn decay_contains_allows_extra_final_states() {
        let db = test_pdg();
        let particles = db
            .search_particles(
                ParticleSearchQuery::new()
                    .class(ParticleClass::Baryon)
                    .decay_contains(["p", "K-"]),
            )
            .unwrap();

        assert!(particles.iter().any(|particle| particle.name == "Xi_b()-"));
    }

    #[test]
    fn literal_exact_decay_search_does_not_expand_state_names() {
        let db = test_pdg();
        let particles = db
            .search_particles(
                ParticleSearchQuery::new()
                    .class(ParticleClass::Baryon)
                    .decays_to(["p", "K-"])
                    .decay_state_expansion(DecayStateExpansion::Literal),
            )
            .unwrap();

        assert!(
            !particles
                .iter()
                .any(|particle| particle.name == "Lambda(1520)0")
        );
    }

    #[test]
    fn searches_by_decay_initial_and_final_states() {
        let db = test_pdg();
        let pion_modes = db
            .search_particles(
                ParticleSearchQuery::new()
                    .decays_from(["pi+"])
                    .decays_to(["mu+", "nu_mu"]),
            )
            .unwrap();

        assert!(pion_modes.iter().any(|particle| particle.name == "pi+"));
    }

    #[test]
    fn searches_decay_states_using_item_expansion() {
        let db = test_pdg();
        let kaon_modes = db
            .search_particles(
                ParticleSearchQuery::new()
                    .decays_from(["K+"])
                    .decay_contains(["pi"]),
            )
            .unwrap();

        assert!(kaon_modes.iter().any(|particle| particle.name == "K+"));
    }

    #[test]
    fn neutral_kaon_exact_decay_search_includes_kaon_family_modes() {
        let db = test_pdg();
        let particles = db
            .search_particles(
                ParticleSearchQuery::new()
                    .class(ParticleClass::Meson)
                    .decays_to(["K(S)0", "K(S)0"]),
            )
            .unwrap();

        assert!(
            particles
                .iter()
                .any(|particle| particle.name == "f_0(980)0")
        );
        assert!(
            particles
                .iter()
                .any(|particle| particle.name == "a_0(980)0")
        );
        assert!(
            particles
                .iter()
                .any(|particle| particle.name == "f_2^'(1525)0")
        );
        assert!(
            particles
                .iter()
                .any(|particle| particle.name == "a_2(1320)0")
        );
        assert!(
            particles
                .iter()
                .any(|particle| particle.name == "a_0(1710)0")
        );
        assert!(
            particles
                .iter()
                .any(|particle| particle.name == "f_2(1910)0")
        );
    }

    #[test]
    fn literal_neutral_kaon_exact_decay_search_does_not_expand_family_modes() {
        let db = test_pdg();
        let particles = db
            .search_particles(
                ParticleSearchQuery::new()
                    .class(ParticleClass::Meson)
                    .decays_to(["K(S)0", "K(S)0"])
                    .decay_state_expansion(DecayStateExpansion::Literal),
            )
            .unwrap();

        assert!(
            !particles
                .iter()
                .any(|particle| particle.name == "f_0(980)0")
        );
        assert!(
            !particles
                .iter()
                .any(|particle| particle.name == "a_0(980)0")
        );
    }

    #[test]
    fn loads_items_by_name() {
        let db = test_pdg();
        let pion_pair = db.item("pi+-").unwrap().unwrap();

        assert_eq!(pion_pair.name, "pi+-");
        assert_eq!(pion_pair.item_type, PdgItemType::ChargeMultiplet);
        assert!(db.item("NO_SUCH_ITEM").unwrap().is_none());
    }

    #[test]
    fn loads_item_children_with_particles() {
        let db = test_pdg();
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
    fn item_exposes_own_navigation() {
        let db = test_pdg();
        let pion = db.item("pi+").unwrap().unwrap();
        let kaon_group = db.item("K").unwrap().unwrap();

        assert_eq!(pion.particle().unwrap().unwrap().name, "pi+");
        assert!(kaon_group.particle().unwrap().is_none());
        assert!(
            pion.parents()
                .unwrap()
                .iter()
                .any(|item| item.name == "pi" && item.item_type == PdgItemType::Group)
        );
        assert!(
            kaon_group
                .children()
                .unwrap()
                .iter()
                .any(|child| child.item.name == "K(S)0")
        );
        assert!(
            kaon_group
                .related_particles()
                .unwrap()
                .iter()
                .any(|particle| particle.name == "K+")
        );
    }

    #[test]
    fn particle_exposes_item_context() {
        let db = test_pdg();
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
        let db = test_pdg();
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
        let db = test_pdg();
        let texts = db.texts_for("S008M").unwrap();

        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].pdgid, "S008M");
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
        let db = test_pdg();
        let footnotes = db.footnotes_for("S008M").unwrap();

        assert!(footnotes.len() >= 10);
        assert_eq!(footnotes[0].pdgid.as_deref(), Some("S008M"));
        assert_eq!(footnotes[0].index, Some(1));
        assert!(footnotes[0].text.as_ref().unwrap().contains("DAUM 2019"));
    }

    #[test]
    fn particle_forwards_text_and_footnote_lookups() {
        let db = test_pdg();
        let pion = db.particle("pi+").unwrap().unwrap();

        assert!(pion.texts().unwrap().is_empty());
        assert!(pion.footnotes().unwrap().is_empty());
    }

    #[test]
    fn loads_measurements_for_data_entries() {
        let db = test_pdg();
        let measurements = db.measurements_for("S008M").unwrap();
        let first = measurements.first().unwrap();
        let first_value = first.values.first().unwrap();
        let first_footnote = first.footnotes.first().unwrap();

        assert_eq!(first.pdgid, "S008M");
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
        assert_eq!(first_footnote.pdgid.as_deref(), Some("S008M"));
        assert_eq!(first_footnote.index, Some(1));
        assert!(first_footnote.text.as_ref().unwrap().contains("DAUM 2019"));
        assert!(!first_footnote.changebar);
    }

    #[test]
    fn particle_loads_measurements_for_data_type() {
        let db = test_pdg();
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
        let db = test_pdg();

        assert!(db.measurements_for("NO_SUCH_PDGID").unwrap().is_empty());
    }

    #[test]
    fn lifetime_uses_lifetime_data_type() {
        let db = test_pdg();
        let pion = db.particle("pi+").unwrap().unwrap();

        let mass = pion.mass().unwrap().unwrap();
        let lifetime = pion.lifetime().unwrap().unwrap();

        assert!(mass.value.unwrap() > 100.0);
        assert!(lifetime.value.unwrap() < 0.000_001);
    }

    #[test]
    fn width_uses_full_width_data_type() {
        let db = test_pdg();
        let z_boson = db.particle("Z0").unwrap().unwrap();

        let width = z_boson.width().unwrap().unwrap();

        assert!(width.value.unwrap() > 2.0);
        assert_eq!(width.unit_text, "GeV");
        assert_eq!(width.pdgid, "S044W");
    }

    #[test]
    fn properties_fall_back_to_section_children() {
        let db = test_pdg();
        let a0 = db.particle("a_0(980)0").unwrap().unwrap();

        let mass = a0.mass().unwrap().unwrap();
        let width = a0.width().unwrap().unwrap();
        let mass_property = a0.property(DataType::Mass).unwrap().unwrap();

        assert_eq!(mass.pdgid, "M036MX");
        assert_eq!(mass.to_string(), "980+-20 MeV");
        assert_eq!(width.pdgid, "M036W1");
        assert_eq!(width.to_string(), "50 to 100 MeV");
        assert!(matches!(
            mass_property.source,
            PropertySource::Section { ref section_pdgid } if section_pdgid == "M036205"
        ));
    }

    #[test]
    fn exclusive_branching_fractions_include_decay_products() {
        let db = test_pdg();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_fractions = pion.exclusive_branching_fractions().unwrap();
        let muon_mode = branching_fractions
            .iter()
            .find(|branching_fraction| branching_fraction.pdgid == "S008.1")
            .unwrap();

        assert_eq!(muon_mode.description, "pi+ --> mu+ nu_mu");
        assert_eq!(muon_mode.mode_number, Some(1));
        assert_eq!(muon_mode.kind, BranchingFractionKind::Exclusive);
        assert_eq!(muon_mode.products.len(), 3);
        assert_eq!(muon_mode.products[0].name, "pi+");
        assert!(!muon_mode.products[0].is_outgoing);
        assert_eq!(muon_mode.products[1].name, "mu+");
        assert!(muon_mode.products[1].is_outgoing);

        let muon_product = &muon_mode.products[1];
        assert_eq!(muon_product.item().unwrap().unwrap().name, "mu+");
        assert_eq!(muon_product.particle().unwrap().unwrap().name, "mu+");
        assert!(
            muon_product
                .parents()
                .unwrap()
                .iter()
                .any(|item| item.name == "mu")
        );
        assert_eq!(
            muon_mode.measurements().unwrap().len(),
            db.measurements_for(muon_mode.pdgid.clone()).unwrap().len()
        );
        assert_eq!(
            muon_mode.footnotes().unwrap().len(),
            db.footnotes_for(muon_mode.pdgid.clone()).unwrap().len()
        );
        assert_eq!(
            muon_mode.texts().unwrap().len(),
            db.texts_for(muon_mode.pdgid.clone()).unwrap().len()
        );
    }

    #[test]
    fn branching_fractions_include_related_ratios() {
        let db = test_pdg();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_fractions = pion.exclusive_branching_fractions().unwrap();
        let muon_mode = branching_fractions
            .iter()
            .find(|branching_fraction| branching_fraction.pdgid == "S008.1")
            .unwrap();
        let related_ratio = muon_mode
            .related_data
            .iter()
            .find(|related_data| related_data.pdgid == "S008R10")
            .unwrap();

        assert_eq!(related_ratio.data_type, DataType::BranchingRatio);
        assert!(related_ratio.description.contains("G(pi+ --> e+ nu_e)"));
        assert!(related_ratio.value.value.unwrap() > 0.0);
        assert!(!related_ratio.value.display_value_text.is_empty());

        assert_eq!(
            related_ratio.measurements().unwrap().len(),
            db.measurements_for(related_ratio.pdgid.clone())
                .unwrap()
                .len()
        );
        assert!(!related_ratio.footnotes().unwrap().is_empty());
        assert!(!related_ratio.texts().unwrap().is_empty());
    }

    #[test]
    fn branching_fractions_preserve_non_ratio_related_data() {
        let db = test_pdg();
        let kaon = db.particle("K+").unwrap().unwrap();

        let branching_fractions = kaon.exclusive_branching_fractions().unwrap();
        let muon_mode = branching_fractions
            .iter()
            .find(|branching_fraction| branching_fraction.pdgid == "S010.1")
            .unwrap();

        assert!(
            muon_mode
                .related_data
                .iter()
                .any(|related_data| related_data.pdgid == "S010T"
                    && related_data.data_type == DataType::Lifetime
                    && related_data.description == "K+- MEAN LIFE"
                    && related_data.value.value.unwrap() > 0.0)
        );
    }

    #[test]
    fn inclusive_and_exclusive_branching_fractions_are_grouped() {
        let db = test_pdg();
        let b0 = db.particle("B0").unwrap().unwrap();

        let inclusive = b0.inclusive_branching_fractions().unwrap();
        let exclusive = b0.exclusive_branching_fractions().unwrap();
        let all = b0.branching_fractions().unwrap();

        assert!(inclusive.iter().any(|mode| mode.pdgid == "S042.94"));
        assert!(exclusive.iter().any(|mode| mode.pdgid == "S042.30"));
        assert_eq!(all.len(), inclusive.len() + exclusive.len());
        let inclusive_position = all.iter().position(|mode| mode.pdgid == "S042.94").unwrap();
        let exclusive_position = all.iter().position(|mode| mode.pdgid == "S042.30").unwrap();

        assert!(inclusive_position < exclusive_position);
    }

    #[test]
    fn branching_ratios_include_descriptions() {
        let db = test_pdg();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_ratios = pion.branching_ratios().unwrap();
        let ratio = branching_ratios
            .iter()
            .find(|ratio| ratio.pdgid == "S008R2")
            .unwrap();

        assert_eq!(ratio.description, "G(pi+ --> e+ nu_e)/G(total)");
        assert!(ratio.value.value.unwrap() > 0.0);
        assert_eq!(
            ratio.measurements().unwrap().len(),
            db.measurements_for(ratio.pdgid.clone()).unwrap().len()
        );
        assert!(!ratio.texts().unwrap().is_empty());
    }

    #[test]
    fn branching_ratios_include_errors() {
        let db = test_pdg();
        let sigma = db
            .search_particles(ParticleSearchQuery::new().name_contains("Sigma(2010)"))
            .unwrap()
            .remove(0);

        let branching_ratios = sigma.branching_ratios().unwrap();
        let ratio = branching_ratios
            .iter()
            .find(|ratio| ratio.pdgid == "B002R1")
            .unwrap();

        assert_eq!(ratio.value.error_positive, Some(0.03));
        assert_eq!(ratio.value.error_negative, Some(0.03));
    }

    #[test]
    fn mass_includes_confidence_level_and_limit_type() {
        let db = test_pdg();
        let down_quark = db.particle("d").unwrap().unwrap();
        let mass = down_quark.mass().unwrap().unwrap();

        assert_eq!(mass.confidence_level, Some(90.0));

        let n1895 = db.particle("N(1895)0").unwrap().unwrap();
        let mass_limit = n1895.mass().unwrap().unwrap();

        assert_eq!(mass_limit.limit_type, Some(LimitType::Range));
    }

    #[test]
    fn lifetime_includes_confidence_level_and_limit_type() {
        let db = test_pdg();
        let proton = db.particle("p").unwrap().unwrap();
        let lifetime = proton.lifetime().unwrap().unwrap();

        assert_eq!(lifetime.confidence_level, Some(90.0));
        assert_eq!(lifetime.limit_type, Some(LimitType::LowerLimit));
    }

    #[test]
    fn width_preserves_confidence_level_and_limit_type() {
        let db = test_pdg();
        let d_star = db.particle("D^*(2007)0").unwrap().unwrap();
        let width = d_star.width().unwrap().unwrap();

        assert_eq!(width.confidence_level, Some(90.0));
        assert_eq!(width.limit_type, Some(LimitType::UpperLimit));
    }

    #[test]
    fn data_entry_loads_measurements_from_own_pdgid() {
        let db = test_pdg();
        let pion = db.particle("pi+").unwrap().unwrap();
        let mass = pion.mass().unwrap().unwrap();

        let entry_measurements = mass.measurements().unwrap();
        let direct_measurements = db.measurements_for(mass.pdgid).unwrap();

        assert_eq!(entry_measurements.len(), direct_measurements.len());
        assert_eq!(
            entry_measurements[0].reference.document_id,
            direct_measurements[0].reference.document_id
        );
    }

    #[test]
    fn data_entry_displays_database_display_fields() {
        let db = test_pdg();
        let z_boson = db.particle("Z0").unwrap().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();
        let d_star = db.particle("D^*(2007)0").unwrap().unwrap();
        let muon = db.particle("mu+").unwrap().unwrap();

        assert_eq!(
            z_boson.width().unwrap().unwrap().to_string(),
            "2.4955+-0.0023 GeV"
        );
        assert_eq!(
            pion.exclusive_branching_fractions()
                .unwrap()
                .into_iter()
                .find(|mode| mode.pdgid == "S008.1")
                .unwrap()
                .value
                .to_string(),
            "99.98770+-0.00004%"
        );
        assert_eq!(d_star.width().unwrap().unwrap().to_string(), "<2.1 MeV");
        assert_eq!(
            muon.lifetime().unwrap().unwrap().to_string(),
            "2.1969811+-0.0000022E-6 s"
        );
    }

    #[test]
    fn upper_limit_branching_fractions_preserve_limit_type() {
        let db = test_pdg();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_fractions = pion.exclusive_branching_fractions().unwrap();
        let limit_mode = branching_fractions
            .iter()
            .find(|branching_fraction| branching_fraction.pdgid == "S008.10")
            .unwrap();

        assert_eq!(limit_mode.value.limit_type, Some(LimitType::UpperLimit));
        assert_eq!(limit_mode.value.error_positive, Some(0.0));
        assert_eq!(limit_mode.value.error_negative, Some(0.0));
    }
}
