use std::collections::HashSet;

use crossterm::event::KeyCode;
use pdg_rs::{
    EntryDetailView, EntryDetails, Lookup, LookupSelector, ParticleSummary, Pdg, PdgIdEntry,
    PdgReference, PdgResult, TextSearchResult,
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
};

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
    Matches,
    Links,
}

impl View {
    const ORDER: [Self; 8] = [
        Self::Text,
        Self::Overview,
        Self::Mass,
        Self::Decays,
        Self::Measurements,
        Self::References,
        Self::Matches,
        Self::Links,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Text => "Matched text",
            Self::Overview => "Overview",
            Self::Mass => "Mass",
            Self::Decays => "Decays",
            Self::Measurements => "Measurements",
            Self::References => "References",
            Self::Matches => "Matching decay channels",
            Self::Links => "Linked entries",
        }
    }

    const fn tab_label(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Measurements => "Meas.",
            Self::References => "Refs",
            Self::Matches => "Matches",
            Self::Links => "Links",
            _ => self.label(),
        }
    }

    const fn selection(self) -> Option<EntryDetailView> {
        match self {
            Self::Text | Self::Matches | Self::Links => None,
            Self::Overview => Some(EntryDetailView::Overview),
            Self::Mass => Some(EntryDetailView::Mass),
            Self::Decays => Some(EntryDetailView::Decays),
            Self::Measurements => Some(EntryDetailView::Measurements),
            Self::References => Some(EntryDetailView::References),
        }
    }

    const fn is_list(self) -> bool {
        matches!(self, Self::Matches | Self::Links)
    }
}

pub(super) struct Detail {
    entry: PdgIdEntry,
    particle: Option<ParticleSummary>,
    text: Option<TextSearchResult>,
    matches: EntryList,
    links: EntryList,
    named_views: Vec<EntryDetailView>,
    overview: Vec<Line<'static>>,
    pub(super) view: View,
    lines: Vec<Line<'static>>,
    scroll: u16,
}

impl Detail {
    pub(super) fn open(db: &Pdg, hit: &Hit) -> PdgResult<Option<Self>> {
        let Lookup::Found { entry, particles } = db.lookup(LookupSelector::PdgId(hit.pdgid()))?
        else {
            return Ok(None);
        };
        let (particle, text, matches) = match hit {
            Hit::Particle(hit) => (
                Some(hit.particle.clone()),
                None,
                EntryList::new(
                    hit.matching_decays
                        .iter()
                        .map(|channel| EntryLink {
                            pdgid: channel.pdgid.clone(),
                            description: channel.description.clone(),
                            relation: None,
                        })
                        .collect(),
                ),
            ),
            Hit::Text(hit) => (
                (particles.len() == 1).then(|| ParticleSummary::from(&particles[0])),
                Some(hit.clone()),
                EntryList::default(),
            ),
        };
        Self::new(db, entry, particle, text, matches).map(Some)
    }

    pub(super) fn open_entry(db: &Pdg, id: &str) -> PdgResult<Option<Self>> {
        let Lookup::Found { entry, particles } = db.lookup(LookupSelector::PdgId(id))? else {
            return Ok(None);
        };
        let particle = (particles.len() == 1).then(|| ParticleSummary::from(&particles[0]));
        Self::new(db, entry, particle, None, EntryList::default()).map(Some)
    }

    fn new(
        db: &Pdg,
        entry: PdgIdEntry,
        particle: Option<ParticleSummary>,
        text: Option<TextSearchResult>,
        matches: EntryList,
    ) -> PdgResult<Self> {
        let (details, named_views) = db.entry_overview_and_views(&entry)?;
        let links = linked_entries(db, &entry)?;
        let mut overview = Vec::new();
        append_details(&mut overview, &details);
        let view = if !matches.entries.is_empty() {
            View::Matches
        } else if text.is_some() {
            View::Text
        } else {
            View::Overview
        };
        let mut detail = Self {
            entry,
            particle,
            text,
            matches,
            links,
            named_views,
            overview,
            view,
            lines: Vec::new(),
            scroll: 0,
        };
        detail.load(db, view)?;
        Ok(detail)
    }

    pub(super) fn id(&self) -> &str {
        &self.entry.pdgid
    }

    pub(super) fn name_or_id(&self) -> &str {
        self.particle
            .as_ref()
            .map_or_else(|| self.id(), |particle| particle.name.as_str())
    }

    pub(super) fn cycle_view(&mut self, db: &Pdg, backwards: bool) -> PdgResult<()> {
        let views: Vec<_> = self.available_views().collect();
        let current = views
            .iter()
            .position(|&view| view == self.view)
            .unwrap_or_default();
        let offset = if backwards { views.len() - 1 } else { 1 };
        self.load(db, views[(current + offset) % views.len()])
    }

    pub(super) fn select_number(&mut self, db: &Pdg, key: char) -> PdgResult<()> {
        let view = "123456789"
            .find(key)
            .and_then(|index| self.available_views().nth(index));
        if let Some(view) = view {
            self.load(db, view)?;
        }
        Ok(())
    }

    fn available_views(&self) -> impl Iterator<Item = View> + '_ {
        View::ORDER.into_iter().filter(|&view| self.has_view(view))
    }

    fn has_view(&self, view: View) -> bool {
        match view {
            View::Text => self.text.is_some(),
            View::Matches => !self.matches.entries.is_empty(),
            View::Links => !self.links.entries.is_empty(),
            _ => view
                .selection()
                .is_some_and(|view| self.named_views.contains(&view)),
        }
    }

    pub(super) fn selected_entry(&self) -> Option<&str> {
        let list = match self.view {
            View::Matches => &self.matches,
            View::Links => &self.links,
            _ => return None,
        };
        let index = list.state.selected()?;
        list.entries.get(index).map(|entry| entry.pdgid.as_str())
    }

    const fn current_list_mut(&mut self) -> Option<&mut EntryList> {
        match self.view {
            View::Matches => Some(&mut self.matches),
            View::Links => Some(&mut self.links),
            _ => None,
        }
    }

    pub(super) fn navigate(&mut self, code: KeyCode, page: u16) {
        if let Some(list) = self.current_list_mut() {
            list.navigate(code, page);
        } else {
            self.scroll = match code {
                KeyCode::Up => self.scroll.saturating_sub(1),
                KeyCode::Down => self.scroll.saturating_add(1),
                KeyCode::PageUp => self.scroll.saturating_sub(page),
                KeyCode::PageDown => self.scroll.saturating_add(page),
                KeyCode::Home => 0,
                KeyCode::End => u16::MAX,
                _ => return,
            };
        }
    }

    pub(super) fn draw(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        accent: Style,
        tabs: &mut Vec<(Rect, View)>,
    ) -> u16 {
        let block = Block::bordered()
            .title(format!(" {} · {} ", self.id(), self.view.label()))
            .border_style(accent);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let content = self.draw_tabs(frame, inner, accent, tabs);
        if content.height > 0 {
            if self.view.is_list() {
                self.draw_list(frame, content);
            } else {
                let paragraph =
                    Paragraph::new(Text::from(self.lines.clone())).wrap(Wrap { trim: true });
                let max_scroll = paragraph
                    .line_count(content.width)
                    .saturating_sub(usize::from(content.height));
                self.scroll = self
                    .scroll
                    .min(u16::try_from(max_scroll).unwrap_or(u16::MAX));
                frame.render_widget(paragraph.scroll((self.scroll, 0)), content);
            }
        }
        content.height.max(1)
    }

    fn draw_tabs(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        accent: Style,
        tabs: &mut Vec<(Rect, View)>,
    ) -> Rect {
        let mut x = area.x;
        let mut y = area.y;
        for (index, view) in self.available_views().enumerate() {
            let label = Span::raw(format!("{} {}", index + 1, view.tab_label()));
            let width = u16::try_from(label.width())
                .unwrap_or(u16::MAX)
                .min(area.width);
            if area.right().saturating_sub(x) < width {
                x = area.x;
                y = y.saturating_add(1);
            }
            if y >= area.bottom() {
                break;
            }
            let tab = Rect::new(x, y, width, 1);
            let style = if view == self.view {
                accent.bold().reversed()
            } else {
                Style::new()
            };
            frame.render_widget(label.style(style), tab);
            tabs.push((tab, view));
            x = tab.right().saturating_add(2);
        }
        let top = y.saturating_add(1).min(area.bottom());
        Rect::new(area.x, top, area.width, area.bottom() - top)
    }

    fn draw_list(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let Some(list) = self.current_list_mut() else {
            return;
        };
        let items = list.entries.iter().map(|entry| {
            ListItem::new(entry.relation.map_or_else(
                || format!("{}  {}", entry.pdgid, entry.description),
                |relation| format!("{}  {relation}  {}", entry.pdgid, entry.description),
            ))
        });
        frame.render_stateful_widget(
            List::new(items)
                .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
                .highlight_symbol("› "),
            area,
            &mut list.state,
        );
    }

    pub(super) fn load(&mut self, db: &Pdg, view: View) -> PdgResult<()> {
        if !self.has_view(view) {
            return Ok(());
        }
        if view.is_list() {
            self.view = view;
            self.lines.clear();
            self.scroll = 0;
            return Ok(());
        }
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
            let header_len = lines.len();
            if selection == EntryDetailView::Overview {
                lines.extend(self.overview.iter().cloned());
            } else {
                let details = db.entry_details(&self.entry, selection)?;
                append_details(&mut lines, &details);
            }
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
        }
        self.view = view;
        self.lines = lines;
        self.scroll = 0;
        Ok(())
    }
}

struct EntryLink {
    pdgid: String,
    description: String,
    relation: Option<&'static str>,
}

impl EntryLink {
    fn from_entry(entry: PdgIdEntry, relation: &'static str) -> Self {
        Self {
            pdgid: entry.pdgid,
            description: entry.description,
            relation: Some(relation),
        }
    }
}

#[derive(Default)]
struct EntryList {
    entries: Vec<EntryLink>,
    state: ListState,
}

impl EntryList {
    fn new(entries: Vec<EntryLink>) -> Self {
        let state = ListState::default().with_selected((!entries.is_empty()).then_some(0));
        Self { entries, state }
    }
    fn navigate(&mut self, code: KeyCode, page: u16) {
        let Some(last) = self.entries.len().checked_sub(1) else {
            return;
        };
        let selected = self.state.selected().unwrap_or_default();
        let index = match code {
            KeyCode::Up => selected.saturating_sub(1),
            KeyCode::Down => selected.saturating_add(1).min(last),
            KeyCode::PageUp => selected.saturating_sub(usize::from(page)),
            KeyCode::PageDown => selected.saturating_add(usize::from(page)).min(last),
            KeyCode::Home => 0,
            KeyCode::End => last,
            _ => return,
        };
        self.state.select(Some(index));
    }
}

fn linked_entries(db: &Pdg, entry: &PdgIdEntry) -> PdgResult<EntryList> {
    let mut links = Vec::new();
    if let Some(parent) = &entry.parent_pdgid
        && let Some(parent) = db.pdgid(parent)?
    {
        links.push(EntryLink::from_entry(parent, "Parent"));
    }
    links.extend(
        db.children_for_pdgid(&entry.pdgid)?
            .into_iter()
            .map(|child| EntryLink::from_entry(child, "Child")),
    );
    links.extend(
        db.mapped_entries_for_pdgid(&entry.pdgid)?
            .into_iter()
            .map(|related| EntryLink::from_entry(related, "Related")),
    );
    let mut seen = HashSet::new();
    links.retain(|link| {
        link.pdgid != entry.pdgid && seen.insert((link.relation, link.pdgid.clone()))
    });
    Ok(EntryList::new(links))
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
