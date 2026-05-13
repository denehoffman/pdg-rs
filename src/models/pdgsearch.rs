use crate::{AngularMomentum, ParticleClass};

#[derive(Clone, Debug, Default)]
pub struct ParticleSearch {
    pub(crate) name_contains: Option<String>,
    pub(crate) particle_class: Option<ParticleClass>,
    pub(crate) angular_momentum: Option<AngularMomentum>,
    pub(crate) mass_range_mev: Option<(f64, f64)>,
    pub(crate) decays_to: Vec<String>,
    pub(crate) decays_from: Vec<String>,
}

impl ParticleSearch {
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

    pub fn angular_momentum(mut self, angular_momentum: AngularMomentum) -> Self {
        self.angular_momentum = Some(angular_momentum);
        self
    }

    pub fn mass_range_mev(mut self, min: f64, max: f64) -> Self {
        self.mass_range_mev = Some((min, max));
        self
    }

    pub fn decays_to<I, S>(mut self, states: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.decays_to = states.into_iter().map(Into::into).collect();
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
}
