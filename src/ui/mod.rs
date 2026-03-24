pub mod comments;
pub mod diff;
pub mod difftastic;
pub mod help;
pub mod pr_detail;
pub mod pr_list;
pub mod theme;

use crate::app::{App, Screen};
use ratatui::Frame;

pub fn render(f: &mut Frame, app: &mut App) {
    let t = app.theme.clone();

    match &app.screen {
        Screen::PrList => pr_list::render(f, app, &t),
        Screen::PrDetail => pr_detail::render(f, app, &t),
    }

    if app.show_help {
        help::render(f, &t);
    }
}
