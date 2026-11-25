use std::{
    collections::HashMap,
    env,
    io::{self, IsTerminal},
    time::{Duration, Instant},
};

use crossterm::{
    clipboard::CopyToClipboard,
    cursor::Show,
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    },
    execute,
};
use pdg_rs::{
    OwnedDecayChannelMatch, ParticleSearchQuery, ParticleSummary, Pdg, PdgResult, TextSearchResult,
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{CliError, CliResult, DatabaseOptions, ParticleSort, TuiCommand, open_pdg};

mod command;
mod detail;
mod filters;
mod help;
mod query;
use command::SearchSource;
use detail::{Detail, View};
use filters::{AppliedFilters, EditorAction, FilterEditor};
use help::Help;
use query::{Completions, InlineQuery, format_query, quote_name};

const RESULT_LIMIT: usize = 50;
const SEARCH_DELAY: Duration = Duration::from_millis(120);

const fn scroll_by_key(scroll: u16, code: KeyCode, page: u16) -> u16 {
    match code {
        KeyCode::Up => scroll.saturating_sub(1),
        KeyCode::Down => scroll.saturating_add(1),
        KeyCode::PageUp => scroll.saturating_sub(page),
        KeyCode::PageDown => scroll.saturating_add(page),
        KeyCode::Home => 0,
        KeyCode::End => u16::MAX,
        _ => scroll,
    }
}

pub fn run(command: &TuiCommand, database: &DatabaseOptions) -> CliResult<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(CliError::InvalidArgument(
            "`pdg tui` requires an interactive terminal. Use `pdg find` for piped output.".into(),
        ));
    }

    let db = open_pdg(database, command.edition.as_deref())?;
    let mut app = App::new(
        command.query.as_deref().unwrap_or_default(),
        command.text,
        SearchSource::new(database, command.edition.as_deref()),
    );
    let mut terminal = ratatui::try_init().inspect_err(|_| ratatui::restore())?;
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = reset_terminal_input();
        panic_hook(info);
    }));
    let result = (|| {
        execute!(io::stdout(), EnableBracketedPaste, EnableMouseCapture)?;
        drive(&mut terminal, &mut app, &db)
    })();
    let reset = reset_terminal_input();
    let restore = ratatui::try_restore();
    result?;
    reset?;
    restore?;
    Ok(())
}

fn reset_terminal_input() -> io::Result<()> {
    execute!(
        io::stdout(),
        DisableMouseCapture,
        DisableBracketedPaste,
        Show
    )
}

fn drive(terminal: &mut DefaultTerminal, app: &mut App, db: &Pdg) -> CliResult<()> {
    loop {
        if app.search_due() {
            app.search(db)?;
        }
        terminal.draw(|frame| app.draw(frame, db))?;
        if let Some(deadline) = app.pending
            && !event::poll(deadline.saturating_duration_since(Instant::now()))?
        {
            continue;
        }
        match event::read()? {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                match app.key(db, key)? {
                    Action::None => {}
                    Action::Quit => return Ok(()),
                    Action::ApplyFilters => {
                        terminal.draw(|frame| app.draw(frame, db))?;
                        app.apply_filters(db);
                    }
                    Action::Copy { value, notice } => {
                        app.notice = Some(
                            match execute!(io::stdout(), CopyToClipboard::to_clipboard_from(&value))
                            {
                                Ok(()) => notice,
                                Err(error) => format!("Clipboard unavailable. {error}"),
                            },
                        );
                    }
                }
            }
            Event::Mouse(mouse) => app.mouse(db, mouse)?,
            Event::Paste(text) if app.detail.is_none() => app.paste(db, &text),
            _ => {}
        }
    }
}

enum Action {
    None,
    Quit,
    ApplyFilters,
    Copy { value: String, notice: String },
}

impl Action {
    fn copy(value: &str) -> Self {
        Self::Copy {
            value: value.to_owned(),
            notice: format!("Sent {value} to the terminal clipboard."),
        }
    }
}

enum Hit {
    Particle(ParticleHit),
    Text(TextSearchResult),
}

struct ParticleHit {
    particle: ParticleSummary,
    matched_name: Option<String>,
    matching_decays: Vec<OwnedDecayChannelMatch>,
}

fn particle_hits(
    db: &Pdg,
    name: &str,
    filters: Option<&AppliedFilters>,
    sort: ParticleSort,
) -> PdgResult<Vec<Hit>> {
    if name.is_empty() {
        let hits = filters.into_iter().flat_map(|filters| &filters.hits);
        return Ok(
            limit_particle_results(db, hits, sort, |hit| &hit.particle.pdgid)?
                .into_iter()
                .map(|hit| {
                    Hit::Particle(ParticleHit {
                        particle: hit.particle.clone(),
                        matched_name: None,
                        matching_decays: hit.matching_decays.clone(),
                    })
                })
                .collect(),
        );
    }

    // The physics query is independent of name rank. Intersect the ranked results
    // with the cached physics matches before applying the browser's result limit.
    let eligible = filters.map(|filters| {
        filters
            .hits
            .iter()
            .map(|hit| (particle_key(&hit.particle), hit.matching_decays.as_slice()))
            .collect::<HashMap<_, _>>()
    });
    let hits = db
        .find_particles(name, ParticleSearchQuery::new())?
        .into_iter()
        .filter_map(|hit| {
            let matching_decays = match &eligible {
                Some(eligible) => eligible
                    .get(&(
                        hit.particle.pdgid.as_str(),
                        hit.particle.name.as_str(),
                        hit.particle.mcid,
                    ))
                    .copied()?,
                None => &[],
            };
            Some((hit, matching_decays))
        });
    Ok(
        limit_particle_results(db, hits, sort, |(hit, _)| &hit.particle.pdgid)?
            .into_iter()
            .map(|(hit, matching_decays)| {
                Hit::Particle(ParticleHit {
                    particle: ParticleSummary::from(&hit.particle),
                    matched_name: Some(hit.matched_name),
                    matching_decays: matching_decays.to_vec(),
                })
            })
            .collect(),
    )
}

fn limit_particle_results<T>(
    db: &Pdg,
    hits: impl Iterator<Item = T>,
    sort: ParticleSort,
    pdgid: impl Fn(&T) -> &str,
) -> PdgResult<Vec<T>> {
    if sort == ParticleSort::Mass {
        let mut hits: Vec<_> = hits.collect();
        db.sort_particles_by_mass(&mut hits, pdgid)?;
        hits.truncate(RESULT_LIMIT);
        Ok(hits)
    } else {
        Ok(hits.take(RESULT_LIMIT).collect())
    }
}

fn particle_key(particle: &ParticleSummary) -> (&str, &str, Option<isize>) {
    (&particle.pdgid, &particle.name, particle.mcid)
}

impl Hit {
    fn pdgid(&self) -> &str {
        match self {
            Self::Particle(hit) => &hit.particle.pdgid,
            Self::Text(hit) => &hit.pdgid,
        }
    }

    fn title(&self) -> &str {
        match self {
            Self::Particle(hit) => &hit.particle.name,
            Self::Text(hit) => hit.title.as_deref().unwrap_or(&hit.snippet),
        }
    }

    fn preview(&self) -> Vec<Line<'_>> {
        let mut lines = vec![
            Line::raw(self.title()).bold(),
            Line::raw(self.pdgid()),
            Line::default(),
        ];
        match self {
            Self::Particle(hit) => {
                if hit.particle.description != hit.particle.name {
                    lines.push(Line::raw(hit.particle.description.as_str()));
                }
                if let Some(spelling) = &hit.matched_name
                    && spelling != &hit.particle.name
                {
                    lines.push(Line::raw(format!("Matched {spelling}")));
                }
                if !hit.matching_decays.is_empty() {
                    lines.push(Line::raw(format!(
                        "Matching decay channels · {}",
                        hit.matching_decays.len()
                    )));
                }
            }
            Self::Text(hit) => lines.push(Line::raw(hit.snippet.as_str())),
        }
        lines.push(Line::default());
        lines.push(Line::raw("Enter to open"));
        lines
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Particles,
    Text,
}

impl Mode {
    const fn label(self) -> &'static str {
        match self {
            Self::Particles => "Particles",
            Self::Text => "PDG text",
        }
    }
}

struct App {
    input: SearchInput,
    other_input: Option<SearchInput>,
    mode: Mode,
    sort: ParticleSort,
    source: SearchSource,
    hits: Vec<Hit>,
    results: ListState,
    detail: Option<Detail>,
    detail_history: Vec<Detail>,
    tab_regions: Vec<(Rect, View)>,
    filters: Option<AppliedFilters>,
    filter_editor: Option<FilterEditor>,
    completion: Option<(Completions, ListState)>,
    help: Option<Help>,
    pending: Option<Instant>,
    notice: Option<String>,
    search_error: Option<String>,
    page: u16,
    accent: Style,
}

impl App {
    fn new(query: &str, text: bool, source: SearchSource) -> Self {
        let mut input = SearchInput::default();
        input.insert(query);
        Self {
            input,
            other_input: None,
            mode: if text { Mode::Text } else { Mode::Particles },
            sort: ParticleSort::default(),
            source,
            hits: Vec::new(),
            results: ListState::default(),
            detail: None,
            detail_history: Vec::new(),
            tab_regions: Vec::new(),
            filters: None,
            filter_editor: None,
            completion: None,
            help: None,
            pending: Some(Instant::now()),
            notice: None,
            search_error: None,
            page: 1,
            accent: if env::var_os("NO_COLOR").is_some() {
                Style::new()
            } else {
                Style::new().fg(Color::Cyan)
            },
        }
    }

    fn search_due(&self) -> bool {
        self.pending
            .is_some_and(|deadline| deadline <= Instant::now())
    }

    fn schedule_search(&mut self) {
        self.pending = Some(Instant::now() + SEARCH_DELAY);
        self.notice = None;
        self.search_error = None;
    }

    fn search(&mut self, db: &Pdg) -> CliResult<()> {
        self.pending = None;
        self.search_error = None;
        let hits = match self.mode {
            Mode::Particles => {
                let result = InlineQuery::parse(self.input.value()).and_then(|query| {
                    if !self
                        .filters
                        .as_ref()
                        .is_some_and(|filters| filters.matches(&query.draft))
                    {
                        self.filters = query.draft.apply(db)?;
                    }
                    Ok(particle_hits(
                        db,
                        &query.name,
                        self.filters.as_ref(),
                        self.sort,
                    )?)
                });
                match result {
                    Ok(hits) => hits,
                    Err(error) => {
                        self.search_error = Some(error.to_string());
                        return Ok(());
                    }
                }
            }
            Mode::Text if self.input.value().is_empty() => Vec::new(),
            Mode::Text => db
                .search_text_with_limit(self.input.value(), Some(RESULT_LIMIT))?
                .into_iter()
                .map(Hit::Text)
                .collect(),
        };
        self.set_hits(hits);
        Ok(())
    }

    fn set_hits(&mut self, hits: Vec<Hit>) {
        let selected = if let Some(Hit::Particle(previous)) = self.selected_hit() {
            hits.iter().position(|hit| {
                matches!(hit, Hit::Particle(result)
                if particle_key(&result.particle) == particle_key(&previous.particle))
            })
        } else {
            None
        };
        self.results
            .select(selected.or_else(|| (!hits.is_empty()).then_some(0)));
        self.hits = hits;
    }

    fn apply_filters(&mut self, db: &Pdg) {
        let Some(editor) = &mut self.filter_editor else {
            return;
        };
        let result = InlineQuery::parse(self.input.value()).and_then(|query| {
            let draft = editor.draft();
            let filters = draft.apply(db)?;
            let hits = particle_hits(db, &query.name, filters.as_ref(), self.sort)?;
            let mut text = format_query(&query.name, draft);
            let cursor = quote_name(&query.name).len();
            if cursor == 0 && !text.is_empty() {
                // Leave space between a new name query and the first tag.
                text.insert(0, ' ');
            }
            Ok((filters, hits, SearchInput { text, cursor }))
        });
        match result {
            Ok((filters, hits, input)) => {
                self.input = input;
                self.filter_editor = None;
                self.filters = filters;
                self.pending = None;
                self.notice = None;
                self.search_error = None;
                self.set_hits(hits);
            }
            Err(error) => {
                editor.error = Some(error.to_string());
                editor.applying = false;
            }
        }
    }

    fn selected_hit(&self) -> Option<&Hit> {
        self.results
            .selected()
            .and_then(|selected| self.hits.get(selected))
    }

    fn move_result(&mut self, forward: bool, amount: usize) {
        if let Some(selected) = self.results.selected() {
            let index = if forward {
                selected
                    .saturating_add(amount)
                    .min(self.hits.len().saturating_sub(1))
            } else {
                selected.saturating_sub(amount)
            };
            self.results.select(Some(index));
        }
    }

    fn paste(&mut self, db: &Pdg, text: &str) {
        if self.help.is_some() {
            return;
        }
        if let Some(editor) = &mut self.filter_editor {
            editor.paste(db, text);
        } else if self.input.insert(text) {
            self.schedule_search();
            if self.completion.is_some() {
                self.refresh_completion(db);
            }
        }
    }

    fn copy_search(&mut self) -> Action {
        let command = match self.mode {
            Mode::Particles => InlineQuery::parse(self.input.value()).and_then(|query| {
                let draft =
                    (!query.name.is_empty() || self.filters.is_some()).then_some(&query.draft);
                self.source
                    .find_command(self.mode, &query.name, draft, self.sort)
            }),
            Mode::Text => self
                .source
                .find_command(self.mode, self.input.value(), None, self.sort),
        };
        match command {
            Ok(Some(value)) => Action::Copy {
                value,
                notice: "Sent the find command to the terminal clipboard.".into(),
            },
            Ok(None) => {
                self.notice = Some(if self.mode == Mode::Text {
                    "Enter a text query to copy its command.".into()
                } else {
                    "Enter a name or apply a filter to copy its command.".into()
                });
                Action::None
            }
            Err(error) => {
                self.notice = Some(error.to_string());
                Action::None
            }
        }
    }

    fn key(&mut self, db: &Pdg, key: KeyEvent) -> CliResult<Action> {
        if matches!(key.code, KeyCode::Char('c' | 'C'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return Ok(Action::Quit);
        }
        if key.modifiers == KeyModifiers::CONTROL
            && self.mode == Mode::Particles
            && self.detail.is_none()
            && self.filter_editor.is_none()
            && self.help.is_none()
        {
            match key.code {
                KeyCode::Char('f') => {
                    self.notice = None;
                    self.completion = None;
                    match InlineQuery::parse(self.input.value()) {
                        Ok(query) => self.filter_editor = Some(FilterEditor::new(query.draft)),
                        Err(error) => self.search_error = Some(error.to_string()),
                    }
                    return Ok(Action::None);
                }
                KeyCode::Char('o') => {
                    self.notice = None;
                    self.sort = match self.sort {
                        ParticleSort::Default => ParticleSort::Mass,
                        ParticleSort::Mass => ParticleSort::Default,
                    };
                    self.search(db)?;
                    return Ok(Action::None);
                }
                _ => {}
            }
        }
        if !matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) {
            return Ok(Action::None);
        }
        let editing_filter_value = self.mode == Mode::Particles
            && self.detail.is_none()
            && self.filter_editor.is_none()
            && self.help.is_none()
            && matches!(key.code, KeyCode::Char('?' | '/'))
            && query::editing_filter_value(&self.input);
        if key.code == KeyCode::Char('?') && !editing_filter_value {
            self.completion = None;
            self.help = self.help.is_none().then(Help::default);
            return Ok(Action::None);
        }
        if let Some(help) = &mut self.help {
            if key.code == KeyCode::Esc {
                self.help = None;
            } else {
                help.navigate(key.code);
            }
            return Ok(Action::None);
        }
        self.notice = None;
        if let Some(editor) = &mut self.filter_editor {
            return Ok(match editor.key(db, key) {
                EditorAction::Continue => Action::None,
                EditorAction::Cancel => {
                    self.filter_editor = None;
                    Action::None
                }
                EditorAction::Apply => Action::ApplyFilters,
            });
        }
        if key.code == KeyCode::Char('/') && !editing_filter_value {
            self.detail = None;
            self.detail_history.clear();
            return Ok(Action::None);
        }
        self.browse_key(db, key)
    }

    fn refresh_completion(&mut self, db: &Pdg) {
        match query::complete(db, &self.input) {
            Ok(completions) if !completions.suggestions.is_empty() => {
                self.completion = Some((completions, ListState::default().with_selected(Some(0))));
            }
            Ok(_) => self.completion = None,
            Err(error) => {
                self.completion = None;
                self.notice = Some(error.to_string());
            }
        }
    }

    fn completion_key(&mut self, code: KeyCode) -> bool {
        let Some((completion, state)) = &mut self.completion else {
            return false;
        };
        let index = state.selected().unwrap_or_default();
        match code {
            KeyCode::Up => state.select_previous(),
            KeyCode::Down => state.select(Some((index + 1).min(completion.suggestions.len() - 1))),
            KeyCode::Tab if completion.suggestions.len() > 1 => {
                state.select(Some((index + 1) % completion.suggestions.len()));
            }
            KeyCode::Esc => self.completion = None,
            KeyCode::Enter | KeyCode::Tab => {
                completion.accept(&mut self.input, index);
                self.completion = None;
                self.schedule_search();
            }
            _ => return false,
        }
        true
    }

    fn switch_mode(&mut self) {
        let next = self.other_input.take().unwrap_or_else(|| {
            let text = match self.mode {
                Mode::Particles => InlineQuery::parse(self.input.value())
                    .map_or_else(|_| self.input.value().to_owned(), |query| query.name),
                Mode::Text => quote_name(self.input.value()),
            };
            SearchInput {
                cursor: text.len(),
                text,
            }
        });
        self.other_input = Some(std::mem::replace(&mut self.input, next));
        self.mode = match self.mode {
            Mode::Particles => Mode::Text,
            Mode::Text => Mode::Particles,
        };
        self.completion = None;
        self.hits.clear();
        self.results = ListState::default();
        self.schedule_search();
    }

    fn browse_key(&mut self, db: &Pdg, key: KeyEvent) -> CliResult<Action> {
        if let Some(detail) = &mut self.detail {
            match key.code {
                KeyCode::Tab => {
                    detail.cycle_view(db, key.modifiers.contains(KeyModifiers::SHIFT))?;
                }
                KeyCode::BackTab => detail.cycle_view(db, true)?,
                KeyCode::Esc => self.detail = self.detail_history.pop(),
                KeyCode::Enter => {
                    if let Some(pdgid) = detail.selected_entry()
                        && let Some(entry) = Detail::open_entry(db, pdgid)?
                    {
                        self.detail_history.extend(self.detail.replace(entry));
                    }
                }
                KeyCode::Char('c') => return Ok(Action::copy(detail.name_or_id())),
                KeyCode::Char('i') => return Ok(Action::copy(detail.id())),
                KeyCode::Char(key) => detail.select_number(db, key)?,
                code => detail.navigate(code, self.page),
            }
        } else {
            if key.code == KeyCode::BackTab
                || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT))
            {
                self.switch_mode();
                return Ok(Action::None);
            }
            if self.completion_key(key.code) {
                return Ok(Action::None);
            }
            match key.code {
                KeyCode::Esc => return Ok(Action::Quit),
                KeyCode::F(3) => {
                    if self.mode == Mode::Particles && self.pending.is_some() {
                        self.search(db)?;
                    }
                    if self.search_error.is_none() {
                        return Ok(self.copy_search());
                    }
                }
                KeyCode::Tab if self.mode == Mode::Particles => {
                    self.refresh_completion(db);
                    if self
                        .completion
                        .as_ref()
                        .is_some_and(|(items, _)| items.suggestions.len() == 1)
                    {
                        self.completion_key(KeyCode::Enter);
                    }
                }
                KeyCode::Enter => {
                    if self.pending.is_some() {
                        self.search(db)?;
                    }
                    if self.search_error.is_none()
                        && let Some(hit) = self.selected_hit()
                    {
                        self.detail = Detail::open(db, hit)?;
                        if self.detail.is_none() {
                            self.notice = Some("This result has no PDG entry to open.".into());
                        }
                    }
                }
                KeyCode::Up => self.move_result(false, 1),
                KeyCode::Down => self.move_result(true, 1),
                KeyCode::PageUp => self.move_result(false, usize::from(self.page)),
                KeyCode::PageDown => self.move_result(true, usize::from(self.page)),
                _ => {
                    let cursor = self.input.cursor;
                    let changed = self.input.key(key);
                    if changed {
                        self.schedule_search();
                    }
                    if self.completion.is_some() && (changed || cursor != self.input.cursor) {
                        self.refresh_completion(db);
                    }
                }
            }
        }
        Ok(Action::None)
    }

    fn mouse(&mut self, db: &Pdg, mouse: MouseEvent) -> PdgResult<()> {
        let position = Position::new(mouse.column, mouse.row);
        if mouse.kind == MouseEventKind::Down(MouseButton::Left)
            && self.help.is_none()
            && let Some(detail) = &mut self.detail
            && let Some(&(_, view)) = self
                .tab_regions
                .iter()
                .find(|(area, _)| area.contains(position))
            && view != detail.view
        {
            self.notice = None;
            detail.load(db, view)?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame<'_>, db: &Pdg) {
        let area = frame.area();
        self.tab_regions.clear();
        if area.width < 30 || area.height < 10 {
            frame.render_widget(
                Paragraph::new("Resize to at least 30 × 10. Ctrl+C quits.")
                    .wrap(Wrap { trim: true }),
                area,
            );
            return;
        }
        if let Some(help) = &mut self.help {
            help.draw(frame, self.accent);
            return;
        }
        let wide = area.width >= 80;
        let input_rows = if self.detail.is_some() && area.height < 14 {
            0
        } else {
            3
        };
        // Fit three rows of tabs and one row of content inside the detail border.
        let body_rows = if self.detail.is_some() { 6 } else { 3 };
        let footer = self.footer();
        let filters = if self.mode == Mode::Particles {
            self.filters.as_ref().map(|filters| {
                Paragraph::new(format!(" {} ", filters.summary)).wrap(Wrap { trim: true })
            })
        } else {
            None
        };
        let filter_rows = filters
            .as_ref()
            .map_or(0, |bar| if bar.line_count(area.width) > 1 { 2 } else { 1 });
        let footer_rows = u16::try_from(footer.line_count(area.width))
            .unwrap_or(u16::MAX)
            .min(
                area.height
                    .saturating_sub(1 + input_rows + filter_rows + body_rows),
            );
        let [header, input, filter_area, body, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(input_rows),
            Constraint::Length(filter_rows),
            Constraint::Min(1),
            Constraint::Length(footer_rows),
        ])
        .areas(area);
        self.page = body.height.saturating_sub(2).max(1);

        frame.render_widget(
            Line::from(vec![
                Span::styled(" pdg ", self.accent.add_modifier(Modifier::BOLD)),
                Span::raw(format!(
                    "  ·  PDG {}  ·  {}",
                    db.edition(),
                    self.mode.label()
                )),
            ]),
            header,
        );
        if input_rows > 0 {
            self.draw_input(frame, input);
        }
        if let Some(filters) = filters {
            frame.render_widget(filters, filter_area);
        }
        if wide {
            let [results, details] =
                Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)])
                    .areas(body);
            self.draw_results(frame, results);
            self.draw_detail(frame, details);
        } else if self.detail.is_some() {
            self.draw_detail(frame, body);
        } else {
            self.draw_results(frame, body);
        }
        frame.render_widget(footer, footer_area);
        self.draw_completion(frame, input, area);
        if let Some(editor) = &mut self.filter_editor {
            editor.draw(frame, area, self.accent);
        }
    }

    fn draw_completion(&mut self, frame: &mut Frame<'_>, input: Rect, area: Rect) {
        let Some((completion, state)) = &mut self.completion else {
            return;
        };
        let height = u16::try_from(completion.suggestions.len() + 2)
            .unwrap_or(u16::MAX)
            .min(7)
            .min(area.bottom().saturating_sub(input.bottom()));
        let bounds = Rect::new(input.x, input.bottom(), input.width.min(46), height);
        let items = completion
            .suggestions
            .iter()
            .map(|choice| ListItem::new(choice.label.as_str()));
        frame.render_widget(Clear, bounds);
        frame.render_stateful_widget(
            List::new(items)
                .block(
                    Block::bordered()
                        .title(" ↑↓  Enter Fill  Esc ")
                        .border_style(self.accent),
                )
                .highlight_symbol("› ")
                .highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
            bounds,
            state,
        );
    }

    fn draw_input(&self, frame: &mut Frame<'_>, area: Rect) {
        let block =
            Block::bordered().title(format!(" Search {} ", self.mode.label().to_lowercase()));
        let focused = self.detail.is_none() && self.filter_editor.is_none();
        let block = if focused {
            block.border_style(self.accent)
        } else {
            block
        };
        let inner = block.inner(area);
        frame.render_widget(block, area);
        self.input.draw(frame, inner, "", focused);
    }

    fn draw_results(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let order = if self.mode == Mode::Particles && self.sort == ParticleSort::Mass {
            " · Mass ↑"
        } else {
            ""
        };
        let block = Block::bordered().title(format!(" Results · {}{order} ", self.hits.len()));
        let items = self.hits.iter().map(|hit| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}  ", hit.pdgid()), self.accent),
                Span::raw(hit.title()),
            ]))
        });
        frame.render_stateful_widget(
            List::new(items)
                .block(block)
                .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
                .highlight_symbol("› "),
            area,
            &mut self.results,
        );
    }

    fn draw_detail(&mut self, frame: &mut Frame<'_>, area: Rect) {
        if let Some(detail) = &mut self.detail {
            self.page = detail.draw(frame, area, self.accent, &mut self.tab_regions);
        } else {
            let content = self.selected_hit().map_or_else(
                || vec![Line::raw("Select a search result to preview it.")],
                Hit::preview,
            );
            frame.render_widget(
                Paragraph::new(content)
                    .wrap(Wrap { trim: true })
                    .block(Block::bordered().title(" Preview ")),
                area,
            );
        }
    }

    fn footer(&self) -> Paragraph<'static> {
        let mut lines = Vec::new();
        let status = self.status();
        if !status.is_empty() {
            lines.push(Line::raw(status));
        }
        if let Some(detail) = &self.detail {
            lines.push(Line::from(vec![
                Span::styled("? Help  / Search", self.accent),
                Span::raw("  Esc Back"),
            ]));
            lines.push(Line::raw("Tab/Shift+Tab Views"));
            let navigation = match detail.view {
                View::Matches => "↑↓ Select  Enter Open channel",
                View::Links => "↑↓ Select  Enter Follow link",
                _ => "↑↓ Scroll  PgUp/PgDn Page",
            };
            lines.push(Line::raw(format!("{navigation}  c Name  i ID")));
        } else {
            let particle_controls = if self.mode == Mode::Particles {
                "Ctrl+F Filters  Ctrl+O Sort  "
            } else {
                ""
            };
            lines.push(Line::raw(format!(
                "? Help  {particle_controls}F3 Copy search  Shift+Tab Switch search  ↑↓ Select  Enter Open  Esc Quit"
            )));
        }
        Paragraph::new(lines).wrap(Wrap { trim: true })
    }

    fn status(&self) -> String {
        if let Some(notice) = &self.notice {
            return notice.clone();
        }
        if self.detail.is_some() {
            return String::new();
        }
        if let Some(error) = &self.search_error {
            return error.clone();
        }
        if self.input.value().is_empty() {
            return match self.mode {
                Mode::Particles => "Try f980 mass:900..1100.".into(),
                Mode::Text => "Search PDG descriptions, text, and footnotes.".into(),
            };
        }
        if self.pending.is_some() {
            return "Searching…".into();
        }
        if self.hits.is_empty() {
            return "No matches.".into();
        }
        if let Some(Hit::Text(hit)) = self.hits.first()
            && let Some(correction) = &hit.correction
        {
            return format!("Showing matches for {:?}.", correction.replacement);
        }
        String::new()
    }
}

#[derive(Clone, Default)]
struct SearchInput {
    text: String,
    cursor: usize,
}

impl SearchInput {
    fn value(&self) -> &str {
        self.text.trim()
    }

    fn draw(&self, frame: &mut Frame<'_>, area: Rect, placeholder: &str, focused: bool) {
        let before_cursor = &self.text[..self.cursor];
        let column = u16::try_from(Line::raw(before_cursor).width()).unwrap_or(u16::MAX);
        let offset = column.saturating_sub(area.width.saturating_sub(1));
        let line = if self.text.is_empty() {
            Line::raw(placeholder).dim()
        } else {
            Line::raw(self.text.as_str())
        };
        frame.render_widget(Paragraph::new(line).scroll((0, offset)), area);
        if focused && area.width > 0 && area.height > 0 {
            frame.set_cursor_position((area.x + (column - offset), area.y));
        }
    }
    fn insert(&mut self, text: &str) -> bool {
        let text = text
            .chars()
            .map(|ch| {
                if ch.is_whitespace() && ch.is_control() {
                    ' '
                } else {
                    ch
                }
            })
            .filter(|ch| !ch.is_control())
            .collect::<String>();
        self.text.insert_str(self.cursor, &text);
        self.cursor += text.len();
        !text.is_empty()
    }

    fn key(&mut self, key: KeyEvent) -> bool {
        let left = || {
            self.text[..self.cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(index, _)| index)
        };
        let right = || {
            self.cursor
                + self.text[self.cursor..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8)
        };
        match key.code {
            KeyCode::Char(ch)
                if matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) =>
            {
                return self.insert(ch.encode_utf8(&mut [0; 4]));
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let start = left();
                self.text.drain(start..self.cursor);
                self.cursor = start;
                return true;
            }
            KeyCode::Delete if self.cursor < self.text.len() => {
                let end = right();
                self.text.drain(self.cursor..end);
                return true;
            }
            KeyCode::Left => self.cursor = left(),
            KeyCode::Right => self.cursor = right(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn test_db() -> Pdg {
        Pdg::open_path(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/pdgall-2025-v0.2.2.sqlite",
        ))
        .unwrap()
    }

    #[test]
    fn browser_retains_the_ranked_result_when_returning_from_a_detail() {
        let db = test_db();
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let mut app = App::new("f980", false, SearchSource::default());
        app.search(&db).unwrap();
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));

        app.key(&db, key(KeyCode::Enter)).unwrap();
        app.key(&db, key(KeyCode::Char('2'))).unwrap();
        assert!(matches!(app.detail.as_ref().unwrap().view, View::Mass));
        assert!(matches!(
            app.key(&db, key(KeyCode::Char('c'))).unwrap(),
            Action::Copy { value, .. } if value == "f_0(980)0"
        ));
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert!(app.detail.is_none());
        assert_eq!(app.input.text, "f980");
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));
    }

    fn search_with_kaon_filters(db: &Pdg) -> App {
        let mut app = App::new("", false, SearchSource::default());
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        app.key(db, KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL))
            .unwrap();
        app.key(db, key(KeyCode::Char(' '))).unwrap();
        app.key(db, key(KeyCode::End)).unwrap();
        app.key(db, key(KeyCode::Up)).unwrap();
        assert!(matches!(
            app.key(db, key(KeyCode::Enter)).unwrap(),
            Action::None
        ));
        app.key(db, key(KeyCode::Tab)).unwrap();
        app.key(db, key(KeyCode::Tab)).unwrap();
        for _ in 0..5 {
            app.key(db, key(KeyCode::Right)).unwrap();
        }
        for _ in 0..7 {
            app.key(db, key(KeyCode::Tab)).unwrap();
        }
        app.paste(db, "900..2000");
        for _ in 0..5 {
            app.key(db, key(KeyCode::Tab)).unwrap();
        }
        app.paste(db, "KS,KS");
        assert!(matches!(
            app.key(db, key(KeyCode::Enter)).unwrap(),
            Action::ApplyFilters
        ));
        app.apply_filters(db);
        assert!(app.filter_editor.is_none());
        app
    }

    #[test]
    fn filters_support_blank_queries_and_open_the_recorded_channel() {
        let db = test_db();
        let mut app = search_with_kaon_filters(&db);
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        assert_eq!(app.hits.len(), 27);
        assert_eq!(
            app.input.value(),
            "class:meson charge:0 mass:900..2000 decays-to:KS,KS"
        );
        assert!(matches!(
            app.key(&db, key(KeyCode::F(3))).unwrap(),
            Action::Copy { value, .. }
                if value == "pdg find --limit=50 --class=meson --charge=0 --mass=900..2000 --decays-to=KS,KS"
        ));

        app.paste(&db, "f980");
        app.search(&db).unwrap();
        let cli = db
            .find_particles(
                "f980",
                ParticleSearchQuery::new()
                    .class(pdg_rs::ParticleClass::Meson)
                    .charge(pdg_rs::Charge::Neutral)
                    .mass_range_mev(900.0, 2000.0)
                    .decays_to(["KS", "KS"]),
            )
            .unwrap();
        assert_eq!(
            app.hits.iter().map(Hit::pdgid).collect::<Vec<_>>(),
            cli.iter()
                .map(|hit| hit.particle.pdgid.as_str())
                .collect::<Vec<_>>()
        );

        app.key(&db, key(KeyCode::Enter)).unwrap();
        assert_eq!(
            app.detail.as_ref().unwrap().selected_entry(),
            Some("M003.1")
        );
        app.key(&db, key(KeyCode::Enter)).unwrap();
        assert_eq!(app.detail.as_ref().unwrap().id(), "M003.1");
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert_eq!(
            app.detail.as_ref().unwrap().selected_entry(),
            Some("M003.1")
        );
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert!(app.detail.is_none());

        app.key(
            &db,
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
        )
        .unwrap();
        for _ in 0..9 {
            app.key(&db, key(KeyCode::Tab)).unwrap();
        }
        app.paste(&db, "invalid");
        app.apply_filters(&db);
        assert!(app.filter_editor.as_ref().unwrap().error.is_some());
        app.key(&db, key(KeyCode::Char('?'))).unwrap();
        assert!(app.help.is_some());
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert!(app.filter_editor.as_ref().unwrap().error.is_some());
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));

        app.key(&db, KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT))
            .unwrap();
        assert!(app.mode == Mode::Text && app.filters.is_some());
        assert!(matches!(
            app.key(&db, key(KeyCode::F(3))).unwrap(),
            Action::Copy { value, .. } if value == "pdg find --limit=50 --text f980"
        ));
        app.key(&db, KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT))
            .unwrap();
        app.search(&db).unwrap();
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));
    }

    #[test]
    fn parity_filters_apply_to_particle_names_and_copy_as_cli_options() {
        let db = test_db();
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let mut app = App::new("pi", false, SearchSource::default());
        app.search(&db).unwrap();
        app.key(
            &db,
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
        )
        .unwrap();
        for _ in 0..6 {
            app.key(&db, key(KeyCode::Tab)).unwrap();
        }
        app.key(&db, key(KeyCode::Right)).unwrap();
        app.key(&db, key(KeyCode::Right)).unwrap();
        app.key(&db, key(KeyCode::Tab)).unwrap();
        app.key(&db, key(KeyCode::Right)).unwrap();
        assert!(matches!(
            app.key(&db, key(KeyCode::Enter)).unwrap(),
            Action::ApplyFilters
        ));
        app.apply_filters(&db);
        let cli = db
            .find_particles(
                "pi",
                ParticleSearchQuery::new()
                    .parity(pdg_rs::Parity::Minus)
                    .charge_conjugation(pdg_rs::Parity::Plus),
            )
            .unwrap();
        assert!(!cli.is_empty());
        assert!(
            app.hits.iter().map(Hit::pdgid).eq(cli
                .iter()
                .take(RESULT_LIMIT)
                .map(|hit| hit.particle.pdgid.as_str()))
        );
        assert!(matches!(
            app.key(&db, key(KeyCode::F(3))).unwrap(),
            Action::Copy { value, .. }
                if value == "pdg find --limit=50 --parity=- --c-parity=+ pi"
        ));
    }

    #[test]
    fn inline_search_keeps_last_good_results_and_exports_the_resolved_query() {
        let db = test_db();
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let mut app = App::new("f980", false, SearchSource::default());
        app.search(&db).unwrap();
        app.paste(&db, " mass:");
        app.search(&db).unwrap();
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));
        assert!(app.status().contains("Enter a value for mass"));
        assert!(matches!(
            app.key(&db, key(KeyCode::F(3))).unwrap(),
            Action::None
        ));
        app.key(&db, key(KeyCode::Enter)).unwrap();
        assert!(app.detail.is_none());
        app.paste(&db, "900..1100 decays-to:KS,KS");
        app.search(&db).unwrap();
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));
        assert!(
            matches!(app.key(&db, key(KeyCode::F(3))).unwrap(), Action::Copy { value, .. }
            if value == "pdg find --limit=50 --mass=900..1100 --decays-to=KS,KS f980")
        );

        app.key(
            &db,
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
        )
        .unwrap();
        app.apply_filters(&db);
        assert_eq!(app.input.value(), "f980 mass:900..1100 decays-to:KS,KS");
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));
    }

    #[test]
    fn search_bar_completes_a_tag_and_each_repeated_decay_product() {
        let db = test_db();
        let complete = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let mut app = App::new("decays-t", false, SearchSource::default());
        app.key(&db, complete).unwrap();
        assert!(app.completion.is_none());
        assert_eq!(app.input.value(), "decays-to:");
        for text in ["KS", ",KS"] {
            app.paste(&db, text);
            app.key(&db, complete).unwrap();
            if app.completion.is_some() {
                app.key(&db, enter).unwrap();
            }
        }
        assert_eq!(app.input.value(), "decays-to:K(S)0,K(S)0");
        app.search(&db).unwrap();
        assert!(!app.hits.is_empty());
        assert!(app.search_error.is_none());
    }

    #[test]
    fn tab_cycles_multiple_tag_matches_and_shift_tab_switches_modes() {
        let db = test_db();
        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        let mut app = App::new("decays-", false, SearchSource::default());
        app.key(&db, tab).unwrap();
        assert!(app.completion.is_some());
        assert_eq!(app.input.value(), "decays-");
        app.key(&db, tab).unwrap();
        app.key(&db, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .unwrap();
        assert_eq!(app.input.value(), "decays-from:");
        app.key(&db, tab).unwrap();
        assert!(app.mode == Mode::Particles && app.completion.is_none());
        app.key(&db, KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT))
            .unwrap();
        assert!(app.mode == Mode::Text && app.completion.is_none());
    }

    #[test]
    fn mass_order_sorts_filtered_results_and_exports_the_selection() {
        let db = test_db();
        let mut app = search_with_kaon_filters(&db);
        let sort = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
        let copy = KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE);
        assert_eq!(app.hits.first().map(Hit::pdgid), Some("M003"));

        app.key(&db, sort).unwrap();
        assert_eq!(app.hits.first().map(Hit::pdgid), Some("M036"));
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));
        assert!(
            matches!(app.key(&db, copy).unwrap(), Action::Copy { value, .. }
                if value == "pdg find --limit=50 --sort=mass --class=meson --charge=0 --mass=900..2000 --decays-to=KS,KS"
            )
        );
        app.paste(&db, "980");
        app.search(&db).unwrap();
        assert_eq!(app.hits.first().map(Hit::pdgid), Some("M036"));

        app.key(&db, sort).unwrap();
        assert_eq!(app.hits.first().map(Hit::pdgid), Some("M003"));
        assert!(
            matches!(app.key(&db, copy).unwrap(), Action::Copy { value, .. }
                if !value.contains("--sort")
            )
        );
    }

    #[test]
    fn following_entry_links_restores_each_detail_and_the_filtered_search() {
        let db = test_db();
        let mut app = search_with_kaon_filters(&db);
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        app.paste(&db, "f980");
        app.search(&db).unwrap();
        app.key(&db, key(KeyCode::Enter)).unwrap();
        app.key(&db, key(KeyCode::Enter)).unwrap();
        assert_eq!(app.detail.as_ref().unwrap().id(), "M003.1");

        app.key(&db, key(KeyCode::Char('2'))).unwrap();
        assert_eq!(app.detail.as_ref().unwrap().selected_entry(), Some("M003"));
        app.key(&db, key(KeyCode::Down)).unwrap();
        assert_eq!(
            app.detail.as_ref().unwrap().selected_entry(),
            Some("M003R1")
        );
        app.key(&db, key(KeyCode::Enter)).unwrap();
        assert_eq!(app.detail.as_ref().unwrap().id(), "M003R1");
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert_eq!(
            app.detail.as_ref().unwrap().selected_entry(),
            Some("M003R1")
        );
        app.key(&db, key(KeyCode::Up)).unwrap();
        app.key(&db, key(KeyCode::Enter)).unwrap();
        assert_eq!(app.detail.as_ref().unwrap().id(), "M003");

        app.key(&db, key(KeyCode::Char('6'))).unwrap();
        app.key(&db, key(KeyCode::Down)).unwrap();
        assert_eq!(
            app.detail.as_ref().unwrap().selected_entry(),
            Some("M003M1")
        );
        let backend = ratatui::backend::TestBackend::new(45, 5);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| app.draw_detail(frame, frame.area()))
            .unwrap();
        let parent_view = terminal.backend().buffer().clone();

        app.key(&db, key(KeyCode::Enter)).unwrap();
        assert_eq!(app.detail.as_ref().unwrap().id(), "M003M1");
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert_eq!(app.detail.as_ref().unwrap().id(), "M003");
        assert_eq!(
            app.detail.as_ref().unwrap().selected_entry(),
            Some("M003M1")
        );
        terminal
            .draw(|frame| app.draw_detail(frame, frame.area()))
            .unwrap();
        assert_eq!(terminal.backend().buffer(), &parent_view);

        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert_eq!(app.detail.as_ref().unwrap().id(), "M003.1");
        assert_eq!(app.detail.as_ref().unwrap().selected_entry(), Some("M003"));
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert_eq!(
            app.detail.as_ref().unwrap().selected_entry(),
            Some("M003.1")
        );
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert!(app.detail.is_none());
        assert!(app.detail_history.is_empty());
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));
        assert_eq!(
            app.input.value(),
            "f980 class:meson charge:0 mass:900..2000 decays-to:KS,KS"
        );
        assert!(app.filters.is_some());
    }

    #[test]
    fn detail_tabs_are_visible_and_switch_directly() {
        let db = test_db();
        for (width, height) in [(100, 26), (48, 16), (30, 10)] {
            let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
            let mut app = App::new("f980", false, SearchSource::default());
            app.search(&db).unwrap();
            app.key(&db, key(KeyCode::Enter)).unwrap();
            let backend = ratatui::backend::TestBackend::new(width, height);
            let mut terminal = ratatui::Terminal::new(backend).unwrap();
            terminal.draw(|frame| app.draw(frame, &db)).unwrap();

            let tab_area = |view| {
                app.tab_regions
                    .iter()
                    .find(|(_, candidate)| *candidate == view)
                    .unwrap()
                    .0
            };
            let overview = tab_area(View::Overview);
            let links = tab_area(View::Links);
            assert_eq!(app.tab_regions.len(), 6);
            let buffer = terminal.backend().buffer();
            let active = buffer.cell((overview.x, overview.y)).unwrap();
            assert_eq!(active.symbol(), "1");
            assert!(active.modifier.contains(Modifier::REVERSED));
            let link_cell = buffer.cell((links.x, links.y)).unwrap();
            assert_eq!(link_cell.symbol(), "6");
            assert!(!link_cell.modifier.contains(Modifier::REVERSED));
            if width >= 80 {
                assert!(overview.x > width / 3);
            }

            app.key(&db, key(KeyCode::Tab)).unwrap();
            assert!(matches!(app.detail.as_ref().unwrap().view, View::Mass));
            app.key(&db, KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT))
                .unwrap();
            assert!(matches!(app.detail.as_ref().unwrap().view, View::Overview));
            app.key(&db, KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT))
                .unwrap();
            assert!(matches!(app.detail.as_ref().unwrap().view, View::Links));
            app.key(&db, key(KeyCode::Tab)).unwrap();
            assert!(matches!(app.detail.as_ref().unwrap().view, View::Overview));

            app.mouse(
                &db,
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: links.right() - 1,
                    row: links.y,
                    modifiers: KeyModifiers::NONE,
                },
            )
            .unwrap();
            assert!(matches!(app.detail.as_ref().unwrap().view, View::Links));
            terminal.draw(|frame| app.draw(frame, &db)).unwrap();
            let selected_entry = app.detail.as_ref().unwrap().selected_entry().unwrap();
            let screen: String = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect();
            assert!(screen.contains(selected_entry));
            app.key(&db, key(KeyCode::Char('2'))).unwrap();
            assert!(matches!(app.detail.as_ref().unwrap().view, View::Mass));
            app.key(&db, key(KeyCode::Esc)).unwrap();
            assert!(app.detail.is_none());
        }
    }

    #[test]
    fn detail_tabs_include_the_views_available_for_each_result() {
        let db = test_db();
        let cases = [
            (search_with_kaon_filters(&db), View::Matches, View::Text, 7),
            (
                App::new("form factors", true, SearchSource::default()),
                View::Text,
                View::Matches,
                5,
            ),
        ];
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let backend = ratatui::backend::TestBackend::new(30, 10);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        for (mut app, initial, unavailable, tab_count) in cases {
            app.search(&db).unwrap();
            app.key(&db, key(KeyCode::Enter)).unwrap();
            terminal.draw(|frame| app.draw(frame, &db)).unwrap();
            assert!(
                app.detail
                    .as_ref()
                    .is_some_and(|detail| detail.view == initial)
            );
            assert_eq!(app.tab_regions.len(), tab_count);
            assert!(app.tab_regions.iter().any(|&(_, view)| view == initial));
            assert!(!app.tab_regions.iter().any(|&(_, view)| view == unavailable));
            assert!(app.tab_regions.iter().any(|&(_, view)| view == View::Links));
        }
    }

    #[test]
    fn branching_ratio_uses_its_overview_and_omits_mass_and_decays() {
        let db = test_db().with_edition("1997").unwrap();
        let mut app = App::new("f980", false, SearchSource::default());
        app.detail = Detail::open_entry(&db, "M003R1").unwrap();
        let backend = ratatui::backend::TestBackend::new(120, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame, &db)).unwrap();

        assert!(app.tab_regions.iter().map(|&(_, view)| view).eq([
            View::Overview,
            View::Measurements,
            View::References,
            View::Links,
        ]));
        let screen: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(screen.contains(&db.data_for("M003R1").unwrap()[0].to_string()));
        assert!(screen.contains("1 Overview  2 Meas.  3 Refs  4 Links"));

        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        app.key(&db, key(KeyCode::Char('0'))).unwrap();
        app.key(&db, key(KeyCode::Char('5'))).unwrap();
        assert!(matches!(app.detail.as_ref().unwrap().view, View::Overview));
        app.key(&db, key(KeyCode::Char('2'))).unwrap();
        assert!(matches!(
            app.detail.as_ref().unwrap().view,
            View::Measurements
        ));
        app.key(&db, key(KeyCode::Char('3'))).unwrap();
        assert!(matches!(
            app.detail.as_ref().unwrap().view,
            View::References
        ));
        app.key(&db, key(KeyCode::Char('4'))).unwrap();
        assert!(matches!(app.detail.as_ref().unwrap().view, View::Links));
    }

    #[test]
    fn help_preserves_detail_navigation_and_slash_focuses_the_search_bar() {
        let db = test_db();
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let mut app = App::new("f980", false, SearchSource::default());
        app.search(&db).unwrap();
        app.key(&db, key(KeyCode::Enter)).unwrap();
        app.key(&db, key(KeyCode::Char('6'))).unwrap();
        app.key(&db, key(KeyCode::Down)).unwrap();
        app.key(&db, key(KeyCode::Enter)).unwrap();
        assert_eq!(app.detail.as_ref().unwrap().id(), "M003M1");

        app.key(&db, KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT))
            .unwrap();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(30, 10)).unwrap();
        let mut screen = |app: &mut App| {
            terminal.draw(|frame| app.draw(frame, &db)).unwrap();
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
        };
        assert!(screen(&mut app).contains("Focus the search bar"));
        assert!(app.tab_regions.is_empty());
        app.key(&db, key(KeyCode::End)).unwrap();
        assert!(screen(&mut app).contains("Ctrl+C"));
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert!(app.help.is_none());
        assert_eq!(app.detail.as_ref().unwrap().id(), "M003M1");

        app.key(&db, key(KeyCode::Char('/'))).unwrap();
        assert!(app.detail.is_none());
        assert!(app.detail_history.is_empty());
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));
        assert_eq!(app.input.value(), "f980");
        app.key(&db, key(KeyCode::Char('x'))).unwrap();
        assert_eq!(app.input.value(), "f980x");
    }

    #[test]
    fn search_input_edits_unicode_at_character_boundaries() {
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let mut input = SearchInput::default();
        input.insert("π⁺\nρ");
        input.key(key(KeyCode::Home));
        input.key(key(KeyCode::Right));
        input.key(key(KeyCode::Delete));
        input.key(key(KeyCode::Backspace));
        assert_eq!(input.text, " ρ");
        assert_eq!(input.cursor, 0);
    }
}
