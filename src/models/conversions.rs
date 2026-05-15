#[cfg(feature = "ratio")]
use std::{any::type_name, fmt::Display};

use thiserror::Error;

use super::pdgparticle::{AngularMomentum, Charge, Isospin, Parity};

/// Error returned when a quantum number cannot be converted to a numeric type.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum QuantumNumberConversionError {
    /// The source value represents more than one possible numeric value.
    #[error("{kind} has no single numeric value")]
    Ambiguous {
        /// Kind of quantum number being converted.
        kind: &'static str,
    },
    /// The source value is explicitly unknown.
    #[error("{kind} is unknown")]
    Unknown {
        /// Kind of quantum number being converted.
        kind: &'static str,
    },
    /// The source value is a custom string that cannot be parsed as a number.
    #[error("{kind} has custom value {value:?}")]
    Custom {
        /// Kind of quantum number being converted.
        kind: &'static str,
        /// Custom value that could not be converted.
        value: String,
    },
    /// The rational value cannot be represented by the requested target type.
    #[error("{kind} value {numerator}/{denominator} cannot be represented as {target}")]
    OutOfRange {
        /// Kind of quantum number being converted.
        kind: &'static str,
        /// Rational numerator before conversion.
        numerator: i32,
        /// Rational denominator before conversion.
        denominator: i32,
        /// Target numeric type name.
        target: &'static str,
    },
}

trait RationalParts {
    fn kind(&self) -> &'static str;
    fn rational_parts(&self) -> Result<(i32, i32), QuantumNumberConversionError>;
}

impl RationalParts for Charge {
    fn kind(&self) -> &'static str {
        "charge"
    }

    fn rational_parts(&self) -> Result<(i32, i32), QuantumNumberConversionError> {
        Ok(match self {
            Self::PlusPlus => (2, 1),
            Self::Plus => (1, 1),
            Self::Neutral => (0, 1),
            Self::Minus => (-1, 1),
            Self::MinusMinus => (-2, 1),
            Self::PlusOneThird => (1, 3),
            Self::PlusTwoThirds => (2, 3),
            Self::MinusOneThird => (-1, 3),
            Self::MinusTwoThirds => (-2, 3),
        })
    }
}

impl RationalParts for Isospin {
    fn kind(&self) -> &'static str {
        "isospin"
    }

    fn rational_parts(&self) -> Result<(i32, i32), QuantumNumberConversionError> {
        match self {
            Self::I0 => Ok((0, 1)),
            Self::I1 => Ok((1, 2)),
            Self::I2 => Ok((1, 1)),
            Self::I3 => Ok((3, 2)),
            Self::Photon => Err(QuantumNumberConversionError::Ambiguous { kind: self.kind() }),
            Self::Unknown => Err(QuantumNumberConversionError::Unknown { kind: self.kind() }),
        }
    }
}

impl RationalParts for AngularMomentum {
    fn kind(&self) -> &'static str {
        "angular momentum"
    }

    fn rational_parts(&self) -> Result<(i32, i32), QuantumNumberConversionError> {
        Ok(match self {
            Self::J0 => (0, 1),
            Self::J1 => (1, 2),
            Self::J2 => (1, 1),
            Self::J3 => (3, 2),
            Self::J4 => (2, 1),
            Self::J5 => (5, 2),
            Self::J6 => (3, 1),
            Self::J7 => (7, 2),
            Self::J8 => (4, 1),
            Self::J9 => (9, 2),
            Self::J10 => (5, 1),
            Self::J11 => (11, 2),
            Self::J12 => (6, 1),
            Self::J13 => (13, 2),
            Self::J14 => (7, 1),
            Self::J15 => (15, 2),
            Self::Custom(value) => {
                return Err(QuantumNumberConversionError::Custom {
                    kind: self.kind(),
                    value: value.clone(),
                });
            }
            Self::Unknown => {
                return Err(QuantumNumberConversionError::Unknown { kind: self.kind() });
            }
        })
    }
}

impl RationalParts for Parity {
    fn kind(&self) -> &'static str {
        "parity"
    }

    fn rational_parts(&self) -> Result<(i32, i32), QuantumNumberConversionError> {
        match self {
            Self::Plus => Ok((1, 1)),
            Self::Minus => Ok((-1, 1)),
            Self::Unknown => Err(QuantumNumberConversionError::Unknown { kind: self.kind() }),
        }
    }
}

fn to_f64<T: RationalParts>(value: &T) -> Result<f64, QuantumNumberConversionError> {
    let (numerator, denominator) = value.rational_parts()?;
    Ok(f64::from(numerator) / f64::from(denominator))
}

macro_rules! impl_f64_conversion {
    ($source:ty) => {
        impl TryFrom<$source> for f64 {
            type Error = QuantumNumberConversionError;

            fn try_from(value: $source) -> Result<Self, Self::Error> {
                to_f64(&value)
            }
        }

        impl TryFrom<&$source> for f64 {
            type Error = QuantumNumberConversionError;

            fn try_from(value: &$source) -> Result<Self, Self::Error> {
                to_f64(value)
            }
        }
    };
}

impl_f64_conversion!(Charge);
impl_f64_conversion!(Isospin);
impl_f64_conversion!(AngularMomentum);
impl_f64_conversion!(Parity);

#[cfg(feature = "ratio")]
fn to_ratio<T, V>(value: &V) -> Result<num::rational::Ratio<T>, QuantumNumberConversionError>
where
    T: num::Integer + num::traits::NumCast + Clone + Display,
    V: RationalParts,
{
    let (raw_numerator, raw_denominator) = value.rational_parts()?;
    let numerator =
        T::from(raw_numerator).ok_or_else(|| QuantumNumberConversionError::OutOfRange {
            kind: value.kind(),
            numerator: raw_numerator,
            denominator: raw_denominator,
            target: type_name::<T>(),
        })?;
    let denominator =
        T::from(raw_denominator).ok_or_else(|| QuantumNumberConversionError::OutOfRange {
            kind: value.kind(),
            numerator: raw_numerator,
            denominator: raw_denominator,
            target: type_name::<T>(),
        })?;
    Ok(num::rational::Ratio::new(numerator, denominator))
}

#[cfg(feature = "ratio")]
macro_rules! impl_ratio_conversion {
    ($source:ty) => {
        impl<T> TryFrom<$source> for num::rational::Ratio<T>
        where
            T: num::Integer + num::traits::NumCast + Clone + Display,
        {
            type Error = QuantumNumberConversionError;

            fn try_from(value: $source) -> Result<Self, Self::Error> {
                to_ratio(&value)
            }
        }

        impl<T> TryFrom<&$source> for num::rational::Ratio<T>
        where
            T: num::Integer + num::traits::NumCast + Clone + Display,
        {
            type Error = QuantumNumberConversionError;

            fn try_from(value: &$source) -> Result<Self, Self::Error> {
                to_ratio(value)
            }
        }
    };
}

#[cfg(feature = "ratio")]
impl_ratio_conversion!(Charge);
#[cfg(feature = "ratio")]
impl_ratio_conversion!(Isospin);
#[cfg(feature = "ratio")]
impl_ratio_conversion!(AngularMomentum);
#[cfg(feature = "ratio")]
impl_ratio_conversion!(Parity);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)]
    fn converts_quantum_numbers_to_f64() {
        assert_eq!(f64::try_from(Charge::MinusOneThird).unwrap(), -1.0 / 3.0);
        assert_eq!(f64::try_from(Isospin::I3).unwrap(), 1.5);
        assert_eq!(f64::try_from(AngularMomentum::J5).unwrap(), 2.5);
        assert_eq!(f64::try_from(Parity::Minus).unwrap(), -1.0);
    }

    #[test]
    fn rejects_unknown_ambiguous_and_custom_values() {
        assert_eq!(
            f64::try_from(Isospin::Photon).unwrap_err(),
            QuantumNumberConversionError::Ambiguous { kind: "isospin" }
        );
        assert_eq!(
            f64::try_from(Parity::Unknown).unwrap_err(),
            QuantumNumberConversionError::Unknown { kind: "parity" }
        );
        assert_eq!(
            f64::try_from(AngularMomentum::Custom("1 or 2".to_string())).unwrap_err(),
            QuantumNumberConversionError::Custom {
                kind: "angular momentum",
                value: "1 or 2".to_string()
            }
        );
    }

    #[cfg(feature = "ratio")]
    #[test]
    fn converts_quantum_numbers_to_ratios() {
        let charge: num::rational::Ratio<i8> = Charge::MinusOneThird.try_into().unwrap();
        let spin: num::rational::Ratio<u8> = AngularMomentum::J5.try_into().unwrap();
        let isospin: num::rational::Ratio<usize> = Isospin::I3.try_into().unwrap();

        assert_eq!(charge, num::rational::Ratio::new(-1, 3));
        assert_eq!(spin, num::rational::Ratio::new(5, 2));
        assert_eq!(isospin, num::rational::Ratio::new(3, 2));
    }

    #[cfg(feature = "ratio")]
    #[test]
    fn rejects_negative_values_for_unsigned_ratios() {
        let error = num::rational::Ratio::<u8>::try_from(Charge::Minus).unwrap_err();

        assert_eq!(
            error,
            QuantumNumberConversionError::OutOfRange {
                kind: "charge",
                numerator: -1,
                denominator: 1,
                target: "u8"
            }
        );
    }
}
