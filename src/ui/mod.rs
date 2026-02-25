pub mod chat;
pub mod input;
pub mod layout;
pub mod login;
pub mod modal;
pub mod sidebar;
pub mod status;

use ratatui::Frame;

use crate::app::AppState;

pub fn render(frame: &mut Frame, state: &AppState) {
    match &state.screen {
        crate::app::Screen::Login(login_state) => {
            login::render(frame, frame.area(), login_state);
        }
        crate::app::Screen::App => {
            layout::render(frame, state);
        }
    }
}
