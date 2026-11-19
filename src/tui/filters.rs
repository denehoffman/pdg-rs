use std::ops::Range;

use clap::Parser;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pdg_rs::{DecayStateExpansion, DecayStateSide, OwnedParticleFilterHit, ParticleFilterHit, Pdg};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use super::SearchInput;
use crate::{
    CHARGE_VALUES, CLASS_VALUES, CliError, CliResult, ISOSPIN_VALUES, PARITY_VALUES,
    PARTICLE_TYPE_VALUES, ParticleFilters, SPIN_VALUES, build_query, has_particle_filters,
};

pub(super) struct AppliedFilters {
    draft: Draft,
    pub(super) summary: String,
    pub(super) hits: Vec<OwnedParticleFilterHit>,
}

impl AppliedFilters {
    pub(super) fn command_options(&self) -> impl Iterator<Item = (&'static str, Option<&str>)> {
        self.draft.command_options()
    }
}

#[derive(Clone)]
enum Control {
    Choice(usize),
    Text(SearchInput),
}

#[derive(Clone)]
struct Draft {
    controls: [Control; Field::ALL.len()],
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            controls: Field::ALL.map(|field| {
                if field.choices().is_empty() {
                    Control::Text(SearchInput::default())
                } else {
                    Control::Choice(0)
                }
            }),
        }
    }
}

impl Draft {
    fn particle_filters(&self) -> CliResult<ParticleFilters> {
        let args = self.command_options().map(|(flag, value)| {
            value.map_or_else(|| flag.to_owned(), |value| format!("{flag}={value}"))
        });
        ParticleFilters::try_parse_from(std::iter::once("pdg find".to_owned()).chain(args)).map_err(
            |error| {
                let message = error.to_string();
                CliError::InvalidArgument(message.lines().next().unwrap_or(&message).to_owned())
            },
        )
    }

    fn value(&self, field: Field) -> &str {
        match &self.controls[field as usize] {
            Control::Choice(index) => field.value(*index),
            Control::Text(input) => input.value(),
        }
    }

    fn applied_value(&self, field: Field) -> &str {
        if field == Field::Spin && self.value(field) == CUSTOM_SPIN {
            self.value(Field::CustomSpin)
        } else {
            self.value(field)
        }
    }

    fn command_options(&self) -> impl Iterator<Item = (&'static str, Option<&str>)> {
        Field::ALL.into_iter().filter_map(|field| {
            let value = self.applied_value(field);
            let flag = match field {
                Field::CustomSpin | Field::DecayMode => return None,
                Field::IncludeUnknown => {
                    return (value == KEEP_PARTICLE)
                        .then_some(("--include-unknown-properties", None));
                }
                Field::DecayExpansion if value == "inclusive" => return None,
                Field::Products if self.value(Field::DecayMode) == "Contains" => "--decay-contains",
                _ => field.flag(),
            };
            (!value.is_empty()).then_some((flag, Some(value)))
        })
    }

    fn summary(&self) -> String {
        Field::ALL
            .into_iter()
            .filter_map(|field| {
                let value = self.applied_value(field);
                if value.is_empty() {
                    return None;
                }
                let text = match field {
                    Field::Class | Field::Type => value.to_owned(),
                    Field::Charge => format!("Q {value}"),
                    Field::Spin => format!("J {value}"),
                    Field::Isospin => format!("I {value}"),
                    Field::Parity => format!("P {value}"),
                    Field::CParity => format!("C {value}"),
                    Field::GParity => format!("G {value}"),
                    Field::Mass => format!("{value} MeV"),
                    Field::Width => format!("Width {value} MeV"),
                    Field::Lifetime => format!("Lifetime {value} s"),
                    Field::IncludeUnknown if value == KEEP_PARTICLE => {
                        "Keep particles with no value".into()
                    }
                    Field::Products => format!("{} {value}", self.value(Field::DecayMode)),
                    Field::DecayExpansion if value == "literal" => "Literal decay names".into(),
                    Field::Mcid | Field::DecaysFrom => format!("{} {value}", field.label()),
                    _ => return None,
                };
                Some(text)
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

const CUSTOM_SPIN: &str = "Custom…";
const KEEP_PARTICLE: &str = "Keep particle";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Class,
    Type,
    Charge,
    Mcid,
    Spin,
    CustomSpin,
    Isospin,
    Parity,
    CParity,
    GParity,
    Mass,
    Width,
    Lifetime,
    IncludeUnknown,
    DecayMode,
    Products,
    DecaysFrom,
    DecayExpansion,
}

impl Field {
    // Each field uses its position in this list to index its control.
    const ALL: [Self; 18] = [
        Self::Class,
        Self::Type,
        Self::Charge,
        Self::Mcid,
        Self::Spin,
        Self::CustomSpin,
        Self::Isospin,
        Self::Parity,
        Self::CParity,
        Self::GParity,
        Self::Mass,
        Self::Width,
        Self::Lifetime,
        Self::IncludeUnknown,
        Self::DecayMode,
        Self::Products,
        Self::DecaysFrom,
        Self::DecayExpansion,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Class => "Class",
            Self::Type => "Type",
            Self::Charge => "Charge",
            Self::Mcid => "MCID",
            Self::Spin => "Spin",
            Self::CustomSpin => "Custom spin",
            Self::Isospin => "Isospin",
            Self::Parity => "Parity",
            Self::CParity => "C-parity",
            Self::GParity => "G-parity",
            Self::Mass => "Mass (MeV)",
            Self::Width => "Width (MeV)",
            Self::Lifetime => "Lifetime (s)",
            Self::IncludeUnknown => "No value",
            Self::DecayMode => "Decay mode",
            Self::Products => "Products",
            Self::DecaysFrom => "Decays from",
            Self::DecayExpansion => "Expansion",
        }
    }

    const fn flag(self) -> &'static str {
        match self {
            Self::Class => "--class",
            Self::Type => "--type",
            Self::Charge => "--charge",
            Self::Mcid => "--mcid",
            Self::Spin => "--spin",
            Self::Isospin => "--isospin",
            Self::Parity => "--parity",
            Self::CParity => "--c-parity",
            Self::GParity => "--g-parity",
            Self::Mass => "--mass",
            Self::Width => "--width",
            Self::Lifetime => "--lifetime",
            Self::Products => "--decays-to",
            Self::DecaysFrom => "--decays-from",
            Self::DecayExpansion => "--decay-expansion",
            Self::CustomSpin | Self::DecayMode | Self::IncludeUnknown => unreachable!(),
        }
    }

    const fn placeholder(self) -> &'static str {
        match self {
            Self::Mcid => "-211",
            Self::Mass => "900..2000",
            Self::Width => "0..100",
            Self::Lifetime => "1e-12..1e-6",
            Self::Products => "KS,KS",
            Self::DecaysFrom => "pi+",
            _ => "",
        }
    }

    const fn decay_side(self) -> Option<DecayStateSide> {
        match self {
            Self::Products => Some(DecayStateSide::Final),
            Self::DecaysFrom => Some(DecayStateSide::Initial),
            _ => None,
        }
    }

    const fn choices(self) -> &'static [&'static str] {
        match self {
            Self::Class => CLASS_VALUES,
            Self::Type => PARTICLE_TYPE_VALUES,
            Self::Charge => CHARGE_VALUES,
            Self::Spin => SPIN_VALUES,
            Self::Isospin => ISOSPIN_VALUES,
            Self::Parity | Self::CParity | Self::GParity => PARITY_VALUES,
            Self::IncludeUnknown => &["Skip particle", KEEP_PARTICLE],
            Self::DecayMode => &["Decays to", "Contains"],
            Self::DecayExpansion => &["inclusive", "literal"],
            _ => &[],
        }
    }

    const fn has_any(self) -> bool {
        !matches!(
            self,
            Self::DecayMode | Self::DecayExpansion | Self::IncludeUnknown
        )
    }

    fn choice_count(self) -> usize {
        self.choices().len() + usize::from(self.has_any()) + usize::from(self == Self::Spin)
    }

    fn value(self, index: usize) -> &'static str {
        let Some(index) = index.checked_sub(usize::from(self.has_any())) else {
            return "";
        };
        if self == Self::Spin && index == SPIN_VALUES.len() {
            CUSTOM_SPIN
        } else {
            self.choices().get(index).copied().unwrap_or("")
        }
    }

    fn choice_label(self, index: usize) -> &'static str {
        let value = self.value(index);
        if value.is_empty() { "Any" } else { value }
    }

    fn hint(self, value: &str) -> Option<&'static str> {
        match (self, value) {
            (Self::IncludeUnknown, _) => Some("Applies to mass, width, and lifetime ranges."),
            (Self::DecayMode, "Decays to") => Some("Require exactly these products in one decay."),
            (Self::DecayMode, "Contains") => Some("Allow extra products in the same decay."),
            (Self::DecayExpansion, "inclusive") => Some("Resolve aliases and expand PDG families."),
            (Self::DecayExpansion, "literal") => {
                Some("Match names exactly as recorded by the PDG.")
            }
            _ => None,
        }
    }
}

pub(super) enum EditorAction {
    Continue,
    Apply,
    Cancel,
}

pub(super) struct FilterEditor {
    draft: Draft,
    focus: usize,
    menu: Option<ListState>,
    suggestions: Option<DecaySuggestions>,
    pub(super) error: Option<String>,
    pub(super) applying: bool,
}

struct DecaySuggestions {
    names: Vec<String>,
    state: ListState,
}

impl DecaySuggestions {
    fn draw_below(&mut self, frame: &mut Frame<'_>, body: Rect) -> Rect {
        let rows = u16::try_from(self.names.len())
            .unwrap_or(u16::MAX)
            .min(4)
            .min(body.height.saturating_sub(1));
        let [fields, choices] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(rows)]).areas(body);
        let items = self.names.iter().map(|name| ListItem::new(name.as_str()));
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("› ")
                .highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
            choices,
            &mut self.state,
        );
        fields
    }
}

fn decay_token(input: &SearchInput) -> Range<usize> {
    let start = input.text[..input.cursor]
        .rfind(',')
        .map_or(0, |index| index + 1);
    let end = input.text[input.cursor..]
        .find(',')
        .map_or(input.text.len(), |index| input.cursor + index);
    start..end
}

impl FilterEditor {
    pub(super) fn new(filters: Option<&AppliedFilters>) -> Self {
        Self {
            draft: filters.map_or_else(Draft::default, |filters| filters.draft.clone()),
            focus: 0,
            menu: None,
            suggestions: None,
            error: None,
            applying: false,
        }
    }

    pub(super) fn apply(&self, db: &Pdg) -> CliResult<Option<AppliedFilters>> {
        let filters = self.draft.particle_filters()?;
        if !has_particle_filters(&filters) {
            return Ok(None);
        }
        Ok(Some(AppliedFilters {
            draft: self.draft.clone(),
            summary: self.draft.summary(),
            hits: db
                .search_particle_hits(build_query(&filters)?)?
                .into_iter()
                .map(ParticleFilterHit::into_owned)
                .collect(),
        }))
    }

    pub(super) fn key(&mut self, db: &Pdg, key: KeyEvent) -> EditorAction {
        self.error = None;
        let field = self.focused_field();
        if let Some(menu) = &mut self.menu {
            let index = menu.selected().unwrap_or_default();
            match key.code {
                KeyCode::Esc => self.menu = None,
                KeyCode::Enter => {
                    self.draft.controls[field as usize] = Control::Choice(index);
                    self.menu = None;
                }
                KeyCode::Home => menu.select(Some(0)),
                KeyCode::End => menu.select(Some(field.choice_count() - 1)),
                KeyCode::Up => menu.select(Some(index.saturating_sub(1))),
                KeyCode::Down => menu.select(Some((index + 1).min(field.choice_count() - 1))),
                _ => {}
            }
            return EditorAction::Continue;
        }
        if self.suggestion_key(field, key.code) {
            return EditorAction::Continue;
        }
        match key.code {
            KeyCode::Esc => return EditorAction::Cancel,
            KeyCode::Enter => {
                self.applying = true;
                return EditorAction::Apply;
            }
            KeyCode::BackTab | KeyCode::Up => self.previous_field(),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.previous_field(),
            KeyCode::Tab | KeyCode::Down => {
                let count = self.fields().count();
                self.focus = (self.focus + 1) % count;
            }
            _ => match &mut self.draft.controls[field as usize] {
                Control::Text(input) => {
                    let previous_token = field.decay_side().map(|_| decay_token(input));
                    let changed = input.key(key);
                    if let Some(previous_token) = previous_token
                        && (changed || previous_token != decay_token(input))
                    {
                        self.refresh_suggestions(db);
                    }
                }
                Control::Choice(index) => {
                    let count = field.choice_count();
                    match key.code {
                        KeyCode::Left => *index = (*index + count - 1) % count,
                        KeyCode::Right => *index = (*index + 1) % count,
                        KeyCode::Home => *index = 0,
                        KeyCode::End => *index = count - 1,
                        KeyCode::Char(' ') => {
                            self.menu = Some(ListState::default().with_selected(Some(*index)));
                        }
                        _ => {}
                    }
                }
            },
        }
        if self.focused_field() != field {
            self.refresh_suggestions(db);
        }
        EditorAction::Continue
    }

    fn refresh_suggestions(&mut self, db: &Pdg) {
        self.suggestions = None;
        let field = self.focused_field();
        let Some(side) = field.decay_side() else {
            return;
        };
        let Control::Text(input) = &self.draft.controls[field as usize] else {
            return;
        };
        let expansion = if self.draft.value(Field::DecayExpansion) == "literal" {
            DecayStateExpansion::Literal
        } else {
            DecayStateExpansion::Inclusive
        };
        match db.suggest_decay_states(&input.text[decay_token(input)], side, expansion, 8) {
            Ok(names) if !names.is_empty() => {
                self.suggestions = Some(DecaySuggestions {
                    names,
                    state: ListState::default(),
                });
            }
            Err(error) => self.error = Some(format!("Decay suggestions unavailable. {error}")),
            _ => {}
        }
    }

    fn suggestion_key(&mut self, field: Field, key: KeyCode) -> bool {
        let Some(suggestions) = &mut self.suggestions else {
            return false;
        };
        let selected = suggestions.state.selected();
        match (key, selected) {
            (KeyCode::Down, selected) => {
                let index =
                    selected.map_or(0, |index| (index + 1).min(suggestions.names.len() - 1));
                suggestions.state.select(Some(index));
            }
            (KeyCode::Up, Some(_)) => suggestions.state.select_previous(),
            (KeyCode::Esc, _) => self.suggestions = None,
            (KeyCode::Enter, Some(index)) => {
                let Control::Text(input) = &mut self.draft.controls[field as usize] else {
                    return false;
                };
                let replacement = &suggestions.names[index];
                let token = decay_token(input);
                input.cursor = token.start + replacement.len();
                input.text.replace_range(token, replacement);
                self.suggestions = None;
            }
            _ => return false,
        }
        true
    }

    fn previous_field(&mut self) {
        let count = self.fields().count();
        self.focus = (self.focus + count - 1) % count;
    }

    fn fields(&self) -> impl Iterator<Item = Field> + use<> {
        let custom_spin = self.draft.value(Field::Spin) == CUSTOM_SPIN;
        Field::ALL
            .into_iter()
            .filter(move |&field| field != Field::CustomSpin || custom_spin)
    }

    fn focused_field(&self) -> Field {
        self.fields()
            .nth(self.focus)
            .expect("focused filter field is visible")
    }

    pub(super) fn paste(&mut self, db: &Pdg, text: &str) {
        let field = self.focused_field();
        if let Control::Text(input) = &mut self.draft.controls[field as usize] {
            input.insert(text);
            self.error = None;
            self.refresh_suggestions(db);
        }
    }

    pub(super) fn draw(&mut self, frame: &mut Frame<'_>, screen: Rect, accent: Style) {
        let field = self.focused_field();
        let field_count = self.fields().count();
        let width = screen.width.min(64);
        let height = screen.height.min(if self.menu.is_some() {
            23
        } else {
            u16::try_from(field_count + 6).unwrap_or(u16::MAX)
        });
        let area = Rect::new(
            screen.x + (screen.width - width) / 2,
            screen.y + (screen.height - height) / 2,
            width,
            height,
        );
        let title = if self.menu.is_some() {
            if field == Field::IncludeUnknown {
                " No comparable value ".to_owned()
            } else {
                format!(" Choose {} ", field.label().to_lowercase())
            }
        } else {
            format!(" Particle filters · {}/{} ", self.focus + 1, field_count)
        };
        let block = Block::bordered().title(title).border_style(accent);
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        if let Some(menu) = &mut self.menu {
            let hint = field.hint(field.value(menu.selected().unwrap_or_default()));
            let lines = hint
                .into_iter()
                .map(Line::raw)
                .chain(std::iter::once(Line::styled(
                    "↑↓ Select  Enter Choose  Esc Back  ? Help",
                    accent,
                )));
            let footer = Paragraph::new(lines.collect::<Vec<_>>()).wrap(Wrap { trim: true });
            let [choices, help] = split_footer(inner, &footer);
            let items =
                (0..field.choice_count()).map(|index| ListItem::new(field.choice_label(index)));
            frame.render_stateful_widget(
                List::new(items)
                    .highlight_symbol("› ")
                    .highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
                choices,
                menu,
            );
            frame.render_widget(footer, help);
            return;
        }

        let message = if self.applying {
            vec![Line::raw("Applying filters…").bold()]
        } else {
            let mut lines = Vec::new();
            if let Some(error) = &self.error {
                lines.push(Line::raw(error.as_str()));
            }
            if let Some(hint) = field.hint(self.draft.value(field)) {
                lines.push(Line::raw(hint));
            }
            if let Some(suggestions) = &self.suggestions {
                let navigation = if suggestions.state.selected().is_some() {
                    "↑↓ Choose  Enter Fill"
                } else {
                    "↓ Suggestions  Enter Apply"
                };
                lines.push(Line::styled(navigation, accent));
                lines.push(Line::styled("Tab Next  Esc Hide  ? Help", accent));
            } else if field.decay_side().is_some() {
                lines.push(Line::styled("Tab Next  Enter Apply", accent));
                lines.push(Line::styled("Esc Cancel  ? Help", accent));
            } else {
                lines.push(Line::styled("Tab Next  ←→ Change  Space Choices", accent));
                lines.push(Line::styled("Enter Apply  Esc Cancel  ? Help", accent));
            }
            lines
        };
        let footer = Paragraph::new(message).wrap(Wrap { trim: true });
        let [body, help] = split_footer(inner, &footer);
        let fields = self
            .suggestions
            .as_mut()
            .map_or(body, |suggestions| suggestions.draw_below(frame, body));
        self.draw_fields(frame, fields, accent);
        frame.render_widget(footer, help);
    }

    fn draw_fields(&self, frame: &mut Frame<'_>, area: Rect, accent: Style) {
        let visible = usize::from(area.height).min(self.fields().count());
        let first = self.focus.saturating_sub(visible.saturating_sub(1));
        for (offset, field) in self.fields().skip(first).take(visible).enumerate() {
            let row = Rect::new(
                area.x,
                area.y + u16::try_from(offset).unwrap_or_default(),
                area.width,
                1,
            );
            let [label, value] =
                Layout::horizontal([Constraint::Length(14), Constraint::Min(1)]).areas(row);
            let focused = first + offset == self.focus;
            let marker = if focused { "›" } else { " " };
            let mut field_label = Line::raw(format!("{marker} {}", field.label()));
            if focused {
                field_label = field_label.style(accent).bold();
            }
            frame.render_widget(field_label, label);
            match &self.draft.controls[field as usize] {
                Control::Text(input) => {
                    input.draw(frame, value, field.placeholder(), focused && !self.applying);
                }
                Control::Choice(index) => {
                    let label = Line::raw(field.choice_label(*index));
                    frame.render_widget(
                        if focused {
                            label.style(accent).bold()
                        } else {
                            label
                        },
                        value,
                    );
                }
            }
        }
    }
}

fn split_footer(area: Rect, footer: &Paragraph<'_>) -> [Rect; 2] {
    let height = u16::try_from(footer.line_count(area.width))
        .unwrap_or(u16::MAX)
        .min(area.height.saturating_sub(1));
    Layout::vertical([Constraint::Min(1), Constraint::Length(height)]).areas(area)
}

#[cfg(test)]
mod tests {
    use pdg_rs::{AngularMomentum, Isospin, ParticleType};

    use super::{super::tests::test_db, *};
    use crate::QuantumArg;

    fn focus(editor: &mut FilterEditor, field: Field) {
        editor.focus = editor
            .fields()
            .position(|candidate| candidate == field)
            .unwrap();
        editor.suggestions = None;
    }

    #[test]
    fn choosing_a_type_and_spin_uses_the_cli_filter_values() {
        let db = test_db();
        let mut editor = FilterEditor::new(None);
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        editor.key(&db, key(KeyCode::Tab));
        editor.key(&db, key(KeyCode::End));
        focus(&mut editor, Field::Spin);
        editor.key(&db, key(KeyCode::Right));
        assert!(matches!(
            editor.draft.particle_filters().unwrap().spin,
            Some(QuantumArg::Value(AngularMomentum::J0))
        ));

        editor.key(&db, key(KeyCode::Char(' ')));
        editor.key(&db, key(KeyCode::End));
        assert!(matches!(
            editor.key(&db, key(KeyCode::Enter)),
            EditorAction::Continue
        ));
        editor.key(&db, key(KeyCode::Tab));
        editor.paste(&db, "2,3");
        let filters = editor.draft.particle_filters().unwrap();
        assert_eq!(filters.particle_type, Some(ParticleType::SelfConjugate));
        assert!(matches!(
            filters.spin,
            Some(QuantumArg::Value(AngularMomentum::Custom(value))) if value == "2,3"
        ));
        focus(&mut editor, Field::DecayMode);
        editor.key(&db, key(KeyCode::Right));
        editor.key(&db, key(KeyCode::Tab));
        editor.paste(&db, "pi+,pi0");
        let applied = AppliedFilters {
            draft: editor.draft,
            summary: String::new(),
            hits: Vec::new(),
        };
        assert_eq!(
            applied.command_options().collect::<Vec<_>>(),
            [
                ("--type", Some("self-conjugate")),
                ("--spin", Some("2,3")),
                ("--decay-contains", Some("pi+,pi0")),
            ]
        );
    }

    #[test]
    fn missing_value_choices_stay_separate_from_the_label_in_a_narrow_terminal() {
        let db = test_db();
        let mut editor = FilterEditor::new(None);
        focus(&mut editor, Field::IncludeUnknown);
        let backend = ratatui::backend::TestBackend::new(30, 10);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        for choice in ["Skip particle", KEEP_PARTICLE] {
            terminal
                .draw(|frame| editor.draw(frame, frame.area(), Style::new()))
                .unwrap();
            let screen: String = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect();
            assert!(screen.contains(&format!("No value    {choice}")));
            editor.key(&db, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }
    }

    #[test]
    fn decay_choice_hints_follow_the_highlighted_option() {
        let db = test_db();
        let mut editor = FilterEditor::new(None);
        let backend = ratatui::backend::TestBackend::new(64, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let mut screen = |editor: &mut FilterEditor| {
            terminal
                .draw(|frame| editor.draw(frame, frame.area(), Style::new()))
                .unwrap();
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
        };
        for (field, first, second) in [
            (
                Field::DecayMode,
                "Require exactly these products in one decay.",
                "Allow extra products in the same decay.",
            ),
            (
                Field::DecayExpansion,
                "Resolve aliases and expand PDG families.",
                "Match names exactly as recorded by the PDG.",
            ),
        ] {
            focus(&mut editor, field);
            assert!(screen(&mut editor).contains(first));
            editor.key(&db, key(KeyCode::Char(' ')));
            assert!(screen(&mut editor).contains(first));
            editor.key(&db, key(KeyCode::Down));
            assert!(screen(&mut editor).contains(second));
            editor.key(&db, key(KeyCode::Enter));
            assert!(screen(&mut editor).contains(second));
        }
    }

    #[test]
    fn decay_completion_replaces_only_the_token_under_the_cursor() {
        let db = test_db();
        let mut editor = FilterEditor::new(None);
        focus(&mut editor, Field::Products);
        editor.paste(&db, "K(S)0, Kₛ,K(S)0");
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        editor.key(&db, key(KeyCode::Home));
        for _ in 0.."K(S)0, K".len() {
            editor.key(&db, key(KeyCode::Right));
        }
        assert_eq!(editor.suggestions.as_ref().unwrap().names[0], "K(S)0");
        editor.key(&db, key(KeyCode::Down));
        assert!(matches!(
            editor.key(&db, key(KeyCode::Enter)),
            EditorAction::Continue
        ));
        assert_eq!(
            editor.draft.particle_filters().unwrap().decays_to,
            ["K(S)0"; 3]
        );
        assert!(editor.suggestions.is_none());

        focus(&mut editor, Field::DecaysFrom);
        editor.paste(&db, "pi+");
        assert!(editor.suggestions.is_some());
        assert!(matches!(
            editor.key(&db, key(KeyCode::Enter)),
            EditorAction::Apply
        ));
    }

    #[test]
    fn additional_controls_use_the_cli_values_and_boolean_flag() {
        let db = test_db();
        let mut editor = FilterEditor::new(None);
        for (field, input) in [
            (Field::Mcid, "-211"),
            (Field::Width, "0..100"),
            (Field::Lifetime, "1e-12..1e-6"),
            (Field::DecaysFrom, "pi+,pi0"),
        ] {
            focus(&mut editor, field);
            editor.paste(&db, input);
        }
        for (field, key) in [
            (Field::Isospin, KeyCode::Right),
            (Field::GParity, KeyCode::End),
            (Field::IncludeUnknown, KeyCode::Right),
            (Field::DecayExpansion, KeyCode::Right),
        ] {
            focus(&mut editor, field);
            editor.key(&db, KeyEvent::new(key, KeyModifiers::NONE));
        }
        let filters = editor.draft.particle_filters().unwrap();
        assert_eq!(filters.mcid, Some(-211));
        assert!(matches!(
            filters.isospin,
            Some(QuantumArg::Value(Isospin::I0))
        ));
        assert!(matches!(filters.g_parity, Some(QuantumArg::Missing)));
        assert_eq!(filters.width, Some((0.0, 100.0)));
        assert_eq!(filters.lifetime, Some((1e-12, 1e-6)));
        assert!(filters.include_unknown_properties);
        assert!(matches!(
            filters.decay_expansion,
            crate::DecayExpansionArg::Literal
        ));
        assert_eq!(filters.decays_from, ["pi+", "pi0"]);
        assert!(
            editor
                .draft
                .command_options()
                .any(|option| option == ("--include-unknown-properties", None))
        );
    }
}
