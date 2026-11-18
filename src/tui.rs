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
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
};
use pdg_rs::{
    OwnedDecayChannelMatch, ParticleSearchQuery, ParticleSummary, Pdg, PdgResult, TextSearchResult,
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{CliError, CliResult, DatabaseOptions, TuiCommand, open_pdg};

mod command;
mod detail;
mod filters;
use command::SearchSource;
use detail::{Detail, View};
use filters::{AppliedFilters, EditorAction, FilterEditor};

const RESULT_LIMIT: usize = 50;
const SEARCH_DELAY: Duration = Duration::from_millis(120);

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
    let result = (|| {
        execute!(io::stdout(), EnableBracketedPaste)?;
        drive(&mut terminal, &mut app, &db)
    })();
    let reset = execute!(io::stdout(), DisableBracketedPaste, Show);
    let restore = ratatui::try_restore();
    result?;
    reset?;
    restore?;
    Ok(())
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
            Event::Paste(text) if app.detail.is_none() => app.paste(&text),
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

fn particle_hits(db: &Pdg, name: &str, filters: Option<&AppliedFilters>) -> PdgResult<Vec<Hit>> {
    if name.is_empty() {
        return Ok(filters
            .into_iter()
            .flat_map(|filters| &filters.hits)
            .take(RESULT_LIMIT)
            .map(|hit| {
                Hit::Particle(ParticleHit {
                    particle: hit.particle.clone(),
                    matched_name: None,
                    matching_decays: hit.matching_decays.clone(),
                })
            })
            .collect());
    }

    // The physics query is independent of name rank. Intersect the ranked results
    // with the cached physics matches before applying the browser's result limit.
    let eligible = filters.map(|filters| {
        filters
            .hits
            .iter()
            .map(|hit| (particle_key(&hit.particle), &hit.matching_decays))
            .collect::<HashMap<_, _>>()
    });
    Ok(db
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
                    .copied()?
                    .clone(),
                None => Vec::new(),
            };
            Some(Hit::Particle(ParticleHit {
                particle: ParticleSummary::from(&hit.particle),
                matched_name: Some(hit.matched_name),
                matching_decays,
            }))
        })
        .take(RESULT_LIMIT)
        .collect())
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
    mode: Mode,
    source: SearchSource,
    hits: Vec<Hit>,
    results: ListState,
    detail: Option<Detail>,
    previous_detail: Option<Detail>,
    filters: Option<AppliedFilters>,
    filter_editor: Option<FilterEditor>,
    pending: Option<Instant>,
    notice: Option<String>,
    page: u16,
    accent: Style,
}

impl App {
    fn new(query: &str, text: bool, source: SearchSource) -> Self {
        let mut input = SearchInput::default();
        input.insert(query);
        Self {
            input,
            mode: if text { Mode::Text } else { Mode::Particles },
            source,
            hits: Vec::new(),
            results: ListState::default(),
            detail: None,
            previous_detail: None,
            filters: None,
            filter_editor: None,
            pending: Some(Instant::now()),
            notice: None,
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
        self.hits.clear();
        self.results = ListState::default();
        self.notice = None;
    }

    fn search(&mut self, db: &Pdg) -> CliResult<()> {
        self.pending = None;
        let name = self.input.value();
        let hits = match self.mode {
            Mode::Particles => particle_hits(db, name, self.filters.as_ref())?,
            Mode::Text if name.is_empty() => Vec::new(),
            Mode::Text => db
                .search_text_with_limit(name, Some(RESULT_LIMIT))?
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
        let result = editor.apply(db).and_then(|filters| {
            let hits = particle_hits(db, self.input.value(), filters.as_ref())?;
            Ok((filters, hits))
        });
        match result {
            Ok((filters, hits)) => {
                self.filter_editor = None;
                self.filters = filters;
                self.pending = None;
                self.notice = None;
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

    fn paste(&mut self, text: &str) {
        if let Some(editor) = &mut self.filter_editor {
            editor.paste(text);
        } else if self.input.insert(text) {
            self.schedule_search();
        }
    }

    fn copy_search(&mut self) -> Action {
        match self
            .source
            .find_command(self.mode, self.input.value(), self.filters.as_ref())
        {
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
        if !matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) {
            return Ok(Action::None);
        }
        self.notice = None;
        if let Some(editor) = &mut self.filter_editor {
            return Ok(match editor.key(key) {
                EditorAction::Continue => Action::None,
                EditorAction::Cancel => {
                    self.filter_editor = None;
                    Action::None
                }
                EditorAction::Apply => Action::ApplyFilters,
            });
        }
        if let Some(detail) = &mut self.detail {
            match key.code {
                KeyCode::Esc => self.detail = self.previous_detail.take(),
                KeyCode::Enter => {
                    if let Some(pdgid) = detail.selected_channel()
                        && let Some(channel) = Detail::open_channel(db, pdgid)?
                    {
                        self.previous_detail = self.detail.replace(channel);
                    }
                }
                KeyCode::Char('c') => return Ok(Action::copy(detail.name_or_id())),
                KeyCode::Char('i') => return Ok(Action::copy(detail.id())),
                KeyCode::Char(key) => {
                    if let Some(view) = View::from_key(key) {
                        detail.load(db, view)?;
                    }
                }
                code => detail.navigate(code, self.page),
            }
        } else {
            match key.code {
                KeyCode::Esc => return Ok(Action::Quit),
                KeyCode::F(2) if self.mode == Mode::Particles => {
                    self.filter_editor = Some(FilterEditor::new(self.filters.as_ref()));
                }
                KeyCode::F(3) => return Ok(self.copy_search()),
                KeyCode::Tab => {
                    self.mode = if self.mode == Mode::Particles {
                        Mode::Text
                    } else {
                        Mode::Particles
                    };
                    self.schedule_search();
                }
                KeyCode::Enter => {
                    if self.pending.is_some() {
                        self.search(db)?;
                    }
                    if let Some(hit) = self.selected_hit() {
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
                    if self.input.key(key) {
                        self.schedule_search();
                    }
                }
            }
        }
        Ok(Action::None)
    }

    fn draw(&mut self, frame: &mut Frame<'_>, db: &Pdg) {
        let area = frame.area();
        if area.width < 30 || area.height < 10 {
            frame.render_widget(
                Paragraph::new("Resize to at least 30 × 10. Ctrl+C quits.")
                    .wrap(Wrap { trim: true }),
                area,
            );
            return;
        }
        let wide = area.width >= 80;
        let footer = self.footer(area.width);
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
            .min(area.height.saturating_sub(7 + filter_rows));
        let [header, input, filter_area, body, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
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
        self.draw_input(frame, input);
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
        if let Some(editor) = &mut self.filter_editor {
            editor.draw(frame, area, self.accent);
        }
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
        let block = Block::bordered().title(format!(" Results · {} ", self.hits.len()));
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
            if detail.view == View::Matches {
                detail.draw_matches(frame, area, self.accent);
                return;
            }
            let block = Block::bordered()
                .title(format!(" {} · {} ", detail.id(), detail.view.label()))
                .border_style(self.accent);
            let inner = block.inner(area);
            let paragraph =
                Paragraph::new(Text::from(detail.lines.clone())).wrap(Wrap { trim: true });
            let max_scroll = paragraph
                .line_count(inner.width)
                .saturating_sub(usize::from(inner.height));
            detail.scroll = detail
                .scroll
                .min(u16::try_from(max_scroll).unwrap_or(u16::MAX));
            frame.render_widget(paragraph.scroll((detail.scroll, 0)).block(block), area);
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

    fn footer(&self, width: u16) -> Paragraph<'static> {
        let mut lines = Vec::new();
        let status = self.status();
        if !status.is_empty() {
            lines.push(Line::raw(status));
        }
        if let Some(detail) = &self.detail {
            let compact = width < 64;
            if detail.view == View::Matches {
                lines.push(Line::raw("↑↓ Select  Enter Open channel  Esc Back"));
            }
            let mut views = if compact {
                lines.push(Line::styled("1 Overview  2 Mass  3 Decays", self.accent));
                "4 Measures  5 Refs".to_owned()
            } else {
                "1 Overview  2 Mass  3 Decays  4 Measurements  5 References".to_owned()
            };
            if detail.has_text() {
                views.push_str("  0 Text");
            }
            if detail.has_matches() {
                views.push_str("  6 Matches");
            }
            lines.push(Line::styled(views, self.accent));
            if detail.view != View::Matches {
                lines.push(Line::raw(if compact {
                    "c Name  i ID  Esc Back"
                } else {
                    "↑↓ Scroll  PgUp/PgDn Page  c Copy name  i Copy ID  Esc Back"
                }));
            }
        } else {
            let filters = if self.mode == Mode::Particles {
                "F2 Filters  "
            } else {
                ""
            };
            lines.push(Line::raw(format!(
                "{filters}F3 Copy search  Tab Switch search  ↑↓ Select  Enter Open  Esc Quit"
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
        if self.input.value().is_empty() && (self.mode == Mode::Text || self.filters.is_none()) {
            return match self.mode {
                Mode::Particles => "Type a name or alias, such as f980.".into(),
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

    fn test_db() -> Pdg {
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

    #[test]
    fn filters_support_blank_queries_and_open_the_recorded_channel() {
        let db = test_db();
        let mut app = App::new("", false, SearchSource::default());
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        app.key(&db, key(KeyCode::F(2))).unwrap();
        app.key(&db, key(KeyCode::Char(' '))).unwrap();
        app.key(&db, key(KeyCode::End)).unwrap();
        app.key(&db, key(KeyCode::Up)).unwrap();
        assert!(matches!(
            app.key(&db, key(KeyCode::Enter)).unwrap(),
            Action::None
        ));
        app.key(&db, key(KeyCode::Tab)).unwrap();
        app.key(&db, key(KeyCode::Tab)).unwrap();
        for _ in 0..5 {
            app.key(&db, key(KeyCode::Right)).unwrap();
        }
        app.key(&db, key(KeyCode::Tab)).unwrap();
        app.key(&db, key(KeyCode::Tab)).unwrap();
        app.paste("900..2000");
        app.key(&db, key(KeyCode::Tab)).unwrap();
        app.key(&db, key(KeyCode::Tab)).unwrap();
        app.paste("KS,KS");
        assert!(matches!(
            app.key(&db, key(KeyCode::Enter)).unwrap(),
            Action::ApplyFilters
        ));
        app.apply_filters(&db);
        assert!(app.filter_editor.is_none());
        assert_eq!(app.hits.len(), 27);
        assert_eq!(app.input.value(), "");
        assert!(matches!(
            app.key(&db, key(KeyCode::F(3))).unwrap(),
            Action::Copy { value, .. }
                if value == "pdg find --limit=50 --class=meson --charge=0 --mass=900..2000 --decays-to=KS,KS"
        ));

        app.paste("f980");
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
            app.detail.as_ref().unwrap().selected_channel(),
            Some("M003.1")
        );
        app.key(&db, key(KeyCode::Enter)).unwrap();
        assert_eq!(app.detail.as_ref().unwrap().id(), "M003.1");
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert_eq!(
            app.detail.as_ref().unwrap().selected_channel(),
            Some("M003.1")
        );
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert!(app.detail.is_none());

        app.key(&db, key(KeyCode::F(2))).unwrap();
        for _ in 0..4 {
            app.key(&db, key(KeyCode::Tab)).unwrap();
        }
        app.paste("invalid");
        app.apply_filters(&db);
        assert!(app.filter_editor.as_ref().unwrap().error.is_some());
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));

        app.key(&db, key(KeyCode::Tab)).unwrap();
        assert!(app.mode == Mode::Text && app.filters.is_some());
        assert!(matches!(
            app.key(&db, key(KeyCode::F(3))).unwrap(),
            Action::Copy { value, .. } if value == "pdg find --limit=50 --text f980"
        ));
        app.key(&db, key(KeyCode::Tab)).unwrap();
        app.search(&db).unwrap();
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));
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
