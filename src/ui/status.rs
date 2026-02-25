use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::app::AppState;
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect) {
    let shortcuts = vec![
        ("N", "New Session"),
        ("Y", "Thumbs Up"),
        ("R", "Report"),
        ("P", "Model"),
        ("T", "Traits"),
        ("L", "Logout"),
        ("Q", "Quit"),
    ];

    let mut spans = vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            "CTRL",
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default()),
    ];

    for (key, label) in &shortcuts {
        spans.push(Span::styled(
            format!("[{key}]"),
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {label} "),
            Style::default().fg(theme::MUTED),
        ));
    }

    frame.render_widget(Line::from(spans), area);
}

/// Render the full-width amber header bar.
pub fn render_header(frame: &mut Frame, area: Rect, title: &str) {
    let width = area.width as usize;
    let padded = format!(" {title} ");
    let display = if padded.len() < width {
        format!("{padded}{}", " ".repeat(width - padded.len()))
    } else {
        padded[..width].to_string()
    };

    let line = Line::from(Span::styled(
        display,
        Style::default()
            .fg(theme::WHITE)
            .bg(theme::AMBER)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(line, area);
}

/// Render the session status bar above the input prompt (tmux/irssi style).
/// Left side: model + turn + steps. Right side: tokens + latency + feedback.
pub fn render_session_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let width = area.width as usize;
    let style = Style::default()
        .fg(theme::WHITE)
        .bg(theme::AMBER)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(theme::BG).bg(theme::AMBER);

    if state.turn_count == 0 {
        // No session — just fill with the amber bar
        let fill = " ".repeat(width);
        let line = Line::from(Span::styled(fill, style));
        frame.render_widget(line, area);
        return;
    }

    let model_name = state
        .model
        .split('/')
        .next_back()
        .unwrap_or(&state.model);

    let sep = Span::styled(" \u{00B7} ", dim);

    // Left: model, turn, steps
    let feedback = match state.feedback {
        Some(1) => "\u{2191}",
        Some(-1) => "\u{2193}",
        _ => "-",
    };

    let left_spans = vec![
        Span::styled(format!(" {}", model_name), style),
        sep.clone(),
        Span::styled(format!("Turn {}", state.turn_count), style),
        sep.clone(),
        Span::styled(format!("Steps {}", state.total_steps), style),
    ];
    let right_spans = vec![
        Span::styled(
            format!(
                "Tkn {}/{}",
                format_count(state.total_input_tokens),
                format_count(state.total_output_tokens),
            ),
            style,
        ),
        sep.clone(),
        Span::styled(format!("{}ms", state.latency_ms), style),
        sep.clone(),
        Span::styled(format!("{} ", feedback), style),
    ];

    let left_len: usize = left_spans.iter().map(|s| s.width()).sum();
    let right_len: usize = right_spans.iter().map(|s| s.width()).sum();
    let gap = width.saturating_sub(left_len + right_len);

    let mut spans = left_spans;
    spans.push(Span::styled(" ".repeat(gap), dim));
    spans.extend(right_spans);

    let line = Line::from(spans);
    frame.render_widget(line, area);
}

/// Format a token count compactly: 1234 → "1.2k", 1234567 → "1.2M".
fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
