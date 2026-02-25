use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

use super::{chat, input, sidebar, status};
use crate::app::AppState;
use crate::theme;

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    // Vertical: status(1) + header(1) + content(fill) + input(3)
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Length(1), // header bar
            Constraint::Min(4),   // content area
            Constraint::Length(3), // input bar
        ])
        .split(area);

    status::render(frame, vertical[0]);
    status::render_header(frame, vertical[1], "GHOSTFOLIO AGENT");

    // Horizontal: chat(fill) + sidebar(SIDEBAR_WIDTH)
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),
            Constraint::Length(theme::SIDEBAR_WIDTH),
        ])
        .split(vertical[2]);

    chat::render(frame, horizontal[0], state);
    sidebar::render(frame, horizontal[1], state);
    input::render(frame, vertical[3], state);

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
