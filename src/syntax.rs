//! Syntax highlighting using tree-sitter.
//!
//! The public interface is [`SyntaxHighlighter`], which is created once and
//! reused for every render frame.  Call [`SyntaxHighlighter::highlight`] with
//! a filename and a slice of source lines to get back a `Vec` of per-line
//! span data that `diff.rs` turns into ratatui `Span`s.

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

// ── Highlight capture names ───────────────────────────────────────────────────
//
// These match the standard nvim-treesitter / Helix capture names used by most
// grammar highlight queries.  The index into this slice is the value carried
// inside `HighlightEvent::HighlightStart`.
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",          // 0
    "comment",            // 1
    "constant",           // 2
    "constant.builtin",   // 3
    "constructor",        // 4
    "delimiter",          // 5  — C grammar uses @delimiter (not @punctuation.*)
    "embedded",           // 6
    "escape",             // 7  — escape sequences inside strings (\n, \", …)
    "function",           // 8  — also matches function.method / function.macro / function.special
    "function.builtin",   // 9
    "keyword",            // 10
    "label",              // 11
    "module",             // 12
    "number",             // 13
    "operator",           // 14
    "property",           // 15
    "punctuation",        // 16 — also matches punctuation.bracket / .delimiter / .special
    "string",             // 17
    "string.special",     // 18
    "tag",                // 19
    "type",               // 20
    "type.builtin",       // 21
    "variable",           // 22
    "variable.builtin",   // 23
    "variable.parameter", // 24
];

pub const HIGHLIGHT_COUNT: usize = HIGHLIGHT_NAMES.len();

// ── Highlight index constants (for use in theme.rs) ──────────────────────────
pub const HL_ATTRIBUTE: usize = 0;
pub const HL_COMMENT: usize = 1;
pub const HL_CONSTANT: usize = 2;
pub const HL_CONSTANT_BUILTIN: usize = 3;
pub const HL_CONSTRUCTOR: usize = 4;
pub const HL_DELIMITER: usize = 5;
pub const HL_EMBEDDED: usize = 6;
pub const HL_ESCAPE: usize = 7;
pub const HL_FUNCTION: usize = 8;
pub const HL_FUNCTION_BUILTIN: usize = 9;
pub const HL_KEYWORD: usize = 10;
pub const HL_LABEL: usize = 11;
pub const HL_MODULE: usize = 12;
pub const HL_NUMBER: usize = 13;
pub const HL_OPERATOR: usize = 14;
pub const HL_PROPERTY: usize = 15;
pub const HL_PUNCTUATION: usize = 16;
pub const HL_STRING: usize = 17;
pub const HL_STRING_SPECIAL: usize = 18;
pub const HL_TAG: usize = 19;
pub const HL_TYPE: usize = 20;
pub const HL_TYPE_BUILTIN: usize = 21;
pub const HL_VARIABLE: usize = 22;
pub const HL_VARIABLE_BUILTIN: usize = 23;
pub const HL_VARIABLE_PARAMETER: usize = 24;

// ── A single highlighted span within a line ───────────────────────────────────

/// A byte-range within a source line associated with a highlight index.
/// `hl_index` is an index into [`HIGHLIGHT_NAMES`]; `None` means no highlight
/// (use the default diff colour).
#[derive(Debug, Clone)]
pub struct HlSpan {
    pub start: usize,
    pub end: usize,
    pub hl_index: Option<usize>,
}

// ── SyntaxHighlighter ─────────────────────────────────────────────────────────

/// Owns all tree-sitter [`HighlightConfiguration`]s and performs per-line
/// highlighting.  Cheap to construct; create once and reuse.
pub struct SyntaxHighlighter {
    configs: Vec<(Vec<String>, HighlightConfiguration)>,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let mut configs: Vec<(Vec<String>, HighlightConfiguration)> = Vec::new();

        // ── Rust ──────────────────────────────────────────────────────────────
        if let Ok(mut cfg) = HighlightConfiguration::new(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
            "",
        ) {
            cfg.configure(HIGHLIGHT_NAMES);
            configs.push((vec!["rs".into()], cfg));
        }

        // ── Python ────────────────────────────────────────────────────────────
        if let Ok(mut cfg) = HighlightConfiguration::new(
            tree_sitter_python::LANGUAGE.into(),
            "python",
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
            "",
        ) {
            cfg.configure(HIGHLIGHT_NAMES);
            configs.push((vec!["py".into(), "pyi".into()], cfg));
        }

        // ── JavaScript ───────────────────────────────────────────────────────
        if let Ok(mut cfg) = HighlightConfiguration::new(
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript",
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        ) {
            cfg.configure(HIGHLIGHT_NAMES);
            configs.push((
                vec!["js".into(), "mjs".into(), "cjs".into(), "jsx".into()],
                cfg,
            ));
        }

        // ── TypeScript ───────────────────────────────────────────────────────
        // tree-sitter-typescript's HIGHLIGHTS_QUERY only covers TS-specific tokens
        // (types, TS keywords).  It has no @function captures at all.  We must
        // prepend the JS highlights so that functions, variables, operators, etc.
        // are picked up.  TS rules come first so they take priority over JS where
        // the two overlap (e.g. `string`/`number` → type.builtin, not keyword).
        {
            let ts_highlights = format!(
                "{}\n{}",
                tree_sitter_typescript::HIGHLIGHTS_QUERY,
                tree_sitter_javascript::HIGHLIGHT_QUERY,
            );
            if let Ok(mut cfg) = HighlightConfiguration::new(
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                "typescript",
                &ts_highlights,
                "",
                tree_sitter_typescript::LOCALS_QUERY,
            ) {
                cfg.configure(HIGHLIGHT_NAMES);
                configs.push((vec!["ts".into()], cfg));
            }
        }

        // ── TSX ───────────────────────────────────────────────────────────────
        {
            let tsx_highlights = format!(
                "{}\n{}",
                tree_sitter_typescript::HIGHLIGHTS_QUERY,
                tree_sitter_javascript::HIGHLIGHT_QUERY,
            );
            if let Ok(mut cfg) = HighlightConfiguration::new(
                tree_sitter_typescript::LANGUAGE_TSX.into(),
                "tsx",
                &tsx_highlights,
                "",
                tree_sitter_typescript::LOCALS_QUERY,
            ) {
                cfg.configure(HIGHLIGHT_NAMES);
                configs.push((vec!["tsx".into()], cfg));
            }
        }

        // ── Go ────────────────────────────────────────────────────────────────
        if let Ok(mut cfg) = HighlightConfiguration::new(
            tree_sitter_go::LANGUAGE.into(),
            "go",
            tree_sitter_go::HIGHLIGHTS_QUERY,
            "",
            "",
        ) {
            cfg.configure(HIGHLIGHT_NAMES);
            configs.push((vec!["go".into()], cfg));
        }

        // ── C ─────────────────────────────────────────────────────────────────
        if let Ok(mut cfg) = HighlightConfiguration::new(
            tree_sitter_c::LANGUAGE.into(),
            "c",
            tree_sitter_c::HIGHLIGHT_QUERY,
            "",
            "",
        ) {
            cfg.configure(HIGHLIGHT_NAMES);
            configs.push((vec!["c".into(), "h".into()], cfg));
        }

        // ── C++ ───────────────────────────────────────────────────────────────
        if let Ok(mut cfg) = HighlightConfiguration::new(
            tree_sitter_cpp::LANGUAGE.into(),
            "cpp",
            tree_sitter_cpp::HIGHLIGHT_QUERY,
            "",
            "",
        ) {
            cfg.configure(HIGHLIGHT_NAMES);
            configs.push((
                vec![
                    "cpp".into(),
                    "cc".into(),
                    "cxx".into(),
                    "hpp".into(),
                    "hh".into(),
                    "hxx".into(),
                ],
                cfg,
            ));
        }

        // ── JSON ──────────────────────────────────────────────────────────────
        if let Ok(mut cfg) = HighlightConfiguration::new(
            tree_sitter_json::LANGUAGE.into(),
            "json",
            tree_sitter_json::HIGHLIGHTS_QUERY,
            "",
            "",
        ) {
            cfg.configure(HIGHLIGHT_NAMES);
            configs.push((vec!["json".into()], cfg));
        }

        // ── Bash / Shell ──────────────────────────────────────────────────────
        if let Ok(mut cfg) = HighlightConfiguration::new(
            tree_sitter_bash::LANGUAGE.into(),
            "bash",
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
            "",
        ) {
            cfg.configure(HIGHLIGHT_NAMES);
            configs.push((
                vec!["sh".into(), "bash".into(), "zsh".into(), "fish".into()],
                cfg,
            ));
        }

        // ── TOML ──────────────────────────────────────────────────────────────
        if let Ok(mut cfg) = HighlightConfiguration::new(
            tree_sitter_toml_ng::LANGUAGE.into(),
            "toml",
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
            "",
        ) {
            cfg.configure(HIGHLIGHT_NAMES);
            configs.push((vec!["toml".into()], cfg));
        }

        Self { configs }
    }

    /// Return the `HighlightConfiguration` for a given filename, using the
    /// file extension to determine the language.  Returns `None` for unknown
    /// extensions.
    fn config_for(&self, filename: &str) -> Option<&HighlightConfiguration> {
        let ext = filename.rsplit('.').next()?.to_ascii_lowercase();
        for (exts, cfg) in &self.configs {
            if exts.iter().any(|e| e == &ext) {
                return Some(cfg);
            }
        }
        None
    }

    /// Highlight all lines of `source` as a single document, then split the
    /// resulting spans back by line.
    ///
    /// Returns `None` if the language is unknown or highlighting fails; in
    /// that case the caller should fall back to plain diff colours.
    pub fn highlight(&self, filename: &str, source: &str) -> Option<Vec<Vec<HlSpan>>> {
        let cfg = self.config_for(filename)?;
        let src_bytes = source.as_bytes();

        let mut highlighter = Highlighter::new();
        let events = highlighter.highlight(cfg, src_bytes, None, |_| None).ok()?;

        // Build a flat list of (byte_start, byte_end, hl_index?) spans.
        let mut flat: Vec<(usize, usize, Option<usize>)> = Vec::new();
        let mut stack: Vec<usize> = Vec::new(); // active highlight indices

        for event in events {
            match event.ok()? {
                HighlightEvent::Source { start, end } => {
                    let hl = stack.last().copied();
                    flat.push((start, end, hl));
                }
                HighlightEvent::HighlightStart(h) => {
                    stack.push(h.0);
                }
                HighlightEvent::HighlightEnd => {
                    stack.pop();
                }
            }
        }

        // Split flat spans by line.
        // We need to know byte offsets of each line boundary.
        let line_starts: Vec<usize> = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        let num_lines = line_starts.len();

        let mut lines: Vec<Vec<HlSpan>> = vec![Vec::new(); num_lines];

        for (abs_start, abs_end, hl_index) in flat {
            if abs_start == abs_end {
                continue;
            }
            // Find which line(s) this span overlaps.
            // Most spans fit within a single line; cross-line spans (e.g. multi-line
            // strings) are clipped to each line they touch.
            let first_line = line_starts
                .partition_point(|&ls| ls <= abs_start)
                .saturating_sub(1);
            let last_line = line_starts
                .partition_point(|&ls| ls <= abs_end.saturating_sub(1))
                .saturating_sub(1);

            for li in first_line..=last_line.min(num_lines - 1) {
                let line_start = line_starts[li];
                let line_end = if li + 1 < num_lines {
                    line_starts[li + 1].saturating_sub(1) // exclude the '\n'
                } else {
                    src_bytes.len()
                };

                let span_start = abs_start.max(line_start).saturating_sub(line_start);
                let span_end = abs_end.min(line_end).saturating_sub(line_start);

                if span_start < span_end {
                    lines[li].push(HlSpan {
                        start: span_start,
                        end: span_end,
                        hl_index,
                    });
                }
            }
        }

        Some(lines)
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}
