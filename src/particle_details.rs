use std::fmt::Display;

use serde::{Serialize, Serializer};

use crate::{
    AngularMomentum, Charge, DataEntry, DataType, Isospin, LimitType, Parity, ParticleClass,
    ParticleProperty, ParticleType, PdgId, PdgMeasurement, PdgParticle, PdgResult, PropertySource,
    ValueType,
};

/// A particle's identity and quantum numbers, without a database borrow.
#[derive(Clone, Debug, Serialize)]
pub struct ParticleSummary {
    /// PDG identifier, which can be shared by multiple particle variants.
    pub pdgid: PdgId,
    /// Canonical PDG particle name.
    pub name: String,
    /// PDG particle description.
    pub description: String,
    /// Particle/antiparticle relationship.
    #[serde(serialize_with = "serialize_display")]
    pub particle_type: ParticleType,
    /// Broad particle class.
    #[serde(serialize_with = "serialize_display")]
    pub particle_class: ParticleClass,
    /// Signed Monte Carlo particle ID, when assigned.
    pub mcid: Option<isize>,
    /// Electric charge.
    #[serde(serialize_with = "serialize_display")]
    pub charge: Charge,
    /// Isospin quantum number.
    #[serde(serialize_with = "serialize_optional_display")]
    pub isospin: Option<Isospin>,
    /// G-parity quantum number.
    #[serde(serialize_with = "serialize_optional_display")]
    pub g_parity: Option<Parity>,
    /// Angular momentum quantum number.
    #[serde(serialize_with = "serialize_optional_display")]
    pub spin: Option<AngularMomentum>,
    /// Parity quantum number.
    #[serde(serialize_with = "serialize_optional_display")]
    pub parity: Option<Parity>,
    /// Charge-conjugation parity quantum number.
    #[serde(serialize_with = "serialize_optional_display")]
    pub c_parity: Option<Parity>,
}

fn serialize_display<T: Display, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

#[expect(clippy::ref_option, reason = "Serde passes a reference to the field")]
fn serialize_optional_display<T: Display, S: Serializer>(
    value: &Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    value
        .as_ref()
        .map(ToString::to_string)
        .serialize(serializer)
}

impl From<&PdgParticle<'_>> for ParticleSummary {
    fn from(particle: &PdgParticle<'_>) -> Self {
        Self {
            pdgid: particle.pdgid.clone(),
            name: particle.name.clone(),
            description: particle.description.clone(),
            particle_type: particle.particle_type,
            particle_class: particle.particle_class,
            mcid: particle.mcid,
            charge: particle.charge,
            isospin: particle.quantum_i,
            g_parity: particle.quantum_g,
            spin: particle.quantum_j.clone(),
            parity: particle.quantum_p,
            c_parity: particle.quantum_c,
        }
    }
}

/// Selects the property data loaded by [`PdgParticle::details`].
#[derive(Clone, Copy, Debug, Default)]
pub enum ParticleDetailSelection<'a> {
    /// Mass, lifetime, width, and other properties listed in child sections.
    #[default]
    Overview,
    /// Values for the requested property kinds, without supporting measurements.
    Properties(&'a [DataType]),
    /// Values and supporting measurements for the requested property kinds.
    Measurements(&'a [DataType]),
}

/// Owned particle details for a selected set of properties and a numeric edition.
#[derive(Clone, Debug, Serialize)]
pub struct ParticleDetails {
    /// Particle identity and quantum numbers.
    pub particle: ParticleSummary,
    /// The selected numeric edition.
    pub edition: String,
    /// Available properties in the selection, with each kind included once.
    pub properties: Vec<ParticlePropertyDetail>,
}

impl ParticleDetails {
    /// Returns a property from the loaded details, if present.
    #[must_use]
    pub fn property(&self, data_type: DataType) -> Option<&ParticlePropertyDetail> {
        self.properties
            .iter()
            .find(|property| property.data_type == data_type)
    }
}

/// An owned particle property with its source and optional supporting measurements.
#[derive(Clone, Debug, Serialize)]
pub struct ParticlePropertyDetail {
    /// Kind of property.
    pub data_type: DataType,
    /// Value and its PDG identifier.
    pub value: DataValue,
    /// Where the value was found in the particle listing.
    pub source: PropertySource,
    /// Supporting measurements and references, present when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurements: Option<Vec<PdgMeasurement>>,
}

/// A PDG numeric or textual value that can be stored independently of the database.
#[derive(Clone, Debug, Serialize)]
pub struct DataValue {
    /// PDG identifier for the value.
    pub pdgid: PdgId,
    /// PDG edition for the value.
    pub edition: String,
    /// Classification of the value.
    pub value_type: ValueType,
    /// Parsed central value, when available.
    pub value: Option<f64>,
    /// Raw PDG value text, including ranges and qualitative evaluations.
    pub value_text: Option<String>,
    /// Positive uncertainty.
    pub error_positive: Option<f64>,
    /// Negative uncertainty.
    pub error_negative: Option<f64>,
    /// Unit text from the PDG table.
    pub unit: String,
    /// PDG scale factor applied to the value.
    pub scale_factor: Option<f64>,
    /// Confidence level associated with the value.
    pub confidence_level: Option<f64>,
    /// Limit or range type, when applicable.
    pub limit_type: Option<LimitType>,
    /// Comment attached to the value.
    pub comment: Option<String>,
    /// Whether the value appears in the PDG summary table.
    pub in_summary_table: bool,
    /// Formatted value including any exponent, percentage, or units.
    pub display: String,
}

impl From<DataEntry<'_>> for DataValue {
    fn from(entry: DataEntry<'_>) -> Self {
        let display = entry.to_string();
        Self {
            pdgid: entry.pdgid,
            edition: entry.edition,
            value_type: entry.value_type,
            value: entry.value,
            value_text: entry.value_text,
            error_positive: entry.error_positive,
            error_negative: entry.error_negative,
            unit: entry.unit_text,
            scale_factor: entry.scale_factor,
            confidence_level: entry.confidence_level,
            limit_type: entry.limit_type,
            comment: entry.comment,
            in_summary_table: entry.in_summary_table,
            display,
        }
    }
}

impl PdgParticle<'_> {
    /// Loads owned, serializable details for the requested properties.
    ///
    /// Explicit selections preserve request order, skip unavailable properties, and
    /// ignore duplicates. An empty property selection loads identity only.
    /// Measurements are queried only for [`ParticleDetailSelection::Measurements`].
    /// Property values use the selected numeric edition. Measurements and references
    /// come from the opened database release.
    ///
    /// # Errors
    /// Returns a database error if the selected data cannot be loaded.
    pub fn details(&self, selection: ParticleDetailSelection<'_>) -> PdgResult<ParticleDetails> {
        let properties = match selection {
            ParticleDetailSelection::Overview => self.headline_properties()?,
            ParticleDetailSelection::Properties(kinds)
            | ParticleDetailSelection::Measurements(kinds) => self.selected_properties(kinds)?,
        };
        let load_measurements = matches!(selection, ParticleDetailSelection::Measurements(_));
        let properties = properties
            .into_iter()
            .map(|property| {
                let measurements = load_measurements
                    .then(|| property.value.measurements())
                    .transpose()?;
                Ok(ParticlePropertyDetail {
                    data_type: property.data_type,
                    value: property.value.into(),
                    source: property.source,
                    measurements,
                })
            })
            .collect::<PdgResult<_>>()?;

        Ok(ParticleDetails {
            particle: ParticleSummary::from(self),
            edition: self.db.edition().to_owned(),
            properties,
        })
    }

    fn selected_properties(&self, kinds: &[DataType]) -> PdgResult<Vec<ParticleProperty<'_>>> {
        let mut properties = Vec::new();
        for (index, &kind) in kinds.iter().enumerate() {
            if !kinds[..index].contains(&kind)
                && let Some(property) = self.property(kind)?
            {
                properties.push(property);
            }
        }
        Ok(properties)
    }

    fn headline_properties(&self) -> PdgResult<Vec<ParticleProperty<'_>>> {
        const HEADLINE: [DataType; 3] = [DataType::Mass, DataType::Lifetime, DataType::FullWidth];
        let mut properties = self.selected_properties(&HEADLINE)?;
        for section in self.db.children_for_pdgid(&self.pdgid)? {
            if section.data_type != DataType::Section {
                continue;
            }
            for child in self.db.children_for_pdgid(&section.pdgid)? {
                if child.data_type.is_particle_property()
                    && !HEADLINE.contains(&child.data_type)
                    && !properties
                        .iter()
                        .any(|property| property.data_type == child.data_type)
                    && let Some(property) = self.section_property(&section, &child)?
                {
                    properties.push(property);
                }
            }
        }
        Ok(properties)
    }
}
