use crate::{
    ParticleMatchKind, ParticleSearchHit, ParticleSearchQuery, Pdg, PdgError, PdgIdEntry,
    PdgParticle, PdgResult,
};

/// Identifies an entry or particle to resolve with [`Pdg::lookup`].
#[derive(Clone, Copy, Debug)]
pub enum LookupSelector<'a> {
    /// A case-insensitive PDG database string ID. The entry may have multiple particles.
    PdgId(&'a str),
    /// A particle name or alias in a spelling accepted by ranked particle search.
    Name(&'a str),
    /// A signed Monte Carlo particle ID.
    Mcid(isize),
}

/// Result of resolving a PDG entry or particle.
#[derive(Debug)]
pub enum Lookup<'pdg> {
    /// The selector resolved to a PDG entry, with any selected particles.
    Found {
        /// The PDG entry's metadata.
        entry: PdgIdEntry,
        /// The selected particle, all variants of a PDG ID, or an empty list for a non-particle entry.
        particles: Vec<PdgParticle<'pdg>>,
    },
    /// Multiple particles or listings matched at the best direct priority.
    Ambiguous(Vec<PdgParticle<'pdg>>),
    /// No entry or unambiguous particle match was found.
    Missing {
        /// Up to five partial or typo matches when looking up a name.
        suggestions: Vec<ParticleSearchHit<'pdg>>,
    },
}

impl Pdg {
    /// Resolves a PDG ID, a particle name or alias, or a signed Monte Carlo ID.
    ///
    /// PDG IDs return the entry with all particle variants in name and MCID order.
    /// Name lookup prefers exact normalized names, exact aliases, and resonance
    /// shorthand, in that order. Resonance shorthand can select several variants
    /// under a shared PDG ID. Multiple direct matches that cannot share a listing
    /// produce [`Lookup::Ambiguous`]. A single partial or typo match resolves;
    /// multiple close matches are suggestions in [`Lookup::Missing`].
    ///
    /// # Examples
    /// ```no_run
    /// use pdg_rs::{Lookup, LookupSelector, Pdg};
    /// # fn main() -> pdg_rs::PdgResult<()> {
    /// let pdg = Pdg::open()?;
    /// if let Lookup::Found { particles, .. } = pdg.lookup(LookupSelector::Mcid(-211))? {
    ///     assert_eq!(particles.first().map(|particle| particle.name.as_str()), Some("pi-"));
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns an error if a query fails or its matching particle lacks a PDG entry.
    pub fn lookup(&self, selector: LookupSelector<'_>) -> PdgResult<Lookup<'_>> {
        match selector {
            LookupSelector::PdgId(pdgid) => {
                let Some(entry) = self.pdgid(pdgid.trim())? else {
                    return Ok(Lookup::Missing {
                        suggestions: Vec::new(),
                    });
                };
                let particles = self.entry_particles(&entry.pdgid)?;
                Ok(Lookup::Found { entry, particles })
            }
            LookupSelector::Name(name) => {
                let mut hits = self.find_particles(name, ParticleSearchQuery::new())?;
                let Some(kind) = hits.first().map(|hit| hit.kind) else {
                    return Ok(Lookup::Missing { suggestions: hits });
                };
                if !kind.is_direct() && hits.len() > 1 {
                    hits.truncate(5);
                    return Ok(Lookup::Missing { suggestions: hits });
                }
                let particles = hits
                    .into_iter()
                    .take_while(|hit| hit.kind == kind)
                    .map(|hit| hit.particle)
                    .collect();
                self.resolve_particle_candidates(particles, kind == ParticleMatchKind::Resonance)
            }
            LookupSelector::Mcid(mcid) => {
                let particles = self.search_particles(ParticleSearchQuery::new().mcid(mcid))?;
                self.resolve_particle_candidates(particles, false)
            }
        }
    }

    pub(crate) fn entry_particles(&self, pdgid: &str) -> PdgResult<Vec<PdgParticle<'_>>> {
        // The ID was resolved through `pdgid`, so the stored spelling is available here.
        let sql = format!(
            "SELECT {} FROM pdgparticle {} WHERE pdgparticle.pdgid = ?1 ORDER BY name, mcid",
            Self::PARTICLE_COLUMNS,
            Self::PARTICLE_JOIN
        );
        let mut stmt = self.conn.prepare(&sql)?;
        Ok(stmt
            .query_map([pdgid], |row| PdgParticle::from_row(self, row))?
            .collect::<Result<_, _>>()?)
    }

    fn resolve_particle_candidates<'pdg>(
        &'pdg self,
        particles: Vec<PdgParticle<'pdg>>,
        allow_shared_entry: bool,
    ) -> PdgResult<Lookup<'pdg>> {
        match particles.as_slice() {
            [] => Ok(Lookup::Missing {
                suggestions: Vec::new(),
            }),
            [particle, ..]
                if particles.len() == 1
                    || (allow_shared_entry
                        && particles
                            .iter()
                            .all(|candidate| candidate.pdgid == particle.pdgid)) =>
            {
                let entry = self.pdgid(&particle.pdgid)?.ok_or_else(|| {
                    PdgError::Custom(format!(
                        "missing PDG entry {} for {}",
                        particle.pdgid, particle.name
                    ))
                })?;
                Ok(Lookup::Found { entry, particles })
            }
            _ => Ok(Lookup::Ambiguous(particles)),
        }
    }
}
