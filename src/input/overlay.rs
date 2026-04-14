use crate::app::{App, SearchState};
use crate::ui::theme::{Theme, ALL_THEMES};
use crossterm::event::{KeyCode, KeyModifiers};
use std::sync::Arc;

pub fn handle_key_review_overlay(app: &mut App, code: KeyCode, mods: KeyModifiers) -> bool {
    // Ctrl+Enter submits; Esc cancels; everything else edits the buffer.
    if mods.contains(KeyModifiers::CONTROL) {
        match code {
            // KeyCode::Enter requires the keyboard enhancement protocol (enabled in main.rs).
            // On terminals without enhancement (e.g. macOS Terminal), Ctrl+Enter sends
            // Ctrl+m (carriage return), so we handle both.
            // KeyCode::Char('s') is an explicit Ctrl+S fallback shown in the hint bar.
            KeyCode::Enter | KeyCode::Char('m') | KeyCode::Char('s') => {
                app.submit_review_overlay();
                return false;
            }
            _ => {}
        }
    }

    // Esc closes the overlay without needing a mutable borrow of its contents.
    if code == KeyCode::Esc {
        app.review_overlay = None;
        return false;
    }

    // All remaining keys mutate the overlay buffer — bail early if there is none.
    let Some(overlay) = app.review_overlay.as_mut() else {
        return false;
    };

    match code {
        KeyCode::Enter => overlay.insert_newline(),
        KeyCode::Backspace => overlay.backspace(),
        KeyCode::Left => overlay.move_left(),
        KeyCode::Right => overlay.move_right(),
        KeyCode::Up => overlay.move_up(),
        KeyCode::Down => overlay.move_down(),
        KeyCode::Char(ch) => overlay.insert_char(ch),
        _ => {}
    }
    false
}

pub fn handle_key_update_prompt(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            // Signal that the user confirmed the update.  Returning `true` causes
            // the run loop to exit so the TUI is torn down before cargo runs.
            app.update_confirmed = true;
            true // exit run loop
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.update_available = None;
            false
        }
        _ => false,
    }
}

pub fn handle_key_theme_picker(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if app.theme_picker_cursor + 1 < ALL_THEMES.len() {
                app.theme_picker_cursor += 1;
                app.theme = Arc::new(Theme::from_name(ALL_THEMES[app.theme_picker_cursor].0));
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.theme_picker_cursor > 0 {
                app.theme_picker_cursor -= 1;
                app.theme = Arc::new(Theme::from_name(ALL_THEMES[app.theme_picker_cursor].0));
            }
        }
        KeyCode::Enter => {
            app.config.ui.theme = ALL_THEMES[app.theme_picker_cursor].0.to_string();
            app.theme_picker_original = None;
            app.show_theme_picker = false;
        }
        KeyCode::Esc => {
            if let Some(original) = app.theme_picker_original.take() {
                app.theme = original;
            }
            app.show_theme_picker = false;
        }
        _ => {}
    }
    false
}

pub fn handle_key_repo_switcher(app: &mut App, code: KeyCode, mods: KeyModifiers) -> bool {
    // ── Ctrl+W — delete last word ────────────────────────────────────────
    if mods.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char('w') = code {
            if let Some(state) = app.repo_switcher.as_mut() {
                let q = &mut state.query;
                // Trim trailing spaces then pop until the previous word boundary
                // (space or '/') so `ctrl+w` over "rust-lang/rust" backs to "rust-lang/".
                while q.ends_with(' ') {
                    q.pop();
                }
                while !q.is_empty() && !q.ends_with('/') && !q.ends_with(' ') {
                    q.pop();
                }
                state.cursor = 0;
                state.results.clear();
                if q.is_empty() {
                    state.search_state = SearchState::Idle;
                } else {
                    state.search_state = SearchState::Loading;
                    state.last_keystroke = Some(std::time::Instant::now());
                }
            }
            return false;
        }
    }

    // Esc closes the switcher without a mutable borrow on its contents.
    if code == KeyCode::Esc {
        app.repo_switcher = None;
        return false;
    }

    let Some(state) = app.repo_switcher.as_mut() else {
        return false;
    };

    match code {
        KeyCode::Enter => {
            let query = app
                .repo_switcher
                .as_ref()
                .map(|s| s.query.trim().to_string())
                .unwrap_or_default();

            // If the query looks like "owner/repo" already, switch directly
            // without requiring a search result to be selected.
            let target = if crate::app::looks_like_full_name(&query) {
                Some(query)
            } else {
                app.repo_switcher
                    .as_ref()
                    .and_then(|s| s.selected_full_name())
            };

            if let Some(full_name) = target {
                app.repo_switcher = None;
                app.switch_repo(&full_name);
            }
        }
        KeyCode::Tab => {
            // Fill the input with the currently highlighted suggestion so
            // the user can refine it before confirming.
            if let Some(name) = state.selected_full_name() {
                state.query = name;
                state.cursor = 0;
                state.results.clear();
                state.search_state = SearchState::Loading;
                state.last_keystroke = Some(std::time::Instant::now());
            }
        }
        KeyCode::Down => {
            let count = state.suggestion_count();
            if count > 0 && state.cursor + 1 < count {
                state.cursor += 1;
            }
        }
        KeyCode::Up => {
            state.cursor = state.cursor.saturating_sub(1);
        }
        KeyCode::Backspace => {
            state.query.pop();
            state.cursor = 0;
            state.results.clear();
            if state.query.is_empty() {
                state.search_state = SearchState::Idle;
            } else {
                state.search_state = SearchState::Loading;
                state.last_keystroke = Some(std::time::Instant::now());
            }
        }
        KeyCode::Char(ch) => {
            state.query.push(ch);
            state.cursor = 0;
            state.search_state = SearchState::Loading;
            state.last_keystroke = Some(std::time::Instant::now());
        }
        _ => {}
    }
    false
}
