use crate::{AngularMomentum, Charge, Isospin, Parity, ParticleClass, ParticleType};

#[derive(Clone, Debug, Default)]
pub struct ParticleSearchQuery {
    pub(crate) name_contains: Option<String>,
    pub(crate) particle_class: Option<ParticleClass>,
    pub(crate) particle_type: Option<ParticleType>,
    pub(crate) charge: Option<Charge>,
    pub(crate) isospin: QuantumFilter<Isospin>,
    pub(crate) g_parity: QuantumFilter<Parity>,
    pub(crate) angular_momentum: QuantumFilter<AngularMomentum>,
    pub(crate) parity: QuantumFilter<Parity>,
    pub(crate) charge_conjugation: QuantumFilter<Parity>,
    pub(crate) mass_range_mev: Option<(f64, f64)>,
    pub(crate) width_range_mev: Option<(f64, f64)>,
    pub(crate) lifetime_range_seconds: Option<(f64, f64)>,
    pub(crate) decays_to: DecayFilter,
    pub(crate) decays_from: Vec<String>,
    pub(crate) decay_state_expansion: DecayStateExpansion,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DecayFilter {
    pub(crate) states: Vec<String>,
    pub(crate) mode: DecayMatchMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DecayMatchMode {
    #[default]
    Exact,
    Contains,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DecayStateExpansion {
    #[default]
    Inclusive,
    Literal,
}

#[derive(Clone, Debug)]
pub(crate) enum QuantumFilter<T> {
    Any,
    Missing,
    Value(T),
}

impl<T> Default for QuantumFilter<T> {
    fn default() -> Self {
        Self::Any
    }
}

impl ParticleSearchQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name_contains(mut self, value: impl Into<String>) -> Self {
        self.name_contains = Some(value.into());
        self
    }

    pub fn class(mut self, particle_class: ParticleClass) -> Self {
        self.particle_class = Some(particle_class);
        self
    }

    pub fn particle_type(mut self, particle_type: ParticleType) -> Self {
        self.particle_type = Some(particle_type);
        self
    }

    pub fn charge(mut self, charge: Charge) -> Self {
        self.charge = Some(charge);
        self
    }

    pub fn isospin(mut self, isospin: impl Into<Option<Isospin>>) -> Self {
        self.isospin = quantum_filter(isospin);
        self
    }

    pub fn g_parity(mut self, g_parity: impl Into<Option<Parity>>) -> Self {
        self.g_parity = quantum_filter(g_parity);
        self
    }

    pub fn angular_momentum(
        mut self,
        angular_momentum: impl Into<Option<AngularMomentum>>,
    ) -> Self {
        self.angular_momentum = quantum_filter(angular_momentum);
        self
    }

    pub fn parity(mut self, parity: impl Into<Option<Parity>>) -> Self {
        self.parity = quantum_filter(parity);
        self
    }

    pub fn charge_conjugation(mut self, charge_conjugation: impl Into<Option<Parity>>) -> Self {
        self.charge_conjugation = quantum_filter(charge_conjugation);
        self
    }

    pub fn mass_range_mev(mut self, min: f64, max: f64) -> Self {
        self.mass_range_mev = Some((min, max));
        self
    }

    pub fn width_range_mev(mut self, min: f64, max: f64) -> Self {
        self.width_range_mev = Some((min, max));
        self
    }

    pub fn lifetime_range_seconds(mut self, min: f64, max: f64) -> Self {
        self.lifetime_range_seconds = Some((min, max));
        self
    }

    pub fn decays_to<I, S>(mut self, states: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.decays_to = DecayFilter {
            states: states.into_iter().map(Into::into).collect(),
            mode: DecayMatchMode::Exact,
        };
        self
    }

    pub fn decay_contains<I, S>(mut self, states: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.decays_to = DecayFilter {
            states: states.into_iter().map(Into::into).collect(),
            mode: DecayMatchMode::Contains,
        };
        self
    }

    pub fn decays_from<I, S>(mut self, states: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.decays_from = states.into_iter().map(Into::into).collect();
        self
    }

    pub fn decay_state_expansion(mut self, expansion: DecayStateExpansion) -> Self {
        self.decay_state_expansion = expansion;
        self
    }
}

fn quantum_filter<T>(filter: impl Into<Option<T>>) -> QuantumFilter<T> {
    match filter.into() {
        Some(value) => QuantumFilter::Value(value),
        None => QuantumFilter::Missing,
    }
}
