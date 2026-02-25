use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::markdown;
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let width = area.width.saturating_sub(6) as usize; // 4 for label + 2 padding
    let max_rows = area.height as usize;

    let mut all_lines: Vec<Line<'static>> = Vec::new();

    for msg in &state.messages {
        let (label, label_color) = match msg.role.as_str() {
            "user" => ("YOU", theme::AMBER),
            "agent" => ("AGT", theme::AMBER),
            _ => ("SYS", theme::AMBER),
        };

        let text_color = if msg.is_warning {
            theme::WARNING
        } else {
            theme::WHITE
        };

        let blocks = markdown::parse_blocks(&msg.text);
        let mut msg_lines: Vec<Line<'static>> = Vec::new();
        for block in &blocks {
            msg_lines.extend(markdown::render_block(block, width));
        }

        // Prepend role label to first line
        for (i, line) in msg_lines.iter_mut().enumerate() {
            let prefix = if i == 0 {
                Span::styled(
                    format!("{label} "),
                    Style::default()
                        .fg(label_color)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("    ")
            };

            let mut new_spans = vec![prefix];
            new_spans.append(&mut line.spans);

            // Apply text color to spans that don't have explicit styling
            for span in &mut new_spans[1..] {
                if span.style.fg.is_none() && !msg.is_warning {
                    span.style = span.style.fg(text_color);
                }
            }

            *line = Line::from(new_spans);
        }

        all_lines.extend(msg_lines);
    }

    // Spinner when loading
    if state.loading {
        all_lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled("\u{25CF} ", Style::default().fg(theme::AMBER)),
            Span::styled("Thinking...", Style::default().fg(theme::AMBER)),
        ]));
    }

    // Scroll to bottom: show last max_rows lines
    let start = all_lines.len().saturating_sub(max_rows);
    let visible: Vec<Line<'static>> = all_lines[start..].to_vec();

    let paragraph = Paragraph::new(visible);
    frame.render_widget(paragraph, area);
}
