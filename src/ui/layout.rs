use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

use super::{chat, input, sidebar, status};
use crate::app::AppState;
use crate::theme;

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Vertical: status(1) + header(1) + content(fill) + session bar(1) + input(3)
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar (keyboard shortcuts)
            Constraint::Length(1), // header bar
            Constraint::Min(4),    // content area
            Constraint::Length(1), // session bar
            Constraint::Length(3), // input bar
        ])
        .split(area);

    status::render(frame, vertical[0]);
    status::render_header(frame, vertical[1], "GHOSTFOLIO AGENT");

    // 1-char padding around the content area
    let content_area = vertical[2].inner(Margin::new(1, 1));

    // Horizontal: chat(fill) + gap(2) + sidebar(SIDEBAR_WIDTH)
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),
            Constraint::Length(2),
            Constraint::Length(theme::SIDEBAR_WIDTH),
        ])
        .split(content_area);

    chat::render(frame, horizontal[0], state);
    sidebar::render(frame, horizontal[2], state);
    status::render_session_bar(frame, vertical[3], state);
    input::render(frame, vertical[4], state);

    // Modal overlay
    if let Some(ref modal) = state.modal {
        let modal_area = centered_rect(60, 60, area);
        super::modal::render(frame, modal_area, modal);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
