use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    render_tools_panel(frame, area, state);
}

fn render_tools_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(Span::styled(
            "TOOLS",
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::new(1, 1, 1, 0));

    let mut lines = Vec::new();

    if state.tool_calls.is_empty() && !state.loading {
        lines.push(Line::from(Span::styled(
            "(none)",
            Style::default().fg(theme::MUTED),
        )));
    }

    for tc in &state.tool_calls {
        let check = if tc.success { "\u{2713}" } else { "\u{2717}" };
        let color = if tc.success { theme::GREEN } else { theme::RED };
        lines.push(Line::from(Span::styled(
            tc.name.clone(),
            Style::default().fg(theme::WHITE),
        )));
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}ms ", tc.duration_ms),
                Style::default().fg(theme::MUTED),
            ),
            Span::styled(check.to_string(), Style::default().fg(color)),
        ]));
    }

    if state.loading {
        lines.push(Line::from(vec![
            Span::styled("\u{25CF} ", Style::default().fg(theme::AMBER)),
            Span::styled("running..", Style::default().fg(theme::MUTED)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

