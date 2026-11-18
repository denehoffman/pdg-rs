use std::{
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
    OwnedParticleSearchHit, ParticleSearchHit, ParticleSearchQuery, Pdg, TextSearchResult,
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{CliError, CliResult, DatabaseOptions, TuiCommand, open_pdg};

mod detail;
use detail::{Detail, View};

const RESULT_LIMIT: usize = 50;
const SEARCH_DELAY: Duration = Duration::from_millis(120);

pub fn run(command: &TuiCommand, database: &DatabaseOptions) -> CliResult<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(CliError::InvalidArgument(
            "`pdg tui` requires an interactive terminal. Use `pdg find` for piped output.".into(),
        ));
    }

    let db = open_pdg(database, command.edition.as_deref())?;
    let mut app = App::new(command.query.as_deref().unwrap_or_default(), command.text);
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
                    Action::Copy(value) => {
                        app.notice = Some(
                            match execute!(io::stdout(), CopyToClipboard::to_clipboard_from(&value))
                            {
                                Ok(()) => format!("Sent {value} to the terminal clipboard."),
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
    Copy(String),
}

enum Hit {
    Particle(OwnedParticleSearchHit),
    Text(TextSearchResult),
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
                if hit.matched_name != hit.particle.name {
                    lines.push(Line::raw(format!("Matched {}", hit.matched_name)));
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
    hits: Vec<Hit>,
    results: ListState,
    detail: Option<Detail>,
    pending: Option<Instant>,
    notice: Option<String>,
    page: u16,
    accent: Style,
}

impl App {
    fn new(query: &str, text: bool) -> Self {
        let mut input = SearchInput::default();
        input.insert(query);
        Self {
            input,
            mode: if text { Mode::Text } else { Mode::Particles },
            hits: Vec::new(),
            results: ListState::default(),
            detail: None,
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
        let query = self.input.text.trim();
        if query.is_empty() {
            return Ok(());
        }
        self.hits = match self.mode {
            Mode::Particles => db
                .find_particles(query, ParticleSearchQuery::new())?
                .into_iter()
                .take(RESULT_LIMIT)
                .map(ParticleSearchHit::into_owned)
                .map(Hit::Particle)
                .collect(),
            Mode::Text => db
                .search_text_with_limit(query, Some(RESULT_LIMIT))?
                .into_iter()
                .map(Hit::Text)
                .collect(),
        };
        self.results.select((!self.hits.is_empty()).then_some(0));
        Ok(())
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
        if self.input.insert(text) {
            self.schedule_search();
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
        if let Some(detail) = &mut self.detail {
            match key.code {
                KeyCode::Esc => self.detail = None,
                KeyCode::Up => detail.scroll = detail.scroll.saturating_sub(1),
                KeyCode::Down => detail.scroll = detail.scroll.saturating_add(1),
                KeyCode::PageUp => detail.scroll = detail.scroll.saturating_sub(self.page),
                KeyCode::PageDown => detail.scroll = detail.scroll.saturating_add(self.page),
                KeyCode::Home => detail.scroll = 0,
                KeyCode::End => detail.scroll = u16::MAX,
                KeyCode::Char('c') => return Ok(Action::Copy(detail.name_or_id().to_owned())),
                KeyCode::Char('i') => return Ok(Action::Copy(detail.id().to_owned())),
                KeyCode::Char(key) => {
                    if let Some(view) = View::from_key(key) {
                        detail.load(db, view)?;
                    }
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Esc => return Ok(Action::Quit),
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
        let footer_rows = u16::try_from(footer.line_count(area.width))
            .unwrap_or(u16::MAX)
            .min(area.height.saturating_sub(7));
        let [header, input, body, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
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
    }

    fn draw_input(&self, frame: &mut Frame<'_>, area: Rect) {
        let block =
            Block::bordered().title(format!(" Search {} ", self.mode.label().to_lowercase()));
        let block = if self.detail.is_none() {
            block.border_style(self.accent)
        } else {
            block
        };
        let inner = block.inner(area);
        let before_cursor = &self.input.text[..self.input.cursor];
        let column = u16::try_from(Line::raw(before_cursor).width()).unwrap_or(u16::MAX);
        let offset = column.saturating_sub(inner.width.saturating_sub(1));
        frame.render_widget(
            Paragraph::new(self.input.text.as_str())
                .block(block)
                .scroll((0, offset)),
            area,
        );
        if self.detail.is_none() && inner.width > 0 {
            frame.set_cursor_position((inner.x + (column - offset), inner.y));
        }
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
            if width < 64 {
                lines.push(Line::styled("1 Overview  2 Mass  3 Decays", self.accent));
                let views = if detail.has_text() {
                    "4 Measures  5 Refs  0 Text"
                } else {
                    "4 Measures  5 Refs"
                };
                lines.push(Line::styled(views, self.accent));
                lines.push(Line::raw("c Name  i ID  Esc Back"));
            } else {
                let views = if detail.has_text() {
                    "0 Text  1 Overview  2 Mass  3 Decays  4 Measurements  5 References"
                } else {
                    "1 Overview  2 Mass  3 Decays  4 Measurements  5 References"
                };
                lines.push(Line::styled(views, self.accent));
                lines.push(Line::raw(
                    "↑↓ Scroll  PgUp/PgDn Page  c Copy name  i Copy ID  Esc Back",
                ));
            }
        } else {
            lines.push(Line::raw(
                "Tab Switch search  ↑↓ Select  Enter Open  Esc Quit",
            ));
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
        if self.input.text.trim().is_empty() {
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

#[derive(Default)]
struct SearchInput {
    text: String,
    cursor: usize,
}

impl SearchInput {
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

    #[test]
    fn browser_retains_the_ranked_result_when_returning_from_a_detail() {
        let db = Pdg::open_path(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/pdgall-2025-v0.2.2.sqlite",
        ))
        .unwrap();
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let mut app = App::new("f980", false);
        app.search(&db).unwrap();
        assert_eq!(app.selected_hit().map(Hit::pdgid), Some("M003"));

        app.key(&db, key(KeyCode::Enter)).unwrap();
        app.key(&db, key(KeyCode::Char('2'))).unwrap();
        assert!(matches!(app.detail.as_ref().unwrap().view, View::Mass));
        assert!(matches!(
            app.key(&db, key(KeyCode::Char('c'))).unwrap(),
            Action::Copy(name) if name == "f_0(980)0"
        ));
        app.key(&db, key(KeyCode::Esc)).unwrap();
        assert!(app.detail.is_none());
        assert_eq!(app.input.text, "f980");
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
