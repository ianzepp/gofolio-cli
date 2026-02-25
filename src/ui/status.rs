use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::Frame;

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
