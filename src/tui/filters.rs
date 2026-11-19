use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pdg_rs::{OwnedParticleFilterHit, ParticleFilterHit, Pdg};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::Line,
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use super::SearchInput;
use crate::{
    CHARGE_VALUES, CLASS_VALUES, CliError, CliResult, PARTICLE_TYPE_VALUES, ParticleFilters,
    SPIN_VALUES, build_query, has_particle_filters, parse_charge, parse_class, parse_particle_type,
    parse_range, parse_spin_filter,
};

pub(super) struct AppliedFilters {
    draft: Draft,
    pub(super) summary: String,
    pub(super) hits: Vec<OwnedParticleFilterHit>,
}

impl AppliedFilters {
    pub(super) fn command_options(&self) -> impl Iterator<Item = (&'static str, &str)> {
        let draft = &self.draft;
        let decay = if draft.decay_mode == 0 {
            "--decays-to"
        } else {
            "--decay-contains"
        };
        [
            ("--class", Field::Class.value(draft.class)),
            ("--type", Field::Type.value(draft.particle_type)),
            ("--charge", Field::Charge.value(draft.charge)),
            ("--spin", draft.spin_value()),
            ("--mass", draft.mass.value()),
            (decay, draft.products.value()),
        ]
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
    }
}
#[derive(Clone, Default)]
struct Draft {
    class: usize,
    particle_type: usize,
    charge: usize,
    spin: usize,
    custom_spin: SearchInput,
    mass: SearchInput,
    decay_mode: usize,
    products: SearchInput,
}

const CUSTOM_SPIN: usize = SPIN_VALUES.len() + 1;

impl Draft {
    fn particle_filters(&self) -> CliResult<ParticleFilters> {
        let mut filters = ParticleFilters {
            class: parse_field("Class", Field::Class.value(self.class), parse_class)?,
            particle_type: parse_field(
                "Type",
                Field::Type.value(self.particle_type),
                parse_particle_type,
            )?,
            charge: parse_field("Charge", Field::Charge.value(self.charge), parse_charge)?,
            spin: parse_field("Spin", self.spin_value(), parse_spin_filter)?,
            mass: parse_field("Mass", self.mass.value(), parse_range)?,
            ..ParticleFilters::default()
        };
        if !self.products.value().is_empty() {
            let states = self
                .products
                .value()
                .split(',')
                .map(str::trim)
                .map(str::to_owned)
                .collect();
            if self.decay_mode == 0 {
                filters.decays_to = states;
            } else {
                filters.decay_contains = states;
            }
        }
        Ok(filters)
    }

    fn summary(&self) -> String {
        let decay = if self.decay_mode == 0 {
            "Decays to "
        } else {
            "Contains "
        };
        [
            ("", Field::Class.value(self.class), ""),
            ("", Field::Type.value(self.particle_type), ""),
            ("Q ", Field::Charge.value(self.charge), ""),
            ("J ", self.spin_value(), ""),
            ("", self.mass.value(), " MeV"),
            (decay, self.products.value(), ""),
        ]
        .into_iter()
        .filter(|(_, value, _)| !value.is_empty())
        .map(|(label, value, unit)| format!("{label}{value}{unit}"))
        .collect::<Vec<_>>()
        .join(" · ")
    }

    fn spin_value(&self) -> &str {
        if self.spin == CUSTOM_SPIN {
            self.custom_spin.value()
        } else {
            Field::Spin.value(self.spin)
        }
    }

    const fn selection(&self, field: Field) -> Option<usize> {
        match field {
            Field::Class => Some(self.class),
            Field::Type => Some(self.particle_type),
            Field::Charge => Some(self.charge),
            Field::Spin => Some(self.spin),
            Field::DecayMode => Some(self.decay_mode),
            _ => None,
        }
    }

    const fn select(&mut self, field: Field, index: usize) {
        let selection = match field {
            Field::Class => &mut self.class,
            Field::Type => &mut self.particle_type,
            Field::Charge => &mut self.charge,
            Field::Spin => &mut self.spin,
            Field::DecayMode => &mut self.decay_mode,
            _ => return,
        };
        *selection = index;
    }

    const fn input(&self, field: Field) -> Option<&SearchInput> {
        match field {
            Field::CustomSpin => Some(&self.custom_spin),
            Field::Mass => Some(&self.mass),
            Field::Products => Some(&self.products),
            _ => None,
        }
    }

    const fn input_mut(&mut self, field: Field) -> Option<&mut SearchInput> {
        match field {
            Field::CustomSpin => Some(&mut self.custom_spin),
            Field::Mass => Some(&mut self.mass),
            Field::Products => Some(&mut self.products),
            _ => None,
        }
    }
}

fn parse_field<T>(
    label: &str,
    value: &str,
    parse: impl FnOnce(&str) -> Result<T, String>,
) -> CliResult<Option<T>> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse(value)
            .map(Some)
            .map_err(|error| CliError::InvalidArgument(format!("{label} — {error}")))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Class,
    Type,
    Charge,
    Spin,
    CustomSpin,
    Mass,
    DecayMode,
    Products,
}

impl Field {
    const STANDARD: &[Self] = &[
        Self::Class,
        Self::Type,
        Self::Charge,
        Self::Spin,
        Self::Mass,
        Self::DecayMode,
        Self::Products,
    ];

    const WITH_CUSTOM_SPIN: &[Self] = &[
        Self::Class,
        Self::Type,
        Self::Charge,
        Self::Spin,
        Self::CustomSpin,
        Self::Mass,
        Self::DecayMode,
        Self::Products,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Class => "Class",
            Self::Type => "Type",
            Self::Charge => "Charge",
            Self::Spin => "Spin",
            Self::CustomSpin => "Custom spin",
            Self::Mass => "Mass (MeV)",
            Self::DecayMode => "Decay mode",
            Self::Products => "Products",
        }
    }

    const fn placeholder(self) -> &'static str {
        match self {
            Self::Mass => "900..2000",
            Self::Products => "KS,KS",
            _ => "",
        }
    }

    const fn choices(self) -> &'static [&'static str] {
        match self {
            Self::Class => CLASS_VALUES,
            Self::Type => PARTICLE_TYPE_VALUES,
            Self::Charge => CHARGE_VALUES,
            Self::Spin => SPIN_VALUES,
            Self::DecayMode => &["Decays to", "Contains"],
            _ => &[],
        }
    }

    fn choice_count(self) -> usize {
        self.choices().len()
            + usize::from(self != Self::DecayMode)
            + usize::from(self == Self::Spin)
    }

    fn value(self, index: usize) -> &'static str {
        if self == Self::Spin && index == CUSTOM_SPIN {
            return "Custom…";
        }
        let index = if self == Self::DecayMode {
            Some(index)
        } else {
            index.checked_sub(1)
        };
        index
            .and_then(|index| self.choices().get(index).copied())
            .unwrap_or("")
    }

    fn choice_label(self, index: usize) -> &'static str {
        let value = self.value(index);
        if value.is_empty() { "Any" } else { value }
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
    pub(super) error: Option<String>,
    pub(super) applying: bool,
}

impl FilterEditor {
    pub(super) fn new(filters: Option<&AppliedFilters>) -> Self {
        Self {
            draft: filters.map_or_else(Draft::default, |filters| filters.draft.clone()),
            focus: 0,
            menu: None,
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

    pub(super) fn key(&mut self, key: KeyEvent) -> EditorAction {
        self.error = None;
        let field = self.fields()[self.focus];
        if let Some(menu) = &mut self.menu {
            let index = menu.selected().unwrap_or_default();
            match key.code {
                KeyCode::Esc => self.menu = None,
                KeyCode::Enter => {
                    self.draft.select(field, index);
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
        match key.code {
            KeyCode::Esc => return EditorAction::Cancel,
            KeyCode::Enter => {
                self.applying = true;
                return EditorAction::Apply;
            }
            KeyCode::BackTab | KeyCode::Up => self.previous_field(),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.previous_field(),
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1) % self.fields().len(),
            _ => {
                if let Some(input) = self.draft.input_mut(field) {
                    input.key(key);
                } else if let Some(index) = self.draft.selection(field) {
                    let count = field.choice_count();
                    match key.code {
                        KeyCode::Left => self.draft.select(field, (index + count - 1) % count),
                        KeyCode::Right => self.draft.select(field, (index + 1) % count),
                        KeyCode::Home => self.draft.select(field, 0),
                        KeyCode::End => self.draft.select(field, count - 1),
                        KeyCode::Char(' ') => {
                            self.menu = Some(ListState::default().with_selected(Some(index)));
                        }
                        _ => {}
                    }
                }
            }
        }
        EditorAction::Continue
    }

    const fn previous_field(&mut self) {
        let count = self.fields().len();
        self.focus = (self.focus + count - 1) % count;
    }

    const fn fields(&self) -> &'static [Field] {
        if self.draft.spin == CUSTOM_SPIN {
            Field::WITH_CUSTOM_SPIN
        } else {
            Field::STANDARD
        }
    }

    pub(super) fn paste(&mut self, text: &str) {
        if let Some(input) = self.draft.input_mut(self.fields()[self.focus]) {
            input.insert(text);
            self.error = None;
        }
    }

    pub(super) fn draw(&mut self, frame: &mut Frame<'_>, screen: Rect, accent: Style) {
        let field = self.fields()[self.focus];
        let width = screen.width.min(64);
        let height = screen.height.min(if self.menu.is_some() { 23 } else { 15 });
        let area = Rect::new(
            screen.x + (screen.width - width) / 2,
            screen.y + (screen.height - height) / 2,
            width,
            height,
        );
        let title = if self.menu.is_some() {
            format!(" Choose {} ", field.label().to_lowercase())
        } else {
            " Particle filters ".to_owned()
        };
        let block = Block::bordered().title(title).border_style(accent);
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        if let Some(menu) = &mut self.menu {
            let footer = Paragraph::new("↑↓ Select  Enter Choose  Esc Back  ? Help")
                .style(accent)
                .wrap(Wrap { trim: true });
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
            lines.push(Line::styled("Tab Next  ←→ Change  Space Choices", accent));
            lines.push(Line::styled("Enter Apply  Esc Cancel  ? Help", accent));
            lines
        };
        let footer = Paragraph::new(message).wrap(Wrap { trim: true });
        let [fields, help] = split_footer(inner, &footer);
        self.draw_fields(frame, fields, accent);
        frame.render_widget(footer, help);
    }

    fn draw_fields(&self, frame: &mut Frame<'_>, area: Rect, accent: Style) {
        let rows = self.fields();
        let visible = usize::from(area.height).min(rows.len());
        let first = self.focus.saturating_sub(visible.saturating_sub(1));
        for (offset, &field) in rows.iter().skip(first).take(visible).enumerate() {
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
            if let Some(input) = self.draft.input(field) {
                input.draw(frame, value, field.placeholder(), focused && !self.applying);
            } else if let Some(index) = self.draft.selection(field) {
                let label = Line::raw(field.choice_label(index));
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

fn split_footer(area: Rect, footer: &Paragraph<'_>) -> [Rect; 2] {
    let height = u16::try_from(footer.line_count(area.width))
        .unwrap_or(u16::MAX)
        .min(area.height.saturating_sub(1));
    Layout::vertical([Constraint::Min(1), Constraint::Length(height)]).areas(area)
}

#[cfg(test)]
mod tests {
    use pdg_rs::{AngularMomentum, ParticleType};

    use super::*;
    use crate::QuantumArg;

    #[test]
    fn choosing_a_type_and_spin_uses_the_cli_filter_values() {
        let mut editor = FilterEditor::new(None);
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        editor.key(key(KeyCode::Tab));
        editor.key(key(KeyCode::End));
        editor.key(key(KeyCode::Tab));
        editor.key(key(KeyCode::Tab));
        editor.key(key(KeyCode::Right));
        assert!(matches!(
            editor.draft.particle_filters().unwrap().spin,
            Some(QuantumArg::Value(AngularMomentum::J0))
        ));

        editor.key(key(KeyCode::Char(' ')));
        editor.key(key(KeyCode::End));
        assert!(matches!(
            editor.key(key(KeyCode::Enter)),
            EditorAction::Continue
        ));
        editor.key(key(KeyCode::Tab));
        editor.paste("2,3");
        let filters = editor.draft.particle_filters().unwrap();
        assert_eq!(filters.particle_type, Some(ParticleType::SelfConjugate));
        assert!(matches!(
            filters.spin,
            Some(QuantumArg::Value(AngularMomentum::Custom(value))) if value == "2,3"
        ));
        editor.key(key(KeyCode::Tab));
        editor.key(key(KeyCode::Tab));
        editor.key(key(KeyCode::Right));
        editor.key(key(KeyCode::Tab));
        editor.paste("pi+,pi0");
        let applied = AppliedFilters {
            draft: editor.draft,
            summary: String::new(),
            hits: Vec::new(),
        };
        assert_eq!(
            applied.command_options().collect::<Vec<_>>(),
            [
                ("--type", "self-conjugate"),
                ("--spin", "2,3"),
                ("--decay-contains", "pi+,pi0"),
            ]
        );
    }
}
