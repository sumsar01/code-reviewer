pub mod comments;
pub mod diff;
pub mod help;
pub mod pr_detail;
pub mod pr_list;

use crate::app::{App, Screen};
use ratatui::Frame;

pub fn render(f: &mut Frame, app: &mut App) {
    match &app.screen {
        Screen::PrList => pr_list::render(f, app),
        Screen::PrDetail => pr_detail::render(f, app),
    }

    if app.show_help {
        help::render(f);
    }
}
