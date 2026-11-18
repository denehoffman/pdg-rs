use crate::{
    AngularMomentum, Charge, DataType, DecayProduct, Isospin, Parity, ParticleClass,
    ParticleSummary, ParticleType, PdgError, PdgId, PdgParticle, PdgResult,
};

/// A particle returned by a filter search, with the channels that satisfied its decay filters.
#[derive(Debug)]
pub struct ParticleFilterHit<'pdg> {
    /// The matching particle.
    pub particle: PdgParticle<'pdg>,
    /// Matching PDG channels, empty for a search without decay filters.
    pub matching_decays: Vec<DecayChannelMatch<'pdg>>,
}

impl ParticleFilterHit<'_> {
    /// Detaches this hit from the database so it can be stored or serialized.
    #[must_use]
    pub fn into_owned(self) -> OwnedParticleFilterHit {
        OwnedParticleFilterHit {
            particle: ParticleSummary::from(&self.particle),
            matching_decays: self
                .matching_decays
                .iter()
                .map(OwnedDecayChannelMatch::from)
                .collect(),
        }
    }
}

/// A filter search hit that can outlive the database connection.
#[derive(Clone, Debug, serde::Serialize)]
pub struct OwnedParticleFilterHit {
    /// Particle identity and quantum numbers.
    pub particle: ParticleSummary,
    /// Matching PDG channels, empty for a search without decay filters.
    pub matching_decays: Vec<OwnedDecayChannelMatch>,
}

/// A recorded decay channel that satisfied an initial- or final-state filter.
#[derive(Debug)]
pub struct DecayChannelMatch<'pdg> {
    /// PDG identifier for the channel, which may be listed under a shared particle entry.
    pub pdgid: PdgId,
    /// PDG description, including any qualifiers shown in the listing.
    pub description: String,
    /// Branching-fraction data type for the channel.
    pub data_type: DataType,
    /// All initial and final states as recorded, with their multiplicities.
    pub products: Vec<DecayProduct<'pdg>>,
    /// Whether this channel satisfied the supplied initial-state filter.
    pub matched_initial_states: bool,
    /// Whether this channel satisfied the supplied final-state filter.
    pub matched_final_states: bool,
}

/// A matching decay channel with no database borrow.
#[derive(Clone, Debug, serde::Serialize)]
pub struct OwnedDecayChannelMatch {
    /// PDG identifier for the channel.
    pub pdgid: PdgId,
    /// Recorded PDG description, including any qualifiers.
    pub description: String,
    /// Branching-fraction data type for the channel.
    pub data_type: DataType,
    /// Recorded initial and final states with their multiplicities.
    pub products: Vec<DecayProductSummary>,
    /// Whether this channel satisfied the supplied initial-state filter.
    pub matched_initial_states: bool,
    /// Whether this channel satisfied the supplied final-state filter.
    pub matched_final_states: bool,
}

impl From<&DecayChannelMatch<'_>> for OwnedDecayChannelMatch {
    fn from(channel: &DecayChannelMatch<'_>) -> Self {
        Self {
            pdgid: channel.pdgid.clone(),
            description: channel.description.clone(),
            data_type: channel.data_type,
            products: channel
                .products
                .iter()
                .map(DecayProductSummary::from)
                .collect(),
            matched_initial_states: channel.matched_initial_states,
            matched_final_states: channel.matched_final_states,
        }
    }
}

/// Recorded initial or final state in an owned decay channel.
#[derive(Clone, Debug, serde::Serialize)]
pub struct DecayProductSummary {
    /// PDG item name.
    pub name: String,
    /// Whether this item is an outgoing product.
    pub is_outgoing: bool,
    /// Number of occurrences represented by this listing row.
    pub multiplier: i64,
}

impl From<&DecayProduct<'_>> for DecayProductSummary {
    fn from(product: &DecayProduct<'_>) -> Self {
        Self {
            name: product.name.clone(),
            is_outgoing: product.is_outgoing,
            multiplier: product.multiplier,
        }
    }
}

/// Builder for particle searches run by [`crate::Pdg::search_particles`] and
/// ranked name searches run by [`crate::Pdg::find_particles`].
///
/// Filters are combined with logical `AND`.
///
/// # Examples
///
/// ```no_run
/// use pdg_rs::{Charge, ParticleClass, ParticleSearchQuery, Pdg};
///
/// # fn main() -> pdg_rs::PdgResult<()> {
/// let query = ParticleSearchQuery::new()
///     .class(ParticleClass::Meson)
///     .charge(Charge::Neutral)
///     .mass_range_mev(100.0, 1000.0);
///
/// let pdg = Pdg::open()?;
/// let particles = pdg.search_particles(query)?;
/// assert!(particles.iter().all(|particle| particle.charge == Charge::Neutral));
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Default)]
pub struct ParticleSearchQuery {
    pub(crate) name_contains: Option<String>,
    pub(crate) mcid: Option<isize>,
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
    pub(crate) include_unknown_properties: bool,
    pub(crate) decays_to: DecayFilter,
    pub(crate) decays_from: Vec<String>,
    pub(crate) decay_state_expansion: DecayStateExpansion,
}

#[derive(Clone, Debug, Default)]
pub struct DecayFilter {
    pub(crate) states: Vec<String>,
    pub(crate) mode: DecayMatchMode,
}

/// Matching mode for decay-product filters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DecayMatchMode {
    /// Require a decay mode whose outgoing products exactly match the requested states.
    #[default]
    Exact,
    /// Require a decay mode containing all requested states, allowing additional products.
    Contains,
}

/// Controls how named decay states are expanded through the PDG item hierarchy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DecayStateExpansion {
    /// Resolve names, aliases, resonance shorthand, and unique close matches,
    /// then include related PDG decay states.
    #[default]
    Inclusive,
    /// Use the exact product spellings supplied in the query.
    Literal,
}

/// Filter for optional quantum numbers.
#[derive(Clone, Debug, Default)]
pub enum QuantumFilter<T> {
    /// Accept any value, including missing values.
    #[default]
    Any,
    /// Require the quantum number to be missing.
    Missing,
    /// Require an exact value.
    Value(T),
}

impl ParticleSearchQuery {
    /// Creates an empty particle search query.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filters particles whose name contains `value`.
    #[must_use]
    pub fn name_contains(mut self, value: impl Into<String>) -> Self {
        self.name_contains = Some(value.into());
        self
    }

    /// Filters particles by their signed Monte Carlo ID.
    #[must_use]
    pub const fn mcid(mut self, mcid: isize) -> Self {
        self.mcid = Some(mcid);
        self
    }

    /// Filters particles by [`ParticleClass`].
    #[must_use]
    pub const fn class(mut self, particle_class: ParticleClass) -> Self {
        self.particle_class = Some(particle_class);
        self
    }

    /// Filters particles by particle/antiparticle relation.
    #[must_use]
    pub const fn particle_type(mut self, particle_type: ParticleType) -> Self {
        self.particle_type = Some(particle_type);
        self
    }

    /// Filters particles by electric charge.
    #[must_use]
    pub const fn charge(mut self, charge: Charge) -> Self {
        self.charge = Some(charge);
        self
    }

    /// Filters particles by isospin.
    ///
    /// Both [`Isospin::I0`] and [`Isospin::I2`] match the photon's ambiguous isospin.
    /// Passing `None` requires the isospin value to be missing.
    #[must_use]
    pub fn isospin(mut self, isospin: impl Into<Option<Isospin>>) -> Self {
        self.isospin = quantum_filter(isospin);
        self
    }

    /// Filters particles by G-parity.
    ///
    /// Passing `None` requires the G-parity value to be missing.
    #[must_use]
    pub fn g_parity(mut self, g_parity: impl Into<Option<Parity>>) -> Self {
        self.g_parity = quantum_filter(g_parity);
        self
    }

    /// Filters particles by angular momentum.
    ///
    /// Passing `None` requires the angular-momentum value to be missing.
    #[must_use]
    pub fn angular_momentum(
        mut self,
        angular_momentum: impl Into<Option<AngularMomentum>>,
    ) -> Self {
        self.angular_momentum = quantum_filter(angular_momentum);
        self
    }

    /// Filters particles by parity.
    ///
    /// Passing `None` requires the parity value to be missing.
    #[must_use]
    pub fn parity(mut self, parity: impl Into<Option<Parity>>) -> Self {
        self.parity = quantum_filter(parity);
        self
    }

    /// Filters particles by charge-conjugation parity.
    ///
    /// Passing `None` requires the charge-conjugation value to be missing.
    #[must_use]
    pub fn charge_conjugation(mut self, charge_conjugation: impl Into<Option<Parity>>) -> Self {
        self.charge_conjugation = quantum_filter(charge_conjugation);
        self
    }

    /// Filters particles whose mass interval overlaps the given range in `MeV`.
    #[must_use]
    pub const fn mass_range_mev(mut self, min: f64, max: f64) -> Self {
        self.mass_range_mev = Some((min, max));
        self
    }

    /// Filters particles whose width interval overlaps the given range in `MeV`.
    #[must_use]
    pub const fn width_range_mev(mut self, min: f64, max: f64) -> Self {
        self.width_range_mev = Some((min, max));
        self
    }

    /// Filters particles whose lifetime interval overlaps the given range in seconds.
    #[must_use]
    pub const fn lifetime_range_seconds(mut self, min: f64, max: f64) -> Self {
        self.lifetime_range_seconds = Some((min, max));
        self
    }

    /// Includes particles with no comparable value for a requested property range.
    /// By default each requested range requires an overlapping property interval.
    #[must_use]
    pub const fn include_unknown_properties(mut self, include: bool) -> Self {
        self.include_unknown_properties = include;
        self
    }

    pub(crate) fn validate_ranges(&self) -> PdgResult<()> {
        for (property, range) in [
            ("mass", self.mass_range_mev),
            ("width", self.width_range_mev),
            ("lifetime", self.lifetime_range_seconds),
        ] {
            if let Some((min, max)) = range
                && (min.is_nan() || max.is_nan() || min > max)
            {
                return Err(PdgError::InvalidPropertyRange { property, min, max });
            }
        }
        Ok(())
    }

    /// Filters particles by an exact outgoing decay state.
    ///
    /// Use [`ParticleSearchQuery::decay_contains`] to allow additional outgoing
    /// products.
    #[must_use]
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

    /// Filters particles by outgoing decay products contained in a decay state.
    /// Each occurrence of a requested product must be present in the same decay mode.
    #[must_use]
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

    /// Filters particles by incoming decay products.
    #[must_use]
    pub fn decays_from<I, S>(mut self, states: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.decays_from = states.into_iter().map(Into::into).collect();
        self
    }

    /// Sets decay-state expansion behavior.
    #[must_use]
    pub const fn decay_state_expansion(mut self, expansion: DecayStateExpansion) -> Self {
        self.decay_state_expansion = expansion;
        self
    }
}

fn quantum_filter<T>(filter: impl Into<Option<T>>) -> QuantumFilter<T> {
    filter.into().map_or_else(
        || QuantumFilter::Missing,
        |value| QuantumFilter::Value(value),
    )
}
