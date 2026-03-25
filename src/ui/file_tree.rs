use crate::ui::theme::Theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use std::collections::HashSet;

// ── Tree data model ──────────────────────────────────────────────────────────

/// A flat row in the rendered tree, produced by `build_rows`.
#[derive(Debug, Clone)]
pub struct TreeRow {
    /// Display label (indented directory name or filename).
    pub label: String,
    /// Depth for indentation (each level = 2 spaces).
    #[allow(dead_code)]
    pub depth: usize,
    /// If this row is a file, this holds the index into the original file list.
    pub file_index: Option<usize>,
}

/// Build a flat list of `TreeRow`s from a slice of file paths.
///
/// Paths like `["src/ui/diff.rs", "src/main.rs", "Cargo.toml"]` become:
/// ```
/// Cargo.toml          (file, depth 0)
/// src/                (dir,  depth 0)
///   main.rs           (file, depth 1)
///   ui/               (dir,  depth 1)
///     diff.rs         (file, depth 2)
/// ```
pub fn build_rows(file_paths: &[String]) -> Vec<TreeRow> {
    // Build a prefix tree from the paths.
    let mut root = DirNode::default();

    for (idx, path) in file_paths.iter().enumerate() {
        let parts: Vec<&str> = path.split('/').collect();
        insert(&mut root, &parts, idx);
    }

    let mut rows = Vec::new();
    emit_rows(&root, 0, &mut rows);
    rows
}

// ── Internal prefix-tree ─────────────────────────────────────────────────────

#[derive(Default)]
struct DirNode {
    /// Sorted child directories: (name, subtree).
    dirs: Vec<(String, DirNode)>,
    /// Sorted leaf files: (name, original file index).
    files: Vec<(String, usize)>,
}

fn insert(node: &mut DirNode, parts: &[&str], file_idx: usize) {
    match parts {
        [] => {}
        [name] => {
            // Leaf file
            node.files.push((name.to_string(), file_idx));
        }
        [dir, rest @ ..] => {
            // Find or create sub-directory
            let pos = node.dirs.iter().position(|(n, _)| n == dir);
            if let Some(i) = pos {
                insert(&mut node.dirs[i].1, rest, file_idx);
            } else {
                node.dirs.push((dir.to_string(), DirNode::default()));
                let last = node.dirs.len() - 1;
                insert(&mut node.dirs[last].1, rest, file_idx);
            }
        }
    }
}

fn emit_rows(node: &DirNode, depth: usize, out: &mut Vec<TreeRow>) {
    // Files before sub-directories at the root level keeps root-level files first.
    // Alphabetic sort within each group is maintained by insertion order from parse_diff.

    // Emit files first (at root depth = 0 this feels natural for single-level repos;
    // for deep trees dirs come after files at each level which is consistent with
    // common tree tools).
    for (name, file_idx) in &node.files {
        let indent = "  ".repeat(depth);
        out.push(TreeRow {
            label: format!("{indent}{name}"),
            depth,
            file_index: Some(*file_idx),
        });
    }

    // Then recurse into sub-directories
    for (dir_name, subtree) in &node.dirs {
        let indent = "  ".repeat(depth);
        out.push(TreeRow {
            label: format!("{indent}{dir_name}/"),
            depth,
            file_index: None,
        });
        emit_rows(subtree, depth + 1, out);
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

/// Render the file tree panel.
///
/// * `rows`            – pre-built list from `build_rows`
/// * `tree_cursor`     – currently highlighted row index (including dir rows)
/// * `active_file`     – the currently-viewed file index (highlighted differently)
/// * `focused`         – whether the tree panel has keyboard focus
/// * `reviewed`        – set of file indices that have been marked as reviewed
pub fn render(
    f: &mut Frame,
    rows: &[TreeRow],
    tree_cursor: usize,
    active_file: usize,
    focused: bool,
    area: Rect,
    t: &Theme,
    reviewed: &HashSet<usize>,
) {
    let border_style = if focused {
        t.border_style()
    } else {
        t.border_dim_style()
    };

    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let is_cursor = i == tree_cursor;
            let is_active_file = row.file_index == Some(active_file);
            let is_dir = row.file_index.is_none();
            let is_reviewed = row.file_index.map_or(false, |fi| reviewed.contains(&fi));

            let style = if is_cursor && focused {
                // Cursor row with focus: full selection highlight
                Style::default()
                    .bg(t.selection_bg)
                    .fg(t.selection_fg)
                    .add_modifier(Modifier::BOLD)
            } else if is_active_file {
                // Currently-viewed file (accent color even without focus)
                Style::default()
                    .fg(t.text_accent)
                    .add_modifier(Modifier::BOLD)
            } else if is_reviewed {
                // Reviewed files: dimmed to indicate "done"
                Style::default().fg(t.text_dim).add_modifier(Modifier::DIM)
            } else if is_dir {
                Style::default().fg(t.text_dim)
            } else {
                Style::default().fg(t.text)
            };

            // Determine the prefix:
            //   ▶  active file (takes precedence over reviewed state)
            //   ☑  reviewed file
            //   ☐  unreviewed file
            //   (space) directory
            let prefix = if is_active_file {
                "▶ "
            } else if is_dir {
                "  "
            } else if is_reviewed {
                "☑ "
            } else {
                "☐ "
            };

            ListItem::new(Line::from(Span::styled(
                format!("{}{}", prefix, row.label),
                style,
            )))
        })
        .collect();

    let title_style = if focused {
        t.text_accent_style()
    } else {
        t.text_dim_style()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(t.background_style())
            .title(Span::styled(" Files ", title_style)),
    );

    // Compute scroll offset so the cursor stays visible
    let mut state = ListState::default();
    if !rows.is_empty() {
        state.select(Some(tree_cursor.min(rows.len() - 1)));
    }

    f.render_stateful_widget(list, area, &mut state);
}
