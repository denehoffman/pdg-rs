use pdg_rs::{
    EntryDetailView, EntryDetails, Lookup, LookupSelector, ParticleSummary, Pdg, PdgIdEntry,
    PdgReference, PdgResult, TextSearchResult,
};
use ratatui::{style::Stylize, text::Line};

use super::Hit;
use crate::footnote_key;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum View {
    Text,
    Overview,
    Mass,
    Decays,
    Measurements,
    References,
}

impl View {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Text => "Matched text",
            Self::Overview => "Overview",
            Self::Mass => "Mass",
            Self::Decays => "Decays",
            Self::Measurements => "Measurements",
            Self::References => "References",
        }
    }

    pub(super) const fn from_key(key: char) -> Option<Self> {
        match key {
            '0' => Some(Self::Text),
            '1' => Some(Self::Overview),
            '2' => Some(Self::Mass),
            '3' => Some(Self::Decays),
            '4' => Some(Self::Measurements),
            '5' => Some(Self::References),
            _ => None,
        }
    }

    const fn selection(self) -> Option<EntryDetailView> {
        match self {
            Self::Text => None,
            Self::Overview => Some(EntryDetailView::Overview),
            Self::Mass => Some(EntryDetailView::Mass),
            Self::Decays => Some(EntryDetailView::Decays),
            Self::Measurements => Some(EntryDetailView::Measurements),
            Self::References => Some(EntryDetailView::References),
        }
    }
}

pub(super) struct Detail {
    entry: PdgIdEntry,
    particle: Option<ParticleSummary>,
    text: Option<TextSearchResult>,
    pub(super) view: View,
    pub(super) lines: Vec<Line<'static>>,
    pub(super) scroll: u16,
}

impl Detail {
    pub(super) fn open(db: &Pdg, hit: &Hit) -> PdgResult<Option<Self>> {
        let Lookup::Found { entry, particles } = db.lookup(LookupSelector::PdgId(hit.pdgid()))?
        else {
            return Ok(None);
        };
        let (particle, text, view) = match hit {
            Hit::Particle(hit) => (Some(hit.particle.clone()), None, View::Overview),
            Hit::Text(hit) => (
                (particles.len() == 1).then(|| ParticleSummary::from(&particles[0])),
                Some(hit.clone()),
                View::Text,
            ),
        };
        let mut detail = Self {
            entry,
            particle,
            text,
            view,
            lines: Vec::new(),
            scroll: 0,
        };
        detail.load(db, view)?;
        Ok(Some(detail))
    }

    pub(super) fn id(&self) -> &str {
        &self.entry.pdgid
    }

    pub(super) fn name_or_id(&self) -> &str {
        self.particle
            .as_ref()
            .map_or_else(|| self.id(), |particle| particle.name.as_str())
    }

    pub(super) const fn has_text(&self) -> bool {
        self.text.is_some()
    }

    pub(super) fn load(&mut self, db: &Pdg, view: View) -> PdgResult<()> {
        let mut lines = vec![
            Line::raw(self.entry.description.clone()).bold(),
            Line::raw(format!("{}  ·  {}", self.entry.pdgid, self.entry.data_type)),
        ];
        if let Some(particle) = &self.particle {
            lines.push(Line::raw(format!(
                "{}  ·  charge {}{}",
                particle.name,
                particle.charge,
                particle
                    .mcid
                    .map_or_else(String::new, |mcid| format!("  ·  MCID {mcid}"))
            )));
        }

        if let Some(selection) = view.selection() {
            let details = db.entry_details(&self.entry, selection)?;
            let header_len = lines.len();
            append_details(&mut lines, &details);
            if lines.len() == header_len {
                lines.push(Line::default());
                lines.push(Line::raw("No data for this view."));
            }
        } else if let Some(hit) = &self.text {
            lines.push(Line::default());
            if let Some(parent) = &hit.parent {
                lines.push(Line::raw(format!(
                    "Parent {}  {}",
                    parent.pdgid, parent.title
                )));
                lines.push(Line::default());
            }
            lines.extend(hit.text.lines().map(|line| Line::raw(line.to_owned())));
        } else {
            return Ok(());
        }
        self.view = view;
        self.lines = lines;
        self.scroll = 0;
        Ok(())
    }
}

fn append_details(lines: &mut Vec<Line<'static>>, details: &EntryDetails<'_>) {
    for property in &details.properties {
        lines.push(Line::default());
        lines.push(
            Line::raw(format!(
                "{}  {}",
                property.data_type, property.value.display
            ))
            .bold(),
        );
        lines.push(Line::raw(format!(
            "{}  ·  {}",
            property.value.pdgid, property.source
        )));
    }
    for value in &details.content.data {
        lines.push(Line::raw(format!("{}  {}", value.value_type, value)));
    }
    for text in &details.content.texts {
        if let Some(text) = &text.text {
            lines.push(Line::default());
            lines.extend(text.lines().map(|line| Line::raw(line.to_owned())));
        }
    }
    for decay in &details.branching_fractions {
        lines.push(Line::default());
        lines.push(Line::raw(format!("{}  {}", decay.pdgid, decay.description)));
        lines.push(Line::raw(format!("{}  ·  {}", decay.kind, decay.value)));
    }
    for ratio in &details.branching_ratios {
        lines.push(Line::default());
        lines.push(Line::raw(format!("{}  {}", ratio.pdgid, ratio.description)));
        lines.push(Line::raw(format!("Branching ratio  ·  {}", ratio.value)));
    }
    for measurement in &details.content.measurements {
        lines.push(Line::default());
        lines.push(
            Line::raw(format!(
                "{}  ·  {}",
                measurement.pdgid, measurement.reference.document_id
            ))
            .bold(),
        );
        for value in &measurement.values {
            lines.push(Line::raw(value.to_string()));
        }
        if let Some(technique) = &measurement.technique {
            lines.push(Line::raw(technique.clone()));
        }
        for footnote in &measurement.footnotes {
            if let Some(text) = &footnote.text {
                lines.extend(text.lines().map(|line| Line::raw(line.to_owned())));
            }
        }
    }
    let attached = details
        .content
        .measurements
        .iter()
        .flat_map(|measurement| &measurement.footnotes)
        .map(footnote_key)
        .collect::<HashSet<_>>();
    for footnote in &details.content.footnotes {
        if let Some(text) = &footnote.text
            && !attached.contains(&footnote_key(footnote))
        {
            lines.push(Line::default());
            lines.extend(text.lines().map(|line| Line::raw(line.to_owned())));
        }
    }
    for reference in &details.references {
        append_reference(lines, reference);
    }
}

fn append_reference(lines: &mut Vec<Line<'static>>, reference: &PdgReference) {
    lines.push(Line::default());
    lines.push(Line::raw(reference.document_id.clone()).bold());
    if let Some(title) = &reference.title {
        lines.extend(title.lines().map(|line| Line::raw(line.to_owned())));
    }
    if let Some(publication) = &reference.publication_name {
        lines.push(Line::raw(reference.publication_year.map_or_else(
            || publication.clone(),
            |year| format!("{publication} ({year})"),
        )));
    }
    if let Some(doi) = &reference.doi {
        lines.push(Line::raw(format!("https://doi.org/{doi}")));
    }
    if let Some(inspire_id) = &reference.inspire_id {
        lines.push(Line::raw(format!(
            "https://inspirehep.net/literature/{inspire_id}"
        )));
    }
}
use std::collections::HashSet;
