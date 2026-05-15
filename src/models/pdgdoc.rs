use std::{fmt::Display, str::FromStr};

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};

use crate::PdgError;

/// Classification for a PDG numeric value row.
#[derive(Debug, Copy, Clone)]
pub enum ValueType {
    /// A weighted average value.
    WeightedAverage,
    /// The best limit selected by PDG.
    BestLimit,
    /// A branching ratio value.
    BranchingRatio,
    /// A PDG evaluation.
    PdgEvaluation,
    /// A PDG limit.
    PdgLimit,
    /// Extra material displayed below a value.
    ExtraBelow,
    /// Extra material displayed above a value.
    ExtraAbove,
    /// A fitted data value.
    FittedData,
    /// A fitted decay-rate value.
    FittedDecayRate,
    /// An estimated value.
    Estimate,
    /// A default evaluation value.
    DefaultEvaluation,
    /// An internal PDG value.
    Internal,
}

impl ValueType {
    /// Returns the compact PDG database code for this value type.
    #[must_use]
    pub const fn to_code(&self) -> &'static str {
        match self {
            Self::WeightedAverage => "AC",
            Self::BestLimit => "L",
            Self::BranchingRatio => "D",
            Self::PdgEvaluation => "V",
            Self::PdgLimit => "OL",
            Self::ExtraBelow => "OM",
            Self::ExtraAbove => "ON",
            Self::FittedData => "FC",
            Self::FittedDecayRate => "DR",
            Self::Estimate => "E",
            Self::DefaultEvaluation => "O",
            Self::Internal => "DV",
        }
    }
}

impl Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::WeightedAverage => "Weighted average",
                Self::BestLimit => "Best limit",
                Self::BranchingRatio => "Branching ratio",
                Self::PdgEvaluation => "PDG evaluation",
                Self::PdgLimit => "PDG limit",
                Self::ExtraBelow => "Extra below",
                Self::ExtraAbove => "Extra above",
                Self::FittedData => "Fitted data",
                Self::FittedDecayRate => "Fitted decay rate",
                Self::Estimate => "Estimate",
                Self::DefaultEvaluation => "Default evaluation",
                Self::Internal => "Internal",
            }
        )
    }
}

impl FromStr for ValueType {
    type Err = PdgError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AC" => Ok(Self::WeightedAverage),
            "L" => Ok(Self::BestLimit),
            "D" => Ok(Self::BranchingRatio),
            "V" => Ok(Self::PdgEvaluation),
            "OL" => Ok(Self::PdgLimit),
            "OM" => Ok(Self::ExtraBelow),
            "ON" => Ok(Self::ExtraAbove),
            "FC" => Ok(Self::FittedData),
            "DR" => Ok(Self::FittedDecayRate),
            "E" => Ok(Self::Estimate),
            "O" => Ok(Self::DefaultEvaluation),
            "DV" => Ok(Self::Internal),
            _ => Err(PdgError::ParseValueType(s.to_string())),
        }
    }
}

impl FromSql for ValueType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|err| FromSqlError::Other(Box::new(err)))?;
                Self::from_str(s).map_err(|err| FromSqlError::Other(Box::new(err)))
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

/// Type of bound or range represented by a data value.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LimitType {
    /// An upper limit.
    UpperLimit,
    /// A lower limit.
    LowerLimit,
    /// A closed range.
    Range,
    /// An excluded range.
    RangeExclusion,
}

impl LimitType {
    /// Returns the compact PDG database code for this limit type.
    #[must_use]
    pub const fn to_code(&self) -> &'static str {
        match self {
            Self::UpperLimit => "U",
            Self::LowerLimit => "L",
            Self::Range => "R",
            Self::RangeExclusion => "X",
        }
    }
}

impl Display for LimitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::UpperLimit => "Upper limit",
                Self::LowerLimit => "Lower limit",
                Self::Range => "Range",
                Self::RangeExclusion => "Range exclusion",
            }
        )
    }
}

impl FromStr for LimitType {
    type Err = PdgError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "U" => Ok(Self::UpperLimit),
            "L" => Ok(Self::LowerLimit),
            "R" => Ok(Self::Range),
            "X" => Ok(Self::RangeExclusion),
            _ => Err(PdgError::ParseLimitType(s.to_string())),
        }
    }
}

impl FromSql for LimitType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|err| FromSqlError::Other(Box::new(err)))?;
                Self::from_str(s).map_err(|err| FromSqlError::Other(Box::new(err)))
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

/// Kind of data represented by a PDG identifier row.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataType {
    /// Electric dipole moment data.
    ElectricDipoleMoment,
    /// Magnetic moment data.
    MagneticMoment,
    /// CP-violation parameter data.
    CPViolationParameter,
    /// Mass-difference data.
    MassDifference,
    /// Form-factor data.
    FormFactor,
    /// Mean-lifetime data.
    MeanLifetime,
    /// Slope-parameter data.
    SlopeParameter,
    /// Full-width data.
    FullWidth,
    /// Mass data.
    Mass,
    /// Coupling-constant ratio data.
    CouplingConstantRatio,
    /// Decay-parameter data.
    DecayParameter,
    /// Lifetime data.
    Lifetime,
    /// Exclusive branching-fraction data.
    ExclusiveBranchingFraction,
    /// Exclusive branching-fraction subtype 1.
    ExclusiveBranchingFraction1,
    /// Exclusive branching-fraction subtype 2.
    ExclusiveBranchingFraction2,
    /// Exclusive branching-fraction subtype 3.
    ExclusiveBranchingFraction3,
    /// Exclusive branching-fraction subtype 4.
    ExclusiveBranchingFraction4,
    /// Exclusive branching-fraction subtype 5.
    ExclusiveBranchingFraction5,
    /// Inclusive branching-fraction data.
    InclusiveBranchingFraction,
    /// Inclusive branching-fraction subtype 1.
    InclusiveBranchingFraction1,
    /// Inclusive branching-fraction subtype 2.
    InclusiveBranchingFraction2,
    /// Inclusive branching-fraction subtype 3.
    InclusiveBranchingFraction3,
    /// Inclusive branching-fraction subtype 4.
    InclusiveBranchingFraction4,
    /// Inclusive branching-fraction subtype 5.
    InclusiveBranchingFraction5,
    /// Branching-ratio data.
    BranchingRatio,
    /// Particle identity row.
    Particle,
    /// Search-result or search-summary row.
    Searches,
    /// Section heading row.
    Section,
    /// Unknown or unclassified data.
    Other,
}

impl FromSql for DataType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|err| FromSqlError::Other(Box::new(err)))?;
                Self::from_str(s).map_err(|err| FromSqlError::Other(Box::new(err)))
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

impl DataType {
    /// Returns the compact PDG database code for this data type.
    #[must_use]
    pub const fn to_code(&self) -> &'static str {
        match self {
            Self::ElectricDipoleMoment => "e",
            Self::MagneticMoment => "m",
            Self::CPViolationParameter => "v",
            Self::MassDifference => "D",
            Self::FormFactor => "f",
            Self::MeanLifetime => "g",
            Self::SlopeParameter => "s",
            Self::FullWidth => "G",
            Self::Mass => "M",
            Self::CouplingConstantRatio => "c",
            Self::DecayParameter => "d",
            Self::Lifetime => "T",
            Self::ExclusiveBranchingFraction => "BFX",
            Self::ExclusiveBranchingFraction1 => "BFX1",
            Self::ExclusiveBranchingFraction2 => "BFX2",
            Self::ExclusiveBranchingFraction3 => "BFX3",
            Self::ExclusiveBranchingFraction4 => "BFX4",
            Self::ExclusiveBranchingFraction5 => "BFX5",
            Self::InclusiveBranchingFraction => "BFI",
            Self::InclusiveBranchingFraction1 => "BFI1",
            Self::InclusiveBranchingFraction2 => "BFI2",
            Self::InclusiveBranchingFraction3 => "BFI3",
            Self::InclusiveBranchingFraction4 => "BFI4",
            Self::InclusiveBranchingFraction5 => "BFI5",
            Self::BranchingRatio => "BR",
            Self::Particle => "PART",
            Self::Searches => "SRCH",
            Self::Section => "SEC",
            Self::Other => "",
        }
    }

    /// Returns `true` for inclusive and exclusive branching-fraction data types.
    #[must_use]
    pub const fn is_branching_fraction(&self) -> bool {
        matches!(
            self,
            Self::ExclusiveBranchingFraction
                | Self::ExclusiveBranchingFraction1
                | Self::ExclusiveBranchingFraction2
                | Self::ExclusiveBranchingFraction3
                | Self::ExclusiveBranchingFraction4
                | Self::ExclusiveBranchingFraction5
                | Self::InclusiveBranchingFraction
                | Self::InclusiveBranchingFraction1
                | Self::InclusiveBranchingFraction2
                | Self::InclusiveBranchingFraction3
                | Self::InclusiveBranchingFraction4
                | Self::InclusiveBranchingFraction5
        )
    }

    /// Returns `true` for particle-level properties such as mass, width, and lifetime.
    #[must_use]
    pub const fn is_particle_property(&self) -> bool {
        !matches!(
            self,
            Self::BranchingRatio | Self::Particle | Self::Searches | Self::Section | Self::Other
        ) && !self.is_branching_fraction()
    }
}

impl Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::ElectricDipoleMoment => "Electric dipole moment",
                Self::MagneticMoment => "Magnetic moment",
                Self::CPViolationParameter => "CP violation parameter",
                Self::MassDifference => "Mass difference",
                Self::FormFactor => "Form factor",
                Self::MeanLifetime => "Mean lifetime",
                Self::SlopeParameter => "Slope parameter",
                Self::FullWidth => "Width",
                Self::Mass => "Mass",
                Self::CouplingConstantRatio => "Coupling constant ratio",
                Self::DecayParameter => "Decay parameter",
                Self::Lifetime => "Lifetime",
                Self::ExclusiveBranchingFraction
                | Self::ExclusiveBranchingFraction1
                | Self::ExclusiveBranchingFraction2
                | Self::ExclusiveBranchingFraction3
                | Self::ExclusiveBranchingFraction4
                | Self::ExclusiveBranchingFraction5 => "Exclusive branching fraction",
                Self::InclusiveBranchingFraction
                | Self::InclusiveBranchingFraction1
                | Self::InclusiveBranchingFraction2
                | Self::InclusiveBranchingFraction3
                | Self::InclusiveBranchingFraction4
                | Self::InclusiveBranchingFraction5 => "Inclusive branching fraction",
                Self::BranchingRatio => "Branching ratio",
                Self::Particle => "Particle",
                Self::Searches => "Searches",
                Self::Section => "Section",
                Self::Other => "Other",
            }
        )
    }
}

impl FromStr for DataType {
    type Err = PdgError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "e" => Ok(Self::ElectricDipoleMoment),
            "m" => Ok(Self::MagneticMoment),
            "v" => Ok(Self::CPViolationParameter),
            "D" => Ok(Self::MassDifference),
            "f" => Ok(Self::FormFactor),
            "g" => Ok(Self::MeanLifetime),
            "s" => Ok(Self::SlopeParameter),
            "G" => Ok(Self::FullWidth),
            "M" => Ok(Self::Mass),
            "c" => Ok(Self::CouplingConstantRatio),
            "d" => Ok(Self::DecayParameter),
            "T" => Ok(Self::Lifetime),
            "BFX" => Ok(Self::ExclusiveBranchingFraction),
            "BFX1" => Ok(Self::ExclusiveBranchingFraction1),
            "BFX2" => Ok(Self::ExclusiveBranchingFraction2),
            "BFX3" => Ok(Self::ExclusiveBranchingFraction3),
            "BFX4" => Ok(Self::ExclusiveBranchingFraction4),
            "BFX5" => Ok(Self::ExclusiveBranchingFraction5),
            "BFI" => Ok(Self::InclusiveBranchingFraction),
            "BFI1" => Ok(Self::InclusiveBranchingFraction1),
            "BFI2" => Ok(Self::InclusiveBranchingFraction2),
            "BFI3" => Ok(Self::InclusiveBranchingFraction3),
            "BFI4" => Ok(Self::InclusiveBranchingFraction4),
            "BFI5" => Ok(Self::InclusiveBranchingFraction5),
            "BR" => Ok(Self::BranchingRatio),
            "PART" => Ok(Self::Particle),
            "SRCH" => Ok(Self::Searches),
            "SEC" => Ok(Self::Section),
            "" => Ok(Self::Other),
            _ => Err(PdgError::ParseDataType(s.to_string())),
        }
    }
}
