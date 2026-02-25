use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::theme;

#[derive(Debug, Clone)]
pub struct ModalState {
    pub title: String,
    pub items: Vec<String>,
    pub selected: usize,
    pub filter: String,
}

impl ModalState {
    pub fn new(title: String, items: Vec<String>) -> Self {
        Self {
            title,
            items,
            selected: 0,
            filter: String::new(),
        }
    }

    pub fn filtered_items(&self) -> Vec<(usize, &String)> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                self.filter.is_empty()
                    || item.to_lowercase().contains(&self.filter.to_lowercase())
            })
            .collect()
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let max = self.filtered_items().len().saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &ModalState) {
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", state.title),
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme::AMBER));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    // Filter input
    if !state.filter.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(theme::MUTED)),
            Span::styled(&state.filter, Style::default().fg(theme::WHITE)),
        ]));
        lines.push(Line::from(""));
    }

    // Items
    let filtered = state.filtered_items();
    for (display_idx, (_orig_idx, item)) in filtered.iter().enumerate() {
        let style = if display_idx == state.selected {
            Style::default()
                .fg(theme::BG)
                .bg(theme::AMBER)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::WHITE)
        };
        lines.push(Line::from(Span::styled(format!(" {item}"), style)));
    }

    if filtered.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No matches",
            Style::default().fg(theme::MUTED),
        )));
    }

    // Hint
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  \u{2191}\u{2193}: navigate | Enter: select | Esc: cancel | Type to filter",
        Style::default().fg(theme::MUTED),
    )));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
