use std::collections::HashSet;

use rusqlite::params_from_iter;

use crate::{
    BranchingFraction, BranchingRatio, DataEntry, DataType, ParticleDetailSelection,
    ParticlePropertyDetail, Pdg, PdgFootnote, PdgId, PdgIdEntry, PdgMeasurement, PdgParticle,
    PdgReference, PdgResult, PdgText,
};

/// Selects the data loaded for an entry by [`Pdg::entry_details`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum EntryDetailView {
    /// Headline properties, direct data, and text for non-particle entries.
    #[default]
    Overview,
    /// The selected edition's mass data.
    Mass,
    /// Branching fractions and ratios, or direct data for a decay entry.
    Decays,
    /// Measurements on the entry and its headline properties or section data.
    Measurements,
    /// Unique references for the same scope as the measurements view.
    References,
    /// The entry's content, hierarchy, and particle details.
    Full {
        /// Include the content of descendants and linked entries.
        related: bool,
    },
}

/// The selected sections of a PDG entry, ready to render.
#[derive(Debug, Default)]
pub struct EntryDetails<'pdg> {
    /// Values, text, measurements, and footnotes selected for the entry.
    pub content: EntryContent<'pdg>,
    /// Headline or mass properties with their source identifiers.
    pub properties: Vec<ParticlePropertyDetail>,
    /// Listed branching fractions, including their decay products and related data.
    pub branching_fractions: Vec<BranchingFraction<'pdg>>,
    /// Listed branching ratios.
    pub branching_ratios: Vec<BranchingRatio<'pdg>>,
    /// References selected for the references view.
    pub references: Vec<PdgReference>,
    /// Immediate child entries.
    pub children: Vec<PdgIdEntry>,
    /// Entries linked by the PDG mapping table.
    pub related_entries: Vec<PdgIdEntry>,
    /// Particles related through the item hierarchy.
    pub related_particles: Vec<PdgParticle<'pdg>>,
    /// Content of requested descendants and linked entries.
    pub related_details: Vec<RelatedEntryDetails<'pdg>>,
}

/// The data stored directly under an entry, or evidence selected for a detail view.
#[derive(Debug, Default)]
pub struct EntryContent<'pdg> {
    /// Numeric and textual data values for the selected numeric edition.
    pub data: Vec<DataEntry<'pdg>>,
    /// Text blocks.
    pub texts: Vec<PdgText>,
    /// Measurements, with their values, references, and attached footnotes.
    pub measurements: Vec<PdgMeasurement>,
    /// Footnotes associated with the selected entries.
    pub footnotes: Vec<PdgFootnote>,
}

/// An expanded descendant or linked entry with its direct content.
#[derive(Debug)]
pub struct RelatedEntryDetails<'pdg> {
    /// Entry metadata.
    pub entry: PdgIdEntry,
    /// Content associated with this entry.
    pub content: EntryContent<'pdg>,
}

impl Pdg {
    /// Loads the selected details for an entry returned by this database.
    ///
    /// Numeric data uses [`Pdg::edition`]. Measurements and references come from
    /// the database release. Property and decay listings are shared by variants
    /// with the same PDG ID.
    ///
    /// # Errors
    /// Returns a database error if a query for the selected view fails.
    pub fn entry_details(
        &self,
        entry: &PdgIdEntry,
        view: EntryDetailView,
    ) -> PdgResult<EntryDetails<'_>> {
        use EntryDetailView::{Decays, Full, Mass, Measurements, Overview, References};

        let id = entry.pdgid.as_str();
        let particles = self.detail_particles(entry)?;
        let particle = particles.first();
        let mut details = EntryDetails::default();

        match view {
            Overview | Full { .. } => {
                details = self.load_entry_overview(entry, particle, matches!(view, Full { .. }))?;
            }
            Mass => {
                if let Some(particle) = particle {
                    details.properties = particle
                        .details(ParticleDetailSelection::Properties(&[DataType::Mass]))?
                        .properties;
                } else if entry.data_type == DataType::Mass {
                    details.content.data = self.data_for(id)?;
                }
            }
            Decays => {
                if entry.data_type.is_branching_fraction()
                    || entry.data_type == DataType::BranchingRatio
                {
                    details.content.data = self.data_for(id)?;
                }
            }
            Measurements | References => {
                let properties = match particle {
                    Some(particle) => {
                        particle
                            .details(ParticleDetailSelection::Overview)?
                            .properties
                    }
                    None => Vec::new(),
                };
                let ids = self.evidence_ids(entry, &properties)?;
                if view == Measurements {
                    for id in ids {
                        details
                            .content
                            .measurements
                            .extend(self.measurements_for(&id)?);
                        details.content.footnotes.extend(self.footnotes_for(id)?);
                    }
                } else {
                    details.references = self.references_for_entries(&ids)?;
                }
            }
        }

        if matches!(view, Decays | Full { .. })
            && let Some(particle) = particle
        {
            details.branching_fractions = particle.branching_fractions()?;
            details.branching_ratios = particle.branching_ratios()?;
        }
        if let Full { related } = view {
            details.children = self.children_for_pdgid(id)?;
            details.related_entries = self.mapped_entries_for_pdgid(id)?;
            details.content.measurements = self.measurements_for(id)?;
            details.content.footnotes = self.footnotes_for(id)?;
            let mut seen = HashSet::new();
            for particle in &particles {
                details.related_particles.extend(
                    particle.related_particles()?.into_iter().filter(|related| {
                        seen.insert((related.pdgid.clone(), related.name.clone()))
                    }),
                );
            }
            if related {
                details.related_details = self.expand_related_details(id, &details)?;
            }
        }
        Ok(details)
    }

    /// Loads the overview and lists the named views available for browsing the entry.
    ///
    /// The list starts with [`EntryDetailView::Overview`], which includes data stored directly
    /// on the entry, such as a mass value or branching ratio. The Mass and Decays views group
    /// data for particle entries. Mass and decay availability uses the selected numeric edition.
    /// Measurements and references use the opened release.
    ///
    /// # Errors
    /// Returns a database error if the overview or view availability cannot be queried.
    pub fn entry_overview_and_views(
        &self,
        entry: &PdgIdEntry,
    ) -> PdgResult<(EntryDetails<'_>, Vec<EntryDetailView>)> {
        let particles = self.detail_particles(entry)?;
        let particle = particles.first();
        let overview = self.load_entry_overview(entry, particle, false)?;
        let mut views = vec![EntryDetailView::Overview];
        if overview
            .properties
            .iter()
            .any(|property| property.data_type == DataType::Mass)
        {
            views.push(EntryDetailView::Mass);
        }
        if particle.is_some() && self.has_decay_data(&entry.pdgid)? {
            views.push(EntryDetailView::Decays);
        }
        let ids = self.evidence_ids(entry, &overview.properties)?;
        let (measurements, references) = self.available_evidence(&ids)?;
        if measurements {
            views.push(EntryDetailView::Measurements);
        }
        if references {
            views.push(EntryDetailView::References);
        }
        Ok((overview, views))
    }

    fn detail_particles(&self, entry: &PdgIdEntry) -> PdgResult<Vec<PdgParticle<'_>>> {
        if entry.data_type == DataType::Particle {
            self.entry_particles(&entry.pdgid)
        } else {
            Ok(Vec::new())
        }
    }

    fn load_entry_overview(
        &self,
        entry: &PdgIdEntry,
        particle: Option<&PdgParticle<'_>>,
        include_particle_text: bool,
    ) -> PdgResult<EntryDetails<'_>> {
        let mut details = EntryDetails::default();
        if let Some(particle) = particle {
            details.properties = particle
                .details(ParticleDetailSelection::Overview)?
                .properties;
        }
        details.content.data = self.data_for(&entry.pdgid)?;
        if particle.is_none() || include_particle_text {
            details.content.texts = self.texts_for(&entry.pdgid)?;
        }
        Ok(details)
    }

    fn has_decay_data(&self, id: &str) -> PdgResult<bool> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT DISTINCT pdgid.data_type
             FROM pdgid JOIN pdgdata ON pdgdata.pdgid_id = pdgid.id
             WHERE pdgid.parent_pdgid = ?1 AND pdgdata.edition = ?2
               AND (pdgid.data_type GLOB 'BF[IX]*' OR pdgid.data_type = ?3)",
        )?;
        let mut rows = stmt.query([id, self.edition(), DataType::BranchingRatio.to_code()])?;
        while let Some(row) = rows.next()? {
            if row
                .get::<_, String>(0)?
                .parse::<DataType>()
                .is_ok_and(|kind| kind.is_branching_fraction() || kind == DataType::BranchingRatio)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn available_evidence(&self, ids: &[PdgId]) -> PdgResult<(bool, bool)> {
        let slots = vec!["?"; ids.len()].join(", ");
        let references = self.conn.query_row(
            &format!(
                "SELECT EXISTS (
                     SELECT 1 FROM pdgmeasurement
                     JOIN pdgreference ON pdgreference.id = pdgmeasurement.pdgreference_id
                     WHERE pdgmeasurement.pdgid IN ({slots})
                 )"
            ),
            params_from_iter(ids),
            |row| row.get::<_, bool>(0),
        )?;
        let measurements = references
            || self.conn.query_row(
                &format!(
                    "SELECT EXISTS (
                     SELECT 1 FROM pdgfootnote WHERE pdgid IN ({slots}) AND trim(text) <> ''
                 )"
                ),
                params_from_iter(ids),
                |row| row.get::<_, bool>(0),
            )?;
        Ok((measurements, references))
    }

    fn evidence_ids(
        &self,
        entry: &PdgIdEntry,
        properties: &[ParticlePropertyDetail],
    ) -> PdgResult<Vec<PdgId>> {
        let mut ids = vec![entry.pdgid.clone()];
        ids.extend(
            properties
                .iter()
                .map(|property| property.value.pdgid.clone()),
        );
        if entry.data_type == DataType::Section {
            ids.extend(
                self.children_for_pdgid(&entry.pdgid)?
                    .into_iter()
                    .map(|child| child.pdgid),
            );
        }
        let mut seen = HashSet::new();
        ids.retain(|id| seen.insert(id.clone()));
        Ok(ids)
    }

    fn references_for_entries(&self, ids: &[PdgId]) -> PdgResult<Vec<PdgReference>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT document_id, publication_name, publication_year, doi, inspire_id, title
             FROM pdgmeasurement
             JOIN pdgreference ON pdgreference.id = pdgmeasurement.pdgreference_id
             WHERE pdgmeasurement.pdgid = ?1 ORDER BY pdgmeasurement.sort, pdgmeasurement.id",
        )?;
        let mut references = Vec::new();
        let mut seen = HashSet::new();
        for id in ids {
            let rows = stmt.query_map([id], |row| PdgReference::from_row(row, 0))?;
            for row in rows {
                let reference = row?;
                if seen.insert(reference.document_id.clone()) {
                    references.push(reference);
                }
            }
        }
        Ok(references)
    }

    fn expand_related_details<'pdg>(
        &'pdg self,
        root: &str,
        details: &EntryDetails<'_>,
    ) -> PdgResult<Vec<RelatedEntryDetails<'pdg>>> {
        let mut pending: Vec<_> = details
            .children
            .iter()
            .chain(&details.related_entries)
            .map(|entry| entry.pdgid.clone())
            .collect();
        for decay in &details.branching_fractions {
            pending.push(decay.pdgid.clone());
            pending.extend(decay.related_data.iter().map(|entry| entry.pdgid.clone()));
        }
        pending.extend(
            details
                .branching_ratios
                .iter()
                .map(|entry| entry.pdgid.clone()),
        );

        let mut seen = HashSet::new();
        let mut related = Vec::new();
        while let Some(id) = pending.pop() {
            if id == root || !seen.insert(id.clone()) {
                continue;
            }
            let Some(entry) = self.pdgid(&id)? else {
                continue;
            };
            pending.extend(
                self.children_for_pdgid(&id)?
                    .into_iter()
                    .map(|child| child.pdgid),
            );
            pending.extend(
                self.mapped_entries_for_pdgid(&id)?
                    .into_iter()
                    .map(|entry| entry.pdgid),
            );
            let content = EntryContent {
                data: self.data_for(&id)?,
                texts: self.texts_for(&id)?,
                measurements: self.measurements_for(&id)?,
                footnotes: self.footnotes_for(id)?,
            };
            if !content.data.is_empty()
                || !content.texts.is_empty()
                || !content.measurements.is_empty()
                || !content.footnotes.is_empty()
            {
                related.push(RelatedEntryDetails { entry, content });
            }
        }
        Ok(related)
    }
}
