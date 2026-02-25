use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // MODEL (name + traits count)
            Constraint::Min(4),    // TOOLS
            Constraint::Length(10), // SESSION
        ])
        .split(area);

    render_model_panel(frame, chunks[0], state);
    render_tools_panel(frame, chunks[1], state);
    render_session_panel(frame, chunks[2], state);
}

fn render_model_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(Span::styled(
            " MODEL ",
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER));

    let model_name = state.model.split('/').next_back().unwrap_or(&state.model);
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Name  ", Style::default().fg(theme::AMBER)),
            Span::styled(model_name.to_string(), Style::default().fg(theme::WHITE)),
        ]),
        Line::from(vec![
            Span::styled("Traits", Style::default().fg(theme::AMBER)),
            Span::styled(
                if state.traits.is_empty() {
                    " -".to_string()
                } else {
                    format!(" {}", state.traits.len())
                },
                Style::default().fg(if state.traits.is_empty() {
                    theme::MUTED
                } else {
                    theme::WHITE
                }),
            ),
        ]),
    ];

    for t in &state.traits {
        lines.push(Line::from(Span::styled(
            format!("  {t}"),
            Style::default().fg(theme::MUTED),
        )));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_tools_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(Span::styled(
            " TOOLS ",
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER));

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

fn render_session_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(Span::styled(
            " SESSION ",
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER));

    if state.turn_count == 0 {
        let lines = vec![Line::from(Span::styled(
            "No session",
            Style::default().fg(theme::MUTED),
        ))];
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let turns = state.turn_count.to_string();
    let input_tok = state.total_input_tokens.to_string();
    let output_tok = state.total_output_tokens.to_string();
    let latency = format!("{}ms", state.latency_ms);
    let steps = state.total_steps.to_string();
    let feedback_str = match state.feedback {
        Some(1) => "\u{1F44D}".to_string(),
        Some(-1) => "\u{1F44E}".to_string(),
        _ => "-".to_string(),
    };

    let lines = vec![
        session_line("Turn", &turns),
        session_line("Tkn In", &input_tok),
        session_line("Tkn Out", &output_tok),
        session_line("Latency", &latency),
        session_line("Steps", &steps),
        session_line("Feedback", &feedback_str),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn session_line<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{label:<8}"),
            Style::default().fg(theme::AMBER),
        ),
        Span::styled(value.to_string(), Style::default().fg(theme::WHITE)),
    ])
}
