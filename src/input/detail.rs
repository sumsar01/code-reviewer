use crate::app::{App, DetailFocus, DetailTab, ReviewAction, ReviewOverlayState, Screen};
use crate::ui::theme::Theme;
use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Instant;

pub fn handle_key_detail(app: &mut App, code: KeyCode, mods: KeyModifiers) -> bool {
    // ── File-tree focus: intercept j/k/Enter/Esc ─────────────────────────
    if app.detail_focus == DetailFocus::FileTree {
        app.pending_count.clear();
        app.g_pending = false;
        return handle_key_file_tree(app, code);
    }

    // ── Count prefix accumulation (digits 0-9) ───────────────────────────
    // '0' with an existing count prefix is a count digit; bare '0' is "scroll to column 0".
    if let KeyCode::Char(c) = code {
        if c.is_ascii_digit() && (c != '0' || !app.pending_count.is_empty()) {
            app.pending_count.push(c);
            app.g_pending = false;
            return false;
        }
    }

    // Consume the count (default 1) and clear the buffer.
    let count: u16 = app.pending_count.parse().unwrap_or(1).max(1);
    app.pending_count.clear();

    // ── Ctrl-d / Ctrl-u (half-page) ──────────────────────────────────────
    if mods.contains(KeyModifiers::CONTROL) {
        let half = (app.last_diff_area_height / 2).max(1);
        let step = half.saturating_mul(count);
        match code {
            KeyCode::Char('d') => {
                app.g_pending = false;
                match app.detail_tab {
                    DetailTab::Diff => {
                        let max = app.max_diff_scroll();
                        app.diff_scroll = app.diff_scroll.saturating_add(step).min(max);
                    }
                    DetailTab::Comments => {
                        let max = app.max_comments_scroll();
                        app.comments_scroll = app.comments_scroll.saturating_add(step).min(max);
                    }
                    DetailTab::Difftastic => {
                        let max = app.max_difft_scroll();
                        app.difft_scroll = app.difft_scroll.saturating_add(step).min(max);
                    }
                }
                return false;
            }
            KeyCode::Char('u') => {
                app.g_pending = false;
                match app.detail_tab {
                    DetailTab::Diff => app.diff_scroll = app.diff_scroll.saturating_sub(step),
                    DetailTab::Comments => {
                        app.comments_scroll = app.comments_scroll.saturating_sub(step)
                    }
                    DetailTab::Difftastic => {
                        app.difft_scroll = app.difft_scroll.saturating_sub(step)
                    }
                }
                return false;
            }
            _ => {}
        }
    }

    match code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.g_pending = false;
            app.diff_scroll = 0;
            app.diff_hscroll = 0;
            app.comments_scroll = 0;
            app.diff_file_cursor = 0;
            app.diff_line_cursor = 0;
            // If we entered PrDetail from the ReviewRequests screen, restore
            // the saved PR list and go back there instead of PrList.
            if app.prev_screen == Screen::ReviewRequests {
                if let Some((prs, cursor)) = app.saved_prs.take() {
                    app.prs = prs;
                    app.pr_cursor = cursor;
                }
                app.prev_screen = Screen::PrList;
                app.screen = Screen::ReviewRequests;
            } else {
                app.screen = Screen::PrList;
            }
        }
        KeyCode::Char('?') => {
            app.g_pending = false;
            app.show_help = true;
        }
        // Space: toggle tree panel visibility
        KeyCode::Char(' ') => {
            app.g_pending = false;
            match app.detail_tab {
                DetailTab::Diff | DetailTab::Difftastic => {
                    app.show_file_tree = !app.show_file_tree;
                    if !app.show_file_tree {
                        app.detail_focus = DetailFocus::Content;
                    }
                }
                DetailTab::Comments => {} // tree not shown on Comments tab
            }
        }
        KeyCode::Tab => {
            app.g_pending = false;
            if app.show_file_tree
                && matches!(app.detail_tab, DetailTab::Diff | DetailTab::Difftastic)
            {
                // Cycle focus: Content → FileTree → Content
                app.detail_focus = match app.detail_focus {
                    DetailFocus::Content => DetailFocus::FileTree,
                    DetailFocus::FileTree => DetailFocus::Content,
                };
            } else {
                // No tree visible: cycle tabs as before
                app.detail_tab = match app.detail_tab {
                    DetailTab::Diff => DetailTab::Comments,
                    DetailTab::Comments => DetailTab::Difftastic,
                    DetailTab::Difftastic => DetailTab::Diff,
                };
            }
        }

        // ── Vertical scroll: j / Down ────────────────────────────────────
        KeyCode::Char('j') | KeyCode::Down => {
            app.g_pending = false;
            match app.detail_tab {
                DetailTab::Diff => {
                    let max = app.max_diff_scroll();
                    app.diff_scroll = app.diff_scroll.saturating_add(count).min(max);
                    app.diff_line_cursor = app
                        .diff_line_cursor
                        .saturating_add(count as usize)
                        .min(max as usize);
                }
                DetailTab::Comments => {
                    let max = app.max_comments_scroll();
                    app.comments_scroll = app.comments_scroll.saturating_add(count).min(max);
                }
                DetailTab::Difftastic => {
                    let max = app.max_difft_scroll();
                    app.difft_scroll = app.difft_scroll.saturating_add(count).min(max);
                }
            }
        }

        // ── Vertical scroll: k / Up ──────────────────────────────────────
        KeyCode::Char('k') | KeyCode::Up => {
            app.g_pending = false;
            match app.detail_tab {
                DetailTab::Diff => {
                    app.diff_scroll = app.diff_scroll.saturating_sub(count);
                    app.diff_line_cursor = app.diff_line_cursor.saturating_sub(count as usize);
                }
                DetailTab::Comments => {
                    app.comments_scroll = app.comments_scroll.saturating_sub(count)
                }
                DetailTab::Difftastic => app.difft_scroll = app.difft_scroll.saturating_sub(count),
            }
        }

        // ── Horizontal scroll: h / Left / l / Right ──────────────────────
        KeyCode::Char('h') | KeyCode::Left => {
            app.g_pending = false;
            match app.detail_tab {
                DetailTab::Diff => app.diff_hscroll = app.diff_hscroll.saturating_sub(count),
                DetailTab::Difftastic => {
                    app.difft_hscroll = app.difft_hscroll.saturating_sub(count)
                }
                DetailTab::Comments => {}
            }
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.g_pending = false;
            match app.detail_tab {
                DetailTab::Diff => app.diff_hscroll = app.diff_hscroll.saturating_add(count),
                DetailTab::Difftastic => {
                    app.difft_hscroll = app.difft_hscroll.saturating_add(count)
                }
                DetailTab::Comments => {}
            }
        }

        // ── G — jump to bottom ───────────────────────────────────────────
        KeyCode::Char('G') => {
            app.g_pending = false;
            match app.detail_tab {
                DetailTab::Diff => {
                    let max = app.max_diff_scroll();
                    app.diff_scroll = max;
                    app.diff_line_cursor = max as usize;
                }
                DetailTab::Comments => app.comments_scroll = app.max_comments_scroll(),
                DetailTab::Difftastic => app.difft_scroll = app.max_difft_scroll(),
            }
        }

        // ── g — first press arms gg; second press jumps to top ───────────
        KeyCode::Char('g') => {
            if app.g_pending {
                // gg: jump to top
                app.g_pending = false;
                match app.detail_tab {
                    DetailTab::Diff => {
                        app.diff_scroll = 0;
                        app.diff_line_cursor = 0;
                    }
                    DetailTab::Comments => app.comments_scroll = 0,
                    DetailTab::Difftastic => app.difft_scroll = 0,
                }
            } else {
                app.g_pending = true;
            }
        }

        // ── 0 — scroll to leftmost column (bare zero, no count prefix) ───
        KeyCode::Char('0') => {
            app.g_pending = false;
            match app.detail_tab {
                DetailTab::Diff => app.diff_hscroll = 0,
                DetailTab::Difftastic => app.difft_hscroll = 0,
                DetailTab::Comments => {}
            }
        }

        // ── $ — scroll to far right (large sentinel value) ───────────────
        KeyCode::Char('$') => {
            app.g_pending = false;
            match app.detail_tab {
                DetailTab::Diff => app.diff_hscroll = u16::MAX,
                DetailTab::Difftastic => app.difft_hscroll = u16::MAX,
                DetailTab::Comments => {}
            }
        }

        // ── File navigation: n / N ────────────────────────────────────────
        KeyCode::Char('n') => {
            app.g_pending = false;
            match app.detail_tab {
                DetailTab::Difftastic => {
                    if !app.difft_files.is_empty() {
                        app.difft_file_cursor =
                            (app.difft_file_cursor + count as usize).min(app.difft_files.len() - 1);
                        app.difft_scroll = 0;
                    }
                }
                _ => {
                    if !app.diff_files.is_empty() {
                        app.diff_file_cursor =
                            (app.diff_file_cursor + count as usize).min(app.diff_files.len() - 1);
                        app.diff_scroll = 0;
                        app.diff_line_cursor = 0;
                    }
                }
            }
        }
        KeyCode::Char('N') => {
            app.g_pending = false;
            match app.detail_tab {
                DetailTab::Difftastic => {
                    app.difft_file_cursor = app.difft_file_cursor.saturating_sub(count as usize);
                    app.difft_scroll = 0;
                }
                _ => {
                    app.diff_file_cursor = app.diff_file_cursor.saturating_sub(count as usize);
                    app.diff_scroll = 0;
                    app.diff_line_cursor = 0;
                }
            }
        }

        KeyCode::Char('o') => {
            app.g_pending = false;
            if let Some(pr) = app.prs.get(app.pr_cursor) {
                let _ = open::that(&pr.url);
            }
        }
        // ── Stack navigation: [ = go to PR below, ] = go to PR above ────
        KeyCode::Char('[') => {
            app.g_pending = false;
            app.navigate_stack(false); // go toward trunk (below)
        }
        KeyCode::Char(']') => {
            app.g_pending = false;
            app.navigate_stack(true); // go away from trunk (above)
        }
        KeyCode::Char('v') => {
            app.g_pending = false;
            app.toggle_reviewed();
        }
        KeyCode::Char('c') => {
            app.g_pending = false;
            app.checkout_pr_branch();
        }
        KeyCode::Char('T') => {
            app.g_pending = false;
            app.theme_picker_original = Some(app.theme.clone());
            app.theme_picker_cursor = Theme::index_of(&app.config.ui.theme);
            app.show_theme_picker = true;
        }
        KeyCode::Char('A') => {
            app.g_pending = false;
            app.status_message = None;
            app.review_overlay = Some(ReviewOverlayState::new(ReviewAction::Approve));
        }
        KeyCode::Char('R') => {
            app.g_pending = false;
            app.status_message = None;
            app.review_overlay = Some(ReviewOverlayState::new(ReviewAction::RequestChanges));
        }
        KeyCode::Char('C') => {
            app.g_pending = false;
            app.status_message = None;
            app.review_overlay = Some(ReviewOverlayState::new(ReviewAction::Comment));
        }
        // ── i — inline comment on the current diff line ──────────────────
        KeyCode::Char('i') => {
            app.g_pending = false;
            if app.detail_tab == DetailTab::Diff {
                if let Some((path, diff_line)) = app.diff_line_at_cursor() {
                    let (line, side) = match diff_line.kind {
                        crate::github::DiffLineKind::Removed => {
                            (diff_line.left_no.unwrap_or(1) as u64, "LEFT".to_string())
                        }
                        _ => (diff_line.right_no.unwrap_or(1) as u64, "RIGHT".to_string()),
                    };
                    app.status_message = None;
                    app.review_overlay =
                        Some(ReviewOverlayState::new(ReviewAction::InlineComment {
                            path,
                            line,
                            side,
                        }));
                } else {
                    app.status_message = Some((
                        "Cursor is on a hunk header — move to a diff line first".to_string(),
                        Instant::now(),
                    ));
                }
            }
        }
        // ── Enter — peek at existing comments on the cursor diff line ────
        KeyCode::Enter => {
            app.g_pending = false;
            if app.detail_tab == DetailTab::Diff {
                if let Some(comments) = app.comments_at_cursor() {
                    app.comment_peek = Some(comments);
                }
            }
        }
        _ => {
            // Any unrecognised key clears the g-pending state.
            app.g_pending = false;
        }
    }
    false
}

/// Key handler when the file-tree sidebar has focus.
pub fn handle_key_file_tree(app: &mut App, code: KeyCode) -> bool {
    use crate::ui::file_tree;

    // Build current rows so we can do index arithmetic.
    let rows: Vec<file_tree::TreeRow> = match app.detail_tab {
        DetailTab::Diff => {
            let paths: Vec<String> = app.diff_files.iter().map(|f| f.filename.clone()).collect();
            file_tree::build_rows(&paths)
        }
        DetailTab::Difftastic => {
            let paths: Vec<String> = app.difft_files.iter().map(|(n, _)| n.clone()).collect();
            file_tree::build_rows(&paths)
        }
        DetailTab::Comments => {
            // Tree not shown on Comments; transfer focus back
            app.detail_focus = DetailFocus::Content;
            return false;
        }
    };

    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if !rows.is_empty() {
                app.file_tree_cursor = (app.file_tree_cursor + 1).min(rows.len() - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.file_tree_cursor = app.file_tree_cursor.saturating_sub(1);
        }
        KeyCode::Enter => {
            // Jump to the file the cursor points at (skip directory rows)
            if let Some(row) = rows.get(app.file_tree_cursor) {
                if let Some(file_idx) = row.file_index {
                    match app.detail_tab {
                        DetailTab::Diff => {
                            app.diff_file_cursor = file_idx;
                            app.diff_scroll = 0;
                            app.diff_line_cursor = 0;
                        }
                        DetailTab::Difftastic => {
                            app.difft_file_cursor = file_idx;
                            app.difft_scroll = 0;
                        }
                        DetailTab::Comments => {}
                    }
                    // Move focus back to content after selecting
                    app.detail_focus = DetailFocus::Content;
                }
            }
        }
        KeyCode::Char(' ') | KeyCode::Esc => {
            // Close tree / return focus to content
            app.detail_focus = DetailFocus::Content;
        }
        KeyCode::Tab => {
            app.detail_focus = DetailFocus::Content;
        }
        _ => {}
    }
    false
}
