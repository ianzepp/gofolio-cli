use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::theme;

#[derive(Debug, Clone)]
pub struct ModalItem {
    pub text: String,
    pub selectable: bool,
}

#[derive(Debug, Clone)]
pub struct ModalState {
    pub title: String,
    pub items: Vec<ModalItem>,
    pub selected: usize,
    pub filter: String,
}

impl ModalState {
    fn filtered_indices(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                !item.selectable
                    || self.filter.is_empty()
                    || item
                        .text
                        .to_lowercase()
                        .contains(&self.filter.to_lowercase())
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    #[allow(dead_code)]
    pub fn new(title: String, items: Vec<String>) -> Self {
        let items = items
            .into_iter()
            .map(|text| ModalItem {
                text,
                selectable: true,
            })
            .collect();
        Self::new_items(title, items)
    }

    pub fn new_items(title: String, items: Vec<ModalItem>) -> Self {
        Self {
            title,
            items,
            selected: 0,
            filter: String::new(),
        }
    }

    pub fn filtered_items(&self) -> Vec<(usize, &ModalItem)> {
        self.filtered_indices()
            .into_iter()
            .map(|idx| (idx, &self.items[idx]))
            .collect()
    }

    pub fn normalize_selection(&mut self) {
        let filtered = self.filtered_indices();
        if filtered.is_empty() {
            self.selected = 0;
            return;
        }

        let max = filtered.len().saturating_sub(1);
        if self.selected > max {
            self.selected = max;
        }

        if self.items[filtered[self.selected]].selectable {
            return;
        }

        if let Some((idx, _)) = filtered
            .iter()
            .enumerate()
            .find(|(_, orig_idx)| self.items[**orig_idx].selectable)
        {
            self.selected = idx;
        }
    }

    pub fn move_up(&mut self) {
        if self.selected == 0 {
            return;
        }
        let filtered = self.filtered_indices();
        let mut idx = self.selected.saturating_sub(1);
        loop {
            if self.items[filtered[idx]].selectable {
                self.selected = idx;
                break;
            }
            if idx == 0 {
                break;
            }
            idx -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let filtered = self.filtered_indices();
        let max = filtered.len().saturating_sub(1);
        if self.selected >= max {
            return;
        }
        let mut idx = self.selected + 1;
        while idx <= max {
            if self.items[filtered[idx]].selectable {
                self.selected = idx;
                break;
            }
            idx += 1;
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
    lines.push(Line::from(vec![
        Span::styled("Filter: ", Style::default().fg(theme::MUTED)),
        Span::styled(&state.filter, Style::default().fg(theme::WHITE)),
    ]));
    lines.push(Line::from(""));

    // Items
    let filtered = state.filtered_items();
    for (display_idx, (_orig_idx, item)) in filtered.iter().enumerate() {
        let style = if !item.selectable {
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD)
        } else if display_idx == state.selected {
            Style::default()
                .fg(theme::BG)
                .bg(theme::AMBER)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::WHITE)
        };
        lines.push(Line::from(Span::styled(format!(" {}", item.text), style)));
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
