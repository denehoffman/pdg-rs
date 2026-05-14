use std::{fmt::Display, str::FromStr};

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};

use crate::PdgError;

#[derive(Debug, Copy, Clone)]
pub enum ValueType {
    WeightedAverage,
    BestLimit,
    BranchingRatio,
    PdgEvaluation,
    PdgLimit,
    ExtraBelow,
    ExtraAbove,
    FittedData,
    FittedDecayRate,
    Estimate,
    DefaultEvaluation,
    Internal,
}

impl ValueType {
    pub fn to_code(&self) -> &'static str {
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
                ValueType::from_str(s).map_err(|err| FromSqlError::Other(Box::new(err)))
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LimitType {
    UpperLimit,
    LowerLimit,
    Range,
    RangeExclusion,
}

impl Display for LimitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::UpperLimit => "U",
                Self::LowerLimit => "L",
                Self::Range => "R",
                Self::RangeExclusion => "X",
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
                LimitType::from_str(s).map_err(|err| FromSqlError::Other(Box::new(err)))
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataType {
    ElectricDipoleMoment,
    MagneticMoment,
    CPViolationParameter,
    MassDifference,
    FormFactor,
    MeanLifetime,
    SlopeParameter,
    FullWidth,
    Mass,
    CouplingConstantRatio,
    DecayParameter,
    Lifetime,
    ExclusiveBranchingFraction,
    ExclusiveBranchingFraction1,
    ExclusiveBranchingFraction2,
    ExclusiveBranchingFraction3,
    ExclusiveBranchingFraction4,
    ExclusiveBranchingFraction5,
    InclusiveBranchingFraction,
    InclusiveBranchingFraction1,
    InclusiveBranchingFraction2,
    InclusiveBranchingFraction3,
    InclusiveBranchingFraction4,
    InclusiveBranchingFraction5,
    BranchingRatio,
    Particle,
    Searches,
    Section,
    Other,
}

impl FromSql for DataType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s =
                    std::str::from_utf8(bytes).map_err(|err| FromSqlError::Other(Box::new(err)))?;
                DataType::from_str(s).map_err(|err| FromSqlError::Other(Box::new(err)))
            }
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

impl DataType {
    pub fn to_code(&self) -> &'static str {
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

    pub fn is_branching_fraction(&self) -> bool {
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

    pub fn is_particle_property(&self) -> bool {
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
