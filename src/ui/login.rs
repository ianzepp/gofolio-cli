use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::theme;

#[derive(Debug, Clone)]
pub struct LoginState {
    pub url: String,
    pub token: String,
    pub focus: LoginField,
    pub error: Option<String>,
    pub authenticating: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoginField {
    Url,
    Token,
}

impl Default for LoginState {
    fn default() -> Self {
        Self {
            url: "http://localhost:3333".to_string(),
            token: String::new(),
            focus: LoginField::Url,
            error: None,
            authenticating: false,
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &LoginState) {
    // Center the dialog
    let dialog = centered_rect(50, 14, area);

    frame.render_widget(Clear, dialog);

    let block = Block::default()
        .title(Span::styled(
            " GHOSTFOLIO CLI ",
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(theme::AMBER));

    let inner = block.inner(dialog);
    frame.render_widget(block, dialog);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // spacer
            Constraint::Length(1), // URL label
            Constraint::Length(1), // URL input
            Constraint::Length(1), // spacer
            Constraint::Length(1), // Token label
            Constraint::Length(1), // Token input
            Constraint::Length(1), // spacer
            Constraint::Length(1), // status/error
            Constraint::Length(1), // hint
            Constraint::Min(0),
        ])
        .split(inner);

    // URL field
    let url_label_style = if state.focus == LoginField::Url {
        Style::default()
            .fg(theme::AMBER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::MUTED)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("Server URL:", url_label_style))),
        chunks[1],
    );

    let url_style = if state.focus == LoginField::Url {
        Style::default().fg(theme::WHITE)
    } else {
        Style::default().fg(theme::MUTED)
    };
    let url_display = if state.focus == LoginField::Url {
        format!("{}\u{2588}", state.url)
    } else {
        state.url.clone()
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(url_display, url_style))),
        chunks[2],
    );

    // Token field
    let token_label_style = if state.focus == LoginField::Token {
        Style::default()
            .fg(theme::AMBER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::MUTED)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("Access Token:", token_label_style))),
        chunks[4],
    );

    let token_style = if state.focus == LoginField::Token {
        Style::default().fg(theme::WHITE)
    } else {
        Style::default().fg(theme::MUTED)
    };
    let token_display = if state.focus == LoginField::Token {
        format!("{}\u{2588}", "*".repeat(state.token.len()))
    } else if state.token.is_empty() {
        "(not set)".to_string()
    } else {
        "*".repeat(state.token.len())
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(token_display, token_style))),
        chunks[5],
    );

    // Status or error
    if let Some(ref err) = state.error {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                err.clone(),
                Style::default().fg(theme::RED),
            ))),
            chunks[7],
        );
    } else if state.authenticating {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Authenticating...",
                Style::default().fg(theme::AMBER),
            ))),
            chunks[7],
        );
    }

    // Hint
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Tab: switch field | Enter: connect | Ctrl+Q: quit",
            Style::default().fg(theme::MUTED),
        ))),
        chunks[8],
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
