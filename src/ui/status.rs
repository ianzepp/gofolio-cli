use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::agent::types::ConfidenceLabel;
use crate::app::AppState;
use crate::theme;

pub fn render(frame: &mut Frame, area: Rect) {
    let shortcuts = vec![
        ("N", "New"),
        ("Y", "Up"),
        ("R", "Report"),
        ("P", "Model"),
        ("L", "Logout"),
        ("Q", "Quit"),
        ("PgUp/Dn", "Scroll"),
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
pub fn render_header(frame: &mut Frame, area: Rect, title: &str, right_info: &str) {
    let width = area.width as usize;
    let left = format!(" {title} ");
    let right = format!(" {right_info} ");
    let display = if left.len() + right.len() < width {
        format!(
            "{left}{}{}",
            " ".repeat(width - left.len() - right.len()),
            right
        )
    } else {
        left.chars().take(width).collect::<String>()
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
        .bg(theme::BORDER)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(theme::MUTED).bg(theme::BORDER);

    let model_name = state.model.split('/').next_back().unwrap_or(&state.model);

    let sep = Span::styled(" \u{00B7} ", dim);

    let left_spans = vec![
        Span::styled(format!(" {}", model_name), style),
        sep.clone(),
        Span::styled(format!("Turn {}", state.turn_count), style),
        sep.clone(),
        Span::styled(format!("Steps {}", state.total_steps), style),
        sep.clone(),
        Span::styled(format!("Tools {}", state.total_tool_calls), style),
    ];
    // Context meter — based on last API call's input tokens (actual context window usage)
    let max_context: u64 = 200_000;
    let pct = if max_context > 0 {
        (state.last_input_tokens as f64 / max_context as f64 * 100.0).min(100.0)
    } else {
        0.0
    };
    let ctx_color = if pct >= 85.0 {
        theme::RED
    } else if pct >= 70.0 {
        theme::WARNING
    } else {
        theme::AMBER
    };
    let ctx_bar = render_braille_bar(pct);

    // Current time
    let now = chrono::Local::now();
    let time_str = now.format("%H:%M").to_string();

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
        Span::styled(
            format!(
                "V:{} {:.0}%",
                confidence_short(&state.confidence_label),
                state.confidence_score * 100.0
            ),
            Style::default()
                .fg(if state.verified {
                    theme::GREEN
                } else {
                    theme::WARNING
                })
                .bg(theme::BORDER),
        ),
        sep.clone(),
        Span::styled(
            format!("{} {:.0}%", ctx_bar, pct),
            Style::default().fg(ctx_color).bg(theme::BORDER),
        ),
        sep.clone(),
        Span::styled(format!("{} ", time_str), style),
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

// Braille snake-fill glyphs: 8 sub-steps per character cell.
// Snake pattern: left column bottom-up, then right column top-down.
// step 0 = empty (─), step 8 = full (█)
const BRAILLE_STEPS: [char; 9] = [
    '\u{2500}', // 0 - empty ─
    '\u{2840}', // 1 - ⡀
    '\u{2844}', // 2 - ⡄
    '\u{2846}', // 3 - ⡆
    '\u{2847}', // 4 - ⡇
    '\u{28C7}', // 5 - ⣇
    '\u{28E7}', // 6 - ⣧
    '\u{28F7}', // 7 - ⣷
    '\u{2588}', // 8 - █
];

const CTX_BAR_WIDTH: usize = 10;

fn render_braille_bar(pct: f64) -> String {
    let total_steps = CTX_BAR_WIDTH * 8;
    let filled_steps = ((pct / 100.0) * total_steps as f64).round() as usize;
    let filled_steps = filled_steps.min(total_steps);

    let mut bar = String::with_capacity(CTX_BAR_WIDTH * 4);
    for i in 0..CTX_BAR_WIDTH {
        let cell_filled = filled_steps.saturating_sub(i * 8).min(8);
        bar.push(BRAILLE_STEPS[cell_filled]);
    }
    bar
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

fn confidence_short(label: &ConfidenceLabel) -> &'static str {
    match label {
        ConfidenceLabel::High => "HIGH",
        ConfidenceLabel::Medium => "MED",
        ConfidenceLabel::Low => "LOW",
    }
}
