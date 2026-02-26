use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::AppState;
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let prompt_style = Style::default()
        .fg(theme::AMBER)
        .add_modifier(Modifier::BOLD);

    let input_line = if state.loading {
        Line::from(vec![
            Span::styled(">>> ", prompt_style),
            Span::styled("waiting...", Style::default().fg(theme::MUTED)),
        ])
    } else {
        Line::from(vec![
            Span::styled(">>> ", prompt_style),
            Span::styled(state.input.clone(), Style::default().fg(theme::WHITE)),
            Span::styled("\u{2588}", Style::default().fg(theme::AMBER)), // cursor block
        ])
    };

    let lines = vec![Line::from(""), input_line, Line::from("")];
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
