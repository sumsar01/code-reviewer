//! Syntax highlighting using tree-sitter.
//!
//! The public interface is [`SyntaxHighlighter`], which is created once and
//! reused for every render frame.  Call [`SyntaxHighlighter::highlight`] with
//! a filename and a slice of source lines to get back a `Vec` of per-line
//! span data that `diff.rs` turns into ratatui `Span`s.

use tree_sitter::Language;
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

/// Descriptor for a single language grammar used to build highlight configs.
struct LangSpec {
    /// Display name passed to [`HighlightConfiguration::new`].
    name: &'static str,
    /// File extensions that map to this language.
    exts: &'static [&'static str],
    /// The tree-sitter language.
    language: Language,
    /// The highlights query string.
    highlights: &'static str,
    /// The injections query string (empty string if none).
    injections: &'static str,
    /// The locals query string (empty string if none).
    locals: &'static str,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        // Build the TypeScript / TSX highlight queries by prepending the TS-specific
        // captures before the shared JS captures so that TS rules take priority.
        let ts_highlights: String = format!(
            "{}\n{}",
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            tree_sitter_javascript::HIGHLIGHT_QUERY,
        );
        let tsx_highlights: String = ts_highlights.clone();

        let specs: Vec<LangSpec> = vec![
            LangSpec {
                name: "rust",
                exts: &["rs"],
                language: tree_sitter_rust::LANGUAGE.into(),
                highlights: tree_sitter_rust::HIGHLIGHTS_QUERY,
                injections: tree_sitter_rust::INJECTIONS_QUERY,
                locals: "",
            },
            LangSpec {
                name: "python",
                exts: &["py", "pyi"],
                language: tree_sitter_python::LANGUAGE.into(),
                highlights: tree_sitter_python::HIGHLIGHTS_QUERY,
                injections: "",
                locals: "",
            },
            LangSpec {
                name: "javascript",
                exts: &["js", "mjs", "cjs", "jsx"],
                language: tree_sitter_javascript::LANGUAGE.into(),
                highlights: tree_sitter_javascript::HIGHLIGHT_QUERY,
                injections: tree_sitter_javascript::INJECTIONS_QUERY,
                locals: tree_sitter_javascript::LOCALS_QUERY,
            },
            LangSpec {
                name: "go",
                exts: &["go"],
                language: tree_sitter_go::LANGUAGE.into(),
                highlights: tree_sitter_go::HIGHLIGHTS_QUERY,
                injections: "",
                locals: "",
            },
            LangSpec {
                name: "c",
                exts: &["c", "h"],
                language: tree_sitter_c::LANGUAGE.into(),
                highlights: tree_sitter_c::HIGHLIGHT_QUERY,
                injections: "",
                locals: "",
            },
            LangSpec {
                name: "cpp",
                exts: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
                language: tree_sitter_cpp::LANGUAGE.into(),
                highlights: tree_sitter_cpp::HIGHLIGHT_QUERY,
                injections: "",
                locals: "",
            },
            LangSpec {
                name: "json",
                exts: &["json"],
                language: tree_sitter_json::LANGUAGE.into(),
                highlights: tree_sitter_json::HIGHLIGHTS_QUERY,
                injections: "",
                locals: "",
            },
            LangSpec {
                name: "bash",
                exts: &["sh", "bash", "zsh", "fish"],
                language: tree_sitter_bash::LANGUAGE.into(),
                highlights: tree_sitter_bash::HIGHLIGHT_QUERY,
                injections: "",
                locals: "",
            },
            LangSpec {
                name: "toml",
                exts: &["toml"],
                language: tree_sitter_toml_ng::LANGUAGE.into(),
                highlights: tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
                injections: "",
                locals: "",
            },
        ];

        let mut configs: Vec<(Vec<String>, HighlightConfiguration)> =
            Vec::with_capacity(specs.len() + 2);

        // Register all simple (non-composed) languages.
        for spec in specs {
            if let Ok(mut cfg) = HighlightConfiguration::new(
                spec.language.clone(),
                spec.name,
                spec.highlights,
                spec.injections,
                spec.locals,
            ) {
                cfg.configure(HIGHLIGHT_NAMES);
                configs.push((spec.exts.iter().map(|e| (*e).into()).collect(), cfg));
            }
        }

        // TypeScript — composed highlights (TS rules + JS rules).
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

        // TSX — same composed highlights, TSX language.
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
