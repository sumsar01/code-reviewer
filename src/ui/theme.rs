use crate::syntax::{
    HIGHLIGHT_COUNT, HL_ATTRIBUTE, HL_COMMENT, HL_CONSTANT, HL_CONSTANT_BUILTIN, HL_CONSTRUCTOR,
    HL_DELIMITER, HL_EMBEDDED, HL_ESCAPE, HL_FUNCTION, HL_FUNCTION_BUILTIN, HL_KEYWORD, HL_LABEL,
    HL_MODULE, HL_NUMBER, HL_OPERATOR, HL_PROPERTY, HL_PUNCTUATION, HL_STRING, HL_STRING_SPECIAL,
    HL_TAG, HL_TYPE, HL_TYPE_BUILTIN, HL_VARIABLE, HL_VARIABLE_BUILTIN, HL_VARIABLE_PARAMETER,
};
use ratatui::style::{Color, Modifier, Style};

/// All named color/style slots used across the UI.
///
/// Adding a new theme: implement a new constructor (`Theme::my_theme() -> Theme`)
/// and add an entry to `ALL_THEMES` and `from_name`. No render function needs to change.
#[derive(Debug, Clone)]
pub struct Theme {
    // ── Background ────────────────────────────────────────────────────────────
    /// Main panel / app background. Applied to all blocks so the terminal's own
    /// background colour does not bleed through.
    pub background: Color,

    // ── Chrome ────────────────────────────────────────────────────────────────
    /// Primary border color (blocks, panels).
    pub border: Color,
    /// Dimmer border color (inner / secondary panels).
    pub border_dim: Color,

    // ── Text ─────────────────────────────────────────────────────────────────
    /// Normal body text (near-white on dark backgrounds).
    pub text: Color,
    /// Secondary / muted text (timestamps, stats, labels).
    pub text_dim: Color,
    /// Accent text (repo name, file paths, branch names).
    pub text_accent: Color,

    // ── PR list ───────────────────────────────────────────────────────────────
    /// `#123` PR number.
    pub pr_number: Color,
    /// Author name.
    pub pr_author: Color,
    /// `DRAFT` badge text.
    pub pr_draft: Color,
    /// Selected row background.
    pub selection_bg: Color,
    /// Selected row foreground.
    pub selection_fg: Color,

    // ── Diff ──────────────────────────────────────────────────────────────────
    pub diff_added_fg: Color,
    pub diff_added_bg: Color,
    pub diff_removed_fg: Color,
    pub diff_removed_bg: Color,
    /// Context (unchanged) lines.
    pub diff_context: Color,
    /// `@@` hunk header lines.
    pub diff_hunk: Color,

    // ── Tabs ─────────────────────────────────────────────────────────────────
    pub tab_active: Color,
    pub tab_inactive: Color,

    // ── Key hint badges ───────────────────────────────────────────────────────
    /// Foreground on key badge (dark, so the bg is readable).
    pub key_fg: Color,
    /// Background of key badge.
    pub key_bg: Color,
    /// Description text next to a key badge.
    pub key_desc: Color,

    // ── PR stats ─────────────────────────────────────────────────────────────
    pub stats_added: Color,
    pub stats_removed: Color,

    // ── Misc ─────────────────────────────────────────────────────────────────
    /// Section headers in help / comments.
    pub section_header: Color,
    /// `──` separators in comments.
    pub separator: Color,

    // ── Syntax highlighting ───────────────────────────────────────────────────
    /// Per-capture-index foreground color for syntax tokens.
    /// Indexed by the constants in `crate::syntax` (HL_KEYWORD, HL_FUNCTION, …).
    pub syntax_colors: [Color; HIGHLIGHT_COUNT],
}

/// Registry of all bundled themes: (id, display name).
/// Order here is the order shown in the picker.
pub const ALL_THEMES: &[(&str, &str)] = &[
    ("tokyonight", "Tokyo Night"),
    ("gruvbox", "Gruvbox Dark"),
    ("catppuccin", "Catppuccin Mocha"),
    ("nord", "Nord"),
    ("dracula", "Dracula"),
    ("rosepine", "Rosé Pine"),
    ("github_light", "GitHub Light"),
    ("catppuccin_latte", "Catppuccin Latte"),
    ("rosepine_dawn", "Rosé Pine Dawn"),
    ("gruvbox_light", "Gruvbox Light"),
    ("solarized_light", "Solarized Light"),
];

#[allow(dead_code)]
impl Theme {
    /// Resolve a theme by id (case-insensitive). Unknown ids fall back to
    /// `tokyonight` so old/bad configs never break.
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "tokyonight" => Self::tokyonight(),
            "gruvbox" => Self::gruvbox(),
            "catppuccin" => Self::catppuccin(),
            "nord" => Self::nord(),
            "dracula" => Self::dracula(),
            "rosepine" => Self::rosepine(),
            "github_light" => Self::github_light(),
            "catppuccin_latte" => Self::catppuccin_latte(),
            "rosepine_dawn" => Self::rosepine_dawn(),
            "gruvbox_light" => Self::gruvbox_light(),
            "solarized_light" => Self::solarized_light(),
            _ => Self::tokyonight(),
        }
    }

    /// Return the index of `name` in ALL_THEMES, defaulting to 0.
    pub fn index_of(name: &str) -> usize {
        let lower = name.to_lowercase();
        ALL_THEMES
            .iter()
            .position(|(id, _)| *id == lower.as_str())
            .unwrap_or(0)
    }

    // ── Theme constructors ────────────────────────────────────────────────────

    /// Build a `syntax_colors` array from a small set of named slots.
    /// Any index not explicitly set falls back to `fallback`.
    fn make_syntax(
        keyword: Color,
        function: Color,
        string: Color,
        comment: Color,
        type_color: Color,
        number: Color,
        operator: Color,
        constant: Color,
        variable: Color,
        escape: Color,
        fallback: Color,
    ) -> [Color; HIGHLIGHT_COUNT] {
        let mut c = [fallback; HIGHLIGHT_COUNT];
        c[HL_ATTRIBUTE] = function;
        c[HL_COMMENT] = comment;
        c[HL_CONSTANT] = constant;
        c[HL_CONSTANT_BUILTIN] = constant;
        c[HL_CONSTRUCTOR] = type_color;
        c[HL_DELIMITER] = fallback;
        c[HL_EMBEDDED] = fallback;
        c[HL_ESCAPE] = escape;
        c[HL_FUNCTION] = function;
        c[HL_FUNCTION_BUILTIN] = function;
        c[HL_KEYWORD] = keyword;
        c[HL_LABEL] = variable;
        c[HL_MODULE] = type_color;
        c[HL_NUMBER] = number;
        c[HL_OPERATOR] = operator;
        c[HL_PROPERTY] = variable;
        c[HL_PUNCTUATION] = fallback;
        c[HL_STRING] = string;
        c[HL_STRING_SPECIAL] = string;
        c[HL_TAG] = keyword;
        c[HL_TYPE] = type_color;
        c[HL_TYPE_BUILTIN] = type_color;
        c[HL_VARIABLE] = variable;
        c[HL_VARIABLE_BUILTIN] = constant;
        c[HL_VARIABLE_PARAMETER] = variable;
        c
    }

    /// Tokyo Night Dark — https://github.com/folke/tokyonight.nvim
    pub fn tokyonight() -> Self {
        Self {
            background: Color::Rgb(26, 27, 38), // #1a1b26 official TN background
            border: Color::Rgb(86, 95, 137),
            border_dim: Color::Rgb(54, 58, 79),
            text: Color::Rgb(192, 202, 245),
            text_dim: Color::Rgb(86, 95, 137),
            text_accent: Color::Rgb(122, 162, 247),
            pr_number: Color::Rgb(224, 175, 104),
            pr_author: Color::Rgb(158, 206, 106),
            pr_draft: Color::Rgb(224, 175, 104),
            selection_bg: Color::Rgb(40, 46, 74),
            selection_fg: Color::Rgb(192, 202, 245),
            diff_added_fg: Color::Rgb(158, 206, 106),
            diff_added_bg: Color::Rgb(29, 43, 29),
            diff_removed_fg: Color::Rgb(247, 118, 142),
            diff_removed_bg: Color::Rgb(43, 23, 28),
            diff_context: Color::Rgb(86, 95, 137),
            diff_hunk: Color::Rgb(187, 154, 247),
            tab_active: Color::Rgb(122, 162, 247),
            tab_inactive: Color::Rgb(86, 95, 137),
            key_fg: Color::Rgb(26, 27, 38),
            key_bg: Color::Rgb(224, 175, 104),
            key_desc: Color::Rgb(192, 202, 245),
            stats_added: Color::Rgb(158, 206, 106),
            stats_removed: Color::Rgb(247, 118, 142),
            section_header: Color::Rgb(187, 154, 247),
            separator: Color::Rgb(54, 58, 79),
            // keyword=purple, function=blue, string=green, comment=dim,
            // type=cyan, number=orange, operator=sky, constant=yellow, variable=text,
            // escape=peach
            syntax_colors: Self::make_syntax(
                Color::Rgb(187, 154, 247), // keyword   — purple
                Color::Rgb(122, 162, 247), // function  — blue
                Color::Rgb(158, 206, 106), // string    — green
                Color::Rgb(86, 95, 137),   // comment   — muted
                Color::Rgb(42, 195, 222),  // type      — cyan
                Color::Rgb(255, 158, 100), // number    — orange
                Color::Rgb(137, 221, 255), // operator  — sky
                Color::Rgb(224, 175, 104), // constant  — yellow
                Color::Rgb(192, 202, 245), // variable  — text
                Color::Rgb(255, 158, 100), // escape    — orange
                Color::Rgb(192, 202, 245), // fallback  — text
            ),
        }
    }

    /// Gruvbox Dark Hard — https://github.com/morhetz/gruvbox
    pub fn gruvbox() -> Self {
        Self {
            background: Color::Rgb(29, 32, 33),      // #1d2021 gruvbox bg hard
            border: Color::Rgb(80, 73, 69),          // bg3
            border_dim: Color::Rgb(60, 56, 54),      // bg2
            text: Color::Rgb(235, 219, 178),         // fg
            text_dim: Color::Rgb(146, 131, 116),     // gray
            text_accent: Color::Rgb(131, 165, 152),  // aqua
            pr_number: Color::Rgb(250, 189, 47),     // yellow
            pr_author: Color::Rgb(184, 187, 38),     // green
            pr_draft: Color::Rgb(254, 128, 25),      // orange
            selection_bg: Color::Rgb(60, 56, 54),    // bg2
            selection_fg: Color::Rgb(235, 219, 178), // fg
            diff_added_fg: Color::Rgb(184, 187, 38), // green
            diff_added_bg: Color::Rgb(36, 43, 27),   // dark green tint
            diff_removed_fg: Color::Rgb(251, 73, 52), // red
            diff_removed_bg: Color::Rgb(43, 24, 20), // dark red tint
            diff_context: Color::Rgb(146, 131, 116), // gray
            diff_hunk: Color::Rgb(211, 134, 155),    // purple
            tab_active: Color::Rgb(131, 165, 152),   // aqua
            tab_inactive: Color::Rgb(146, 131, 116), // gray
            key_fg: Color::Rgb(29, 32, 33),          // bg hard
            key_bg: Color::Rgb(250, 189, 47),        // yellow
            key_desc: Color::Rgb(235, 219, 178),     // fg
            stats_added: Color::Rgb(184, 187, 38),   // green
            stats_removed: Color::Rgb(251, 73, 52),  // red
            section_header: Color::Rgb(211, 134, 155), // purple
            separator: Color::Rgb(60, 56, 54),       // bg2
            // keyword=red, function=green, string=green, comment=gray,
            // type=yellow, number=purple, operator=fg, constant=orange, variable=fg,
            // escape=orange
            syntax_colors: Self::make_syntax(
                Color::Rgb(251, 73, 52),   // keyword   — red
                Color::Rgb(184, 187, 38),  // function  — green
                Color::Rgb(184, 187, 38),  // string    — green
                Color::Rgb(146, 131, 116), // comment   — gray
                Color::Rgb(250, 189, 47),  // type      — yellow
                Color::Rgb(211, 134, 155), // number    — purple
                Color::Rgb(235, 219, 178), // operator  — fg
                Color::Rgb(254, 128, 25),  // constant  — orange
                Color::Rgb(235, 219, 178), // variable  — fg
                Color::Rgb(254, 128, 25),  // escape    — orange
                Color::Rgb(235, 219, 178), // fallback  — fg
            ),
        }
    }

    /// Catppuccin Mocha — https://github.com/catppuccin/catppuccin
    pub fn catppuccin() -> Self {
        Self {
            background: Color::Rgb(30, 30, 46),  // #1e1e2e official mocha base
            border: Color::Rgb(88, 91, 112),     // overlay0
            border_dim: Color::Rgb(49, 50, 68),  // surface0
            text: Color::Rgb(205, 214, 244),     // text
            text_dim: Color::Rgb(108, 112, 134), // overlay1 (muted)
            text_accent: Color::Rgb(137, 180, 250), // blue
            pr_number: Color::Rgb(249, 226, 175), // yellow
            pr_author: Color::Rgb(166, 227, 161), // green
            pr_draft: Color::Rgb(250, 179, 135), // peach
            selection_bg: Color::Rgb(49, 50, 68), // surface0
            selection_fg: Color::Rgb(205, 214, 244), // text
            diff_added_fg: Color::Rgb(166, 227, 161), // green
            diff_added_bg: Color::Rgb(28, 42, 34), // dark green tint
            diff_removed_fg: Color::Rgb(243, 139, 168), // red
            diff_removed_bg: Color::Rgb(42, 26, 34), // dark red tint
            diff_context: Color::Rgb(88, 91, 112), // overlay0
            diff_hunk: Color::Rgb(203, 166, 247), // mauve
            tab_active: Color::Rgb(137, 180, 250), // blue
            tab_inactive: Color::Rgb(88, 91, 112), // overlay0
            key_fg: Color::Rgb(17, 17, 27),      // crust
            key_bg: Color::Rgb(249, 226, 175),   // yellow
            key_desc: Color::Rgb(205, 214, 244), // text
            stats_added: Color::Rgb(166, 227, 161), // green
            stats_removed: Color::Rgb(243, 139, 168), // red
            section_header: Color::Rgb(203, 166, 247), // mauve
            separator: Color::Rgb(49, 50, 68),   // surface0
            // keyword=mauve, function=blue, string=green, comment=overlay1,
            // type=yellow, number=peach, operator=sky, constant=peach, variable=text,
            // escape=peach
            syntax_colors: Self::make_syntax(
                Color::Rgb(203, 166, 247), // keyword   — mauve
                Color::Rgb(137, 180, 250), // function  — blue
                Color::Rgb(166, 227, 161), // string    — green
                Color::Rgb(108, 112, 134), // comment   — overlay1
                Color::Rgb(249, 226, 175), // type      — yellow
                Color::Rgb(250, 179, 135), // number    — peach
                Color::Rgb(137, 220, 235), // operator  — sky
                Color::Rgb(250, 179, 135), // constant  — peach
                Color::Rgb(205, 214, 244), // variable  — text
                Color::Rgb(250, 179, 135), // escape    — peach
                Color::Rgb(205, 214, 244), // fallback  — text
            ),
        }
    }

    /// Nord — https://www.nordtheme.com
    pub fn nord() -> Self {
        Self {
            background: Color::Rgb(46, 52, 64),        // #2e3440 nord0
            border: Color::Rgb(76, 86, 106),           // nord3
            border_dim: Color::Rgb(59, 66, 82),        // nord1
            text: Color::Rgb(236, 239, 244),           // nord6
            text_dim: Color::Rgb(76, 86, 106),         // nord3
            text_accent: Color::Rgb(136, 192, 208),    // nord8 (frost)
            pr_number: Color::Rgb(235, 203, 139),      // nord13 (yellow)
            pr_author: Color::Rgb(163, 190, 140),      // nord14 (green)
            pr_draft: Color::Rgb(208, 135, 112),       // nord12 (orange)
            selection_bg: Color::Rgb(67, 76, 94),      // nord2
            selection_fg: Color::Rgb(236, 239, 244),   // nord6
            diff_added_fg: Color::Rgb(163, 190, 140),  // nord14 green
            diff_added_bg: Color::Rgb(30, 44, 32),     // dark green tint
            diff_removed_fg: Color::Rgb(191, 97, 106), // nord11 red
            diff_removed_bg: Color::Rgb(42, 24, 26),   // dark red tint
            diff_context: Color::Rgb(76, 86, 106),     // nord3
            diff_hunk: Color::Rgb(180, 142, 173),      // nord15 purple
            tab_active: Color::Rgb(136, 192, 208),     // nord8
            tab_inactive: Color::Rgb(76, 86, 106),     // nord3
            key_fg: Color::Rgb(46, 52, 64),            // nord0
            key_bg: Color::Rgb(235, 203, 139),         // nord13
            key_desc: Color::Rgb(236, 239, 244),       // nord6
            stats_added: Color::Rgb(163, 190, 140),    // nord14
            stats_removed: Color::Rgb(191, 97, 106),   // nord11
            section_header: Color::Rgb(180, 142, 173), // nord15
            separator: Color::Rgb(59, 66, 82),         // nord1
            // keyword=purple, function=frost-blue, string=green, comment=nord3,
            // type=yellow, number=frost-light, operator=frost, constant=yellow, variable=nord6,
            // escape=orange
            syntax_colors: Self::make_syntax(
                Color::Rgb(180, 142, 173), // keyword   — nord15 purple
                Color::Rgb(136, 192, 208), // function  — nord8 frost
                Color::Rgb(163, 190, 140), // string    — nord14 green
                Color::Rgb(76, 86, 106),   // comment   — nord3
                Color::Rgb(235, 203, 139), // type      — nord13 yellow
                Color::Rgb(180, 142, 173), // number    — nord15 purple
                Color::Rgb(129, 161, 193), // operator  — nord9 frost
                Color::Rgb(235, 203, 139), // constant  — nord13 yellow
                Color::Rgb(216, 222, 233), // variable  — nord5
                Color::Rgb(208, 135, 112), // escape    — nord12 orange
                Color::Rgb(216, 222, 233), // fallback  — nord5
            ),
        }
    }

    /// Dracula — https://draculatheme.com
    pub fn dracula() -> Self {
        Self {
            background: Color::Rgb(40, 42, 54), // #282a36 official dracula bg
            border: Color::Rgb(98, 114, 164),   // comment
            border_dim: Color::Rgb(68, 71, 90), // current line (darker)
            text: Color::Rgb(248, 248, 242),    // foreground
            text_dim: Color::Rgb(98, 114, 164), // comment
            text_accent: Color::Rgb(139, 233, 253), // cyan
            pr_number: Color::Rgb(255, 184, 108), // orange
            pr_author: Color::Rgb(80, 250, 123), // green
            pr_draft: Color::Rgb(255, 184, 108), // orange
            selection_bg: Color::Rgb(68, 71, 90), // current line
            selection_fg: Color::Rgb(248, 248, 242), // foreground
            diff_added_fg: Color::Rgb(80, 250, 123), // green
            diff_added_bg: Color::Rgb(22, 46, 30), // dark green tint
            diff_removed_fg: Color::Rgb(255, 85, 85), // red
            diff_removed_bg: Color::Rgb(46, 20, 20), // dark red tint
            diff_context: Color::Rgb(98, 114, 164), // comment
            diff_hunk: Color::Rgb(189, 147, 249), // purple
            tab_active: Color::Rgb(139, 233, 253), // cyan
            tab_inactive: Color::Rgb(98, 114, 164), // comment
            key_fg: Color::Rgb(40, 42, 54),     // background
            key_bg: Color::Rgb(255, 184, 108),  // orange
            key_desc: Color::Rgb(248, 248, 242), // foreground
            stats_added: Color::Rgb(80, 250, 123), // green
            stats_removed: Color::Rgb(255, 85, 85), // red
            section_header: Color::Rgb(189, 147, 249), // purple
            separator: Color::Rgb(68, 71, 90),  // current line
            // keyword=pink, function=green, string=yellow, comment=comment,
            // type=cyan, number=purple, operator=pink, constant=orange, variable=fg,
            // escape=orange
            syntax_colors: Self::make_syntax(
                Color::Rgb(255, 121, 198), // keyword   — pink
                Color::Rgb(80, 250, 123),  // function  — green
                Color::Rgb(241, 250, 140), // string    — yellow
                Color::Rgb(98, 114, 164),  // comment   — comment
                Color::Rgb(139, 233, 253), // type      — cyan
                Color::Rgb(189, 147, 249), // number    — purple
                Color::Rgb(255, 121, 198), // operator  — pink
                Color::Rgb(255, 184, 108), // constant  — orange
                Color::Rgb(248, 248, 242), // variable  — foreground
                Color::Rgb(255, 184, 108), // escape    — orange
                Color::Rgb(248, 248, 242), // fallback  — foreground
            ),
        }
    }

    /// Rosé Pine — https://rosepinetheme.com (Main/dark variant)
    pub fn rosepine() -> Self {
        Self {
            background: Color::Rgb(25, 23, 36), // #191724 official rosepine base
            border: Color::Rgb(64, 61, 82),     // overlay
            border_dim: Color::Rgb(38, 35, 58), // surface
            text: Color::Rgb(224, 222, 244),    // text
            text_dim: Color::Rgb(110, 106, 134), // muted
            text_accent: Color::Rgb(156, 207, 216), // foam
            pr_number: Color::Rgb(246, 193, 119), // gold
            pr_author: Color::Rgb(49, 116, 143), // pine (teal)
            pr_draft: Color::Rgb(246, 193, 119), // gold
            selection_bg: Color::Rgb(38, 35, 58), // surface
            selection_fg: Color::Rgb(224, 222, 244), // text
            diff_added_fg: Color::Rgb(156, 207, 216), // foam
            diff_added_bg: Color::Rgb(22, 37, 42), // dark foam tint
            diff_removed_fg: Color::Rgb(235, 111, 146), // love (pink-red)
            diff_removed_bg: Color::Rgb(42, 22, 32), // dark love tint
            diff_context: Color::Rgb(110, 106, 134), // muted
            diff_hunk: Color::Rgb(196, 167, 231), // iris (purple)
            tab_active: Color::Rgb(156, 207, 216), // foam
            tab_inactive: Color::Rgb(110, 106, 134), // muted
            key_fg: Color::Rgb(25, 23, 36),     // base
            key_bg: Color::Rgb(246, 193, 119),  // gold
            key_desc: Color::Rgb(224, 222, 244), // text
            stats_added: Color::Rgb(156, 207, 216), // foam
            stats_removed: Color::Rgb(235, 111, 146), // love
            section_header: Color::Rgb(196, 167, 231), // iris
            separator: Color::Rgb(38, 35, 58),  // surface
            // keyword=iris, function=foam, string=pine-light, comment=muted,
            // type=gold, number=iris, operator=foam, constant=gold, variable=text,
            // escape=gold
            syntax_colors: Self::make_syntax(
                Color::Rgb(196, 167, 231), // keyword   — iris
                Color::Rgb(156, 207, 216), // function  — foam
                Color::Rgb(156, 207, 216), // string    — foam (pine tint)
                Color::Rgb(110, 106, 134), // comment   — muted
                Color::Rgb(246, 193, 119), // type      — gold
                Color::Rgb(235, 188, 186), // number    — rose
                Color::Rgb(156, 207, 216), // operator  — foam
                Color::Rgb(246, 193, 119), // constant  — gold
                Color::Rgb(224, 222, 244), // variable  — text
                Color::Rgb(246, 193, 119), // escape    — gold
                Color::Rgb(224, 222, 244), // fallback  — text
            ),
        }
    }

    // ── Light themes ─────────────────────────────────────────────────────────

    /// GitHub Light — mirrors the GitHub web UI palette
    pub fn github_light() -> Self {
        Self {
            background: Color::Rgb(255, 255, 255),    // #ffffff pure white
            border: Color::Rgb(208, 215, 222),        // border.default
            border_dim: Color::Rgb(234, 238, 241),    // border.muted
            text: Color::Rgb(31, 35, 40),             // fg.default
            text_dim: Color::Rgb(101, 109, 118),      // fg.muted
            text_accent: Color::Rgb(9, 105, 218),     // accent.fg
            pr_number: Color::Rgb(130, 80, 7),        // attention.fg (amber)
            pr_author: Color::Rgb(31, 111, 31),       // success.fg
            pr_draft: Color::Rgb(130, 80, 7),         // attention
            selection_bg: Color::Rgb(218, 233, 252),  // accent.subtle
            selection_fg: Color::Rgb(31, 35, 40),     // fg.default
            diff_added_fg: Color::Rgb(31, 111, 31),   // success.fg
            diff_added_bg: Color::Rgb(218, 251, 225), // success.subtle
            diff_removed_fg: Color::Rgb(209, 36, 47), // danger.fg
            diff_removed_bg: Color::Rgb(255, 235, 233), // danger.subtle
            diff_context: Color::Rgb(101, 109, 118),  // fg.muted
            diff_hunk: Color::Rgb(84, 54, 218),       // done.fg (purple)
            tab_active: Color::Rgb(9, 105, 218),      // accent.fg
            tab_inactive: Color::Rgb(101, 109, 118),  // fg.muted
            key_fg: Color::Rgb(255, 255, 255),        // white on badge
            key_bg: Color::Rgb(9, 105, 218),          // accent.fg
            key_desc: Color::Rgb(31, 35, 40),         // fg.default
            stats_added: Color::Rgb(31, 111, 31),     // success.fg
            stats_removed: Color::Rgb(209, 36, 47),   // danger.fg
            section_header: Color::Rgb(84, 54, 218),  // done.fg
            separator: Color::Rgb(208, 215, 222),     // border.default
            // keyword=purple, function=blue, string=green, comment=muted,
            // type=teal, number=orange, operator=red, constant=amber, variable=fg,
            // escape=amber
            syntax_colors: Self::make_syntax(
                Color::Rgb(207, 34, 46),   // keyword   — danger red
                Color::Rgb(130, 80, 7),    // function  — amber
                Color::Rgb(10, 78, 6),     // string    — dark green
                Color::Rgb(101, 109, 118), // comment   — muted
                Color::Rgb(5, 80, 174),    // type      — blue
                Color::Rgb(130, 80, 7),    // number    — amber
                Color::Rgb(207, 34, 46),   // operator  — red
                Color::Rgb(5, 80, 174),    // constant  — blue
                Color::Rgb(31, 35, 40),    // variable  — fg
                Color::Rgb(130, 80, 7),    // escape    — amber
                Color::Rgb(31, 35, 40),    // fallback  — fg
            ),
        }
    }

    /// Catppuccin Latte — https://github.com/catppuccin/catppuccin (light variant)
    pub fn catppuccin_latte() -> Self {
        Self {
            background: Color::Rgb(239, 241, 245),    // #eff1f5 latte base
            border: Color::Rgb(172, 176, 190),        // overlay0
            border_dim: Color::Rgb(204, 208, 218),    // surface2
            text: Color::Rgb(76, 79, 105),            // text
            text_dim: Color::Rgb(156, 160, 176),      // overlay1
            text_accent: Color::Rgb(30, 102, 245),    // blue
            pr_number: Color::Rgb(223, 142, 29),      // yellow
            pr_author: Color::Rgb(64, 160, 43),       // green
            pr_draft: Color::Rgb(254, 100, 11),       // peach
            selection_bg: Color::Rgb(204, 208, 218),  // surface2
            selection_fg: Color::Rgb(76, 79, 105),    // text
            diff_added_fg: Color::Rgb(64, 160, 43),   // green
            diff_added_bg: Color::Rgb(213, 237, 209), // green subtle
            diff_removed_fg: Color::Rgb(210, 15, 57), // red
            diff_removed_bg: Color::Rgb(249, 210, 218), // red subtle
            diff_context: Color::Rgb(156, 160, 176),  // overlay1
            diff_hunk: Color::Rgb(136, 57, 239),      // mauve
            tab_active: Color::Rgb(30, 102, 245),     // blue
            tab_inactive: Color::Rgb(172, 176, 190),  // overlay0
            key_fg: Color::Rgb(239, 241, 245),        // base (light bg)
            key_bg: Color::Rgb(30, 102, 245),         // blue
            key_desc: Color::Rgb(76, 79, 105),        // text
            stats_added: Color::Rgb(64, 160, 43),     // green
            stats_removed: Color::Rgb(210, 15, 57),   // red
            section_header: Color::Rgb(136, 57, 239), // mauve
            separator: Color::Rgb(204, 208, 218),     // surface2
            // keyword=mauve, function=blue, string=green, comment=overlay1,
            // type=yellow, number=peach, operator=sky, constant=peach, variable=text,
            // escape=peach
            syntax_colors: Self::make_syntax(
                Color::Rgb(136, 57, 239),  // keyword   — mauve
                Color::Rgb(30, 102, 245),  // function  — blue
                Color::Rgb(64, 160, 43),   // string    — green
                Color::Rgb(156, 160, 176), // comment   — overlay1
                Color::Rgb(223, 142, 29),  // type      — yellow
                Color::Rgb(254, 100, 11),  // number    — peach
                Color::Rgb(4, 165, 229),   // operator  — sky
                Color::Rgb(254, 100, 11),  // constant  — peach
                Color::Rgb(76, 79, 105),   // variable  — text
                Color::Rgb(254, 100, 11),  // escape    — peach
                Color::Rgb(76, 79, 105),   // fallback  — text
            ),
        }
    }

    /// Rosé Pine Dawn — https://rosepinetheme.com (light variant)
    pub fn rosepine_dawn() -> Self {
        Self {
            background: Color::Rgb(250, 244, 237),      // #faf4ed dawn base
            border: Color::Rgb(215, 210, 195),          // overlay
            border_dim: Color::Rgb(242, 233, 222),      // surface
            text: Color::Rgb(87, 82, 121),              // text
            text_dim: Color::Rgb(152, 147, 165),        // muted
            text_accent: Color::Rgb(40, 105, 131),      // pine (teal)
            pr_number: Color::Rgb(234, 157, 52),        // gold
            pr_author: Color::Rgb(40, 105, 131),        // pine
            pr_draft: Color::Rgb(234, 157, 52),         // gold
            selection_bg: Color::Rgb(242, 233, 222),    // surface
            selection_fg: Color::Rgb(87, 82, 121),      // text
            diff_added_fg: Color::Rgb(40, 105, 131),    // pine
            diff_added_bg: Color::Rgb(214, 235, 232),   // pine subtle
            diff_removed_fg: Color::Rgb(180, 99, 122),  // love
            diff_removed_bg: Color::Rgb(248, 220, 228), // love subtle
            diff_context: Color::Rgb(152, 147, 165),    // muted
            diff_hunk: Color::Rgb(144, 122, 169),       // iris
            tab_active: Color::Rgb(40, 105, 131),       // pine
            tab_inactive: Color::Rgb(152, 147, 165),    // muted
            key_fg: Color::Rgb(250, 244, 237),          // base
            key_bg: Color::Rgb(40, 105, 131),           // pine
            key_desc: Color::Rgb(87, 82, 121),          // text
            stats_added: Color::Rgb(40, 105, 131),      // pine
            stats_removed: Color::Rgb(180, 99, 122),    // love
            section_header: Color::Rgb(144, 122, 169),  // iris
            separator: Color::Rgb(215, 210, 195),       // overlay
            // keyword=iris, function=pine, string=foam, comment=muted,
            // type=gold, number=iris, operator=pine, constant=gold, variable=text,
            // escape=gold
            syntax_colors: Self::make_syntax(
                Color::Rgb(144, 122, 169), // keyword   — iris
                Color::Rgb(40, 105, 131),  // function  — pine
                Color::Rgb(86, 148, 159),  // string    — foam
                Color::Rgb(152, 147, 165), // comment   — muted
                Color::Rgb(234, 157, 52),  // type      — gold
                Color::Rgb(144, 122, 169), // number    — iris
                Color::Rgb(40, 105, 131),  // operator  — pine
                Color::Rgb(234, 157, 52),  // constant  — gold
                Color::Rgb(87, 82, 121),   // variable  — text
                Color::Rgb(234, 157, 52),  // escape    — gold
                Color::Rgb(87, 82, 121),   // fallback  — text
            ),
        }
    }

    /// Gruvbox Light Hard — https://github.com/morhetz/gruvbox (light variant)
    pub fn gruvbox_light() -> Self {
        Self {
            background: Color::Rgb(249, 245, 215), // #f9f5d7 gruvbox light bg hard
            border: Color::Rgb(189, 174, 147),     // bg3 light
            border_dim: Color::Rgb(213, 196, 161), // bg2 light
            text: Color::Rgb(60, 56, 54),          // fg1
            text_dim: Color::Rgb(124, 111, 100),   // fg4
            text_accent: Color::Rgb(69, 133, 136), // aqua dark
            pr_number: Color::Rgb(181, 118, 20),   // yellow dark
            pr_author: Color::Rgb(121, 116, 14),   // green dark
            pr_draft: Color::Rgb(175, 58, 3),      // orange dark
            selection_bg: Color::Rgb(213, 196, 161), // bg2 light
            selection_fg: Color::Rgb(60, 56, 54),  // fg1
            diff_added_fg: Color::Rgb(121, 116, 14), // green dark
            diff_added_bg: Color::Rgb(215, 232, 196), // green subtle
            diff_removed_fg: Color::Rgb(157, 0, 6), // red dark
            diff_removed_bg: Color::Rgb(248, 210, 198), // red subtle
            diff_context: Color::Rgb(124, 111, 100), // fg4
            diff_hunk: Color::Rgb(143, 63, 113),   // purple dark
            tab_active: Color::Rgb(69, 133, 136),  // aqua dark
            tab_inactive: Color::Rgb(124, 111, 100), // fg4
            key_fg: Color::Rgb(249, 245, 215),     // bg hard (for contrast)
            key_bg: Color::Rgb(181, 118, 20),      // yellow dark
            key_desc: Color::Rgb(60, 56, 54),      // fg1
            stats_added: Color::Rgb(121, 116, 14), // green dark
            stats_removed: Color::Rgb(157, 0, 6),  // red dark
            section_header: Color::Rgb(143, 63, 113), // purple dark
            separator: Color::Rgb(213, 196, 161),  // bg2 light
            // keyword=red, function=green, string=green, comment=gray,
            // type=yellow, number=purple, operator=fg, constant=orange, variable=fg,
            // escape=orange
            syntax_colors: Self::make_syntax(
                Color::Rgb(157, 0, 6),     // keyword   — red dark
                Color::Rgb(121, 116, 14),  // function  — green dark
                Color::Rgb(121, 116, 14),  // string    — green dark
                Color::Rgb(124, 111, 100), // comment   — fg4
                Color::Rgb(181, 118, 20),  // type      — yellow dark
                Color::Rgb(143, 63, 113),  // number    — purple dark
                Color::Rgb(60, 56, 54),    // operator  — fg
                Color::Rgb(175, 58, 3),    // constant  — orange dark
                Color::Rgb(60, 56, 54),    // variable  — fg
                Color::Rgb(175, 58, 3),    // escape    — orange dark
                Color::Rgb(60, 56, 54),    // fallback  — fg
            ),
        }
    }

    /// Solarized Light — https://ethanschoonover.com/solarized/
    pub fn solarized_light() -> Self {
        Self {
            background: Color::Rgb(253, 246, 227), // #fdf6e3 solarized base3
            border: Color::Rgb(147, 161, 161),     // base1 (brighter)
            border_dim: Color::Rgb(238, 232, 213), // base2
            text: Color::Rgb(101, 123, 131),       // base00
            text_dim: Color::Rgb(147, 161, 161),   // base1
            text_accent: Color::Rgb(38, 139, 210), // blue
            pr_number: Color::Rgb(181, 137, 0),    // yellow
            pr_author: Color::Rgb(133, 153, 0),    // green
            pr_draft: Color::Rgb(203, 75, 22),     // orange
            selection_bg: Color::Rgb(238, 232, 213), // base2
            selection_fg: Color::Rgb(101, 123, 131), // base00
            diff_added_fg: Color::Rgb(133, 153, 0), // green
            diff_added_bg: Color::Rgb(220, 237, 193), // green subtle
            diff_removed_fg: Color::Rgb(220, 50, 47), // red
            diff_removed_bg: Color::Rgb(250, 213, 212), // red subtle
            diff_context: Color::Rgb(147, 161, 161), // base1
            diff_hunk: Color::Rgb(108, 113, 196),  // violet
            tab_active: Color::Rgb(38, 139, 210),  // blue
            tab_inactive: Color::Rgb(147, 161, 161), // base1
            key_fg: Color::Rgb(253, 246, 227),     // base3 (light bg text)
            key_bg: Color::Rgb(38, 139, 210),      // blue
            key_desc: Color::Rgb(101, 123, 131),   // base00
            stats_added: Color::Rgb(133, 153, 0),  // green
            stats_removed: Color::Rgb(220, 50, 47), // red
            section_header: Color::Rgb(108, 113, 196), // violet
            separator: Color::Rgb(238, 232, 213),  // base2
            // keyword=green, function=blue, string=cyan, comment=base1,
            // type=yellow, number=magenta, operator=orange, constant=orange, variable=base00,
            // escape=orange
            syntax_colors: Self::make_syntax(
                Color::Rgb(133, 153, 0),   // keyword   — green
                Color::Rgb(38, 139, 210),  // function  — blue
                Color::Rgb(42, 161, 152),  // string    — cyan
                Color::Rgb(147, 161, 161), // comment   — base1
                Color::Rgb(181, 137, 0),   // type      — yellow
                Color::Rgb(211, 54, 130),  // number    — magenta
                Color::Rgb(203, 75, 22),   // operator  — orange
                Color::Rgb(203, 75, 22),   // constant  — orange
                Color::Rgb(101, 123, 131), // variable  — base00
                Color::Rgb(203, 75, 22),   // escape    — orange
                Color::Rgb(101, 123, 131), // fallback  — base00
            ),
        }
    }

    // ── Convenience style builders ────────────────────────────────────────────

    /// Style for the main app/panel background.
    pub fn background_style(&self) -> Style {
        Style::default().bg(self.background)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn border_dim_style(&self) -> Style {
        Style::default().fg(self.border_dim)
    }

    pub fn text_style(&self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn text_dim_style(&self) -> Style {
        Style::default().fg(self.text_dim)
    }

    pub fn text_accent_style(&self) -> Style {
        Style::default().fg(self.text_accent)
    }

    pub fn selection_style(&self) -> Style {
        Style::default().bg(self.selection_bg).fg(self.selection_fg)
    }

    pub fn diff_added_style(&self) -> Style {
        Style::default()
            .fg(self.diff_added_fg)
            .bg(self.diff_added_bg)
    }

    pub fn diff_removed_style(&self) -> Style {
        Style::default()
            .fg(self.diff_removed_fg)
            .bg(self.diff_removed_bg)
    }

    pub fn diff_context_style(&self) -> Style {
        Style::default().fg(self.diff_context)
    }

    pub fn diff_hunk_style(&self) -> Style {
        Style::default()
            .fg(self.diff_hunk)
            .add_modifier(Modifier::BOLD)
    }

    pub fn tab_active_style(&self) -> Style {
        Style::default()
            .fg(self.tab_active)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED)
    }

    pub fn tab_inactive_style(&self) -> Style {
        Style::default().fg(self.tab_inactive)
    }

    /// Style for a key badge background span.
    pub fn key_badge_style(&self) -> Style {
        Style::default()
            .fg(self.key_fg)
            .bg(self.key_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn key_desc_style(&self) -> Style {
        Style::default().fg(self.key_desc)
    }

    pub fn section_header_style(&self) -> Style {
        Style::default()
            .fg(self.section_header)
            .add_modifier(Modifier::BOLD)
    }

    pub fn separator_style(&self) -> Style {
        Style::default().fg(self.separator)
    }

    /// Return the foreground `Color` for a syntax highlight index.
    /// Out-of-range indices return the normal text colour.
    pub fn syntax_color(&self, hl_index: usize) -> Color {
        self.syntax_colors
            .get(hl_index)
            .copied()
            .unwrap_or(self.text)
    }
}
