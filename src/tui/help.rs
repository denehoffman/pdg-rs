use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use super::scroll_by_key;

#[derive(Default)]
pub(super) struct Help {
    scroll: u16,
    page: u16,
}

impl Help {
    pub(super) const fn navigate(&mut self, key: KeyCode) {
        self.scroll = scroll_by_key(self.scroll, key, self.page);
    }

    pub(super) fn draw(&mut self, frame: &mut Frame<'_>, accent: Style) {
        let screen = frame.area();
        let width = screen.width.min(68);
        let area = Rect::new(
            screen.x + (screen.width - width) / 2,
            screen.y,
            width,
            screen.height,
        );
        let block = Block::bordered()
            .title(" Help ")
            .title_bottom(" ↑↓ Scroll  Esc Close ")
            .border_style(accent);
        let inner = block.inner(area);
        let binding =
            |key, action| Line::from(vec![Span::styled(key, accent.bold()), Span::raw(action)]);
        let lines = vec![
            Line::raw("Search").bold(),
            binding("/", "  Focus the search bar"),
            binding("Shift+Tab", "  Switch between particle and text search"),
            binding("↑↓ / PgUp/PgDn", "  Select a result"),
            binding("Enter", "  Open a result"),
            binding("Ctrl+F", "  Open advanced filters in particle search"),
            binding("Tab", "  Complete a filter tag or value"),
            Line::raw("Use inline filters such as f980 mass:900..1100."),
            binding("Ctrl+O", "  Toggle mass order in particle search"),
            binding("F3", "  Copy the search as a pdg find command"),
            Line::default(),
            Line::raw("Entry details").bold(),
            binding("Tab / Shift+Tab", "  Switch views"),
            binding("Number / click", "  Open the matching tab"),
            binding("↑↓ / PgUp/PgDn", "  Scroll or select an entry"),
            binding("Enter", "  Follow a link or open a matched channel"),
            binding("c / i", "  Copy the particle name or PDG ID"),
            binding("Esc", "  Go back"),
            Line::default(),
            Line::raw("Advanced filters").bold(),
            binding("Tab / Shift+Tab", "  Move between fields"),
            binding("←→ / Space", "  Change a choice or open the list"),
            binding("↓ and Enter", "  Highlight and fill a decay suggestion"),
            binding("Enter", "  Choose an option or apply filters"),
            binding("Esc", "  Close the list or cancel edits"),
            Line::default(),
            binding("? / Esc", "  Close help"),
            binding("Ctrl+C", "  Quit"),
        ];
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        self.page = inner.height.max(1);
        let max_scroll = paragraph
            .line_count(inner.width)
            .saturating_sub(usize::from(inner.height));
        self.scroll = self
            .scroll
            .min(u16::try_from(max_scroll).unwrap_or(u16::MAX));
        frame.render_widget(paragraph.scroll((self.scroll, 0)).block(block), area);
    }
}
