use ratatui::style::{Color, Modifier, Style};

/// All named color/style slots used across the UI.
///
/// Adding a new theme: implement a new constructor (`Theme::my_theme() -> Theme`)
/// and add an entry to `ALL_THEMES` and `from_name`. No render function needs to change.
#[derive(Debug, Clone)]
pub struct Theme {
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

    /// Tokyo Night Dark — https://github.com/folke/tokyonight.nvim
    pub fn tokyonight() -> Self {
        Self {
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
        }
    }

    /// Gruvbox Dark Hard — https://github.com/morhetz/gruvbox
    pub fn gruvbox() -> Self {
        Self {
            border: Color::Rgb(80, 73, 69),            // bg3
            border_dim: Color::Rgb(60, 56, 54),        // bg2
            text: Color::Rgb(235, 219, 178),           // fg
            text_dim: Color::Rgb(146, 131, 116),       // gray
            text_accent: Color::Rgb(131, 165, 152),    // aqua
            pr_number: Color::Rgb(250, 189, 47),       // yellow
            pr_author: Color::Rgb(184, 187, 38),       // green
            pr_draft: Color::Rgb(254, 128, 25),        // orange
            selection_bg: Color::Rgb(60, 56, 54),      // bg2
            selection_fg: Color::Rgb(235, 219, 178),   // fg
            diff_added_fg: Color::Rgb(184, 187, 38),   // green
            diff_added_bg: Color::Rgb(36, 43, 27),     // dark green tint
            diff_removed_fg: Color::Rgb(251, 73, 52),  // red
            diff_removed_bg: Color::Rgb(43, 24, 20),   // dark red tint
            diff_context: Color::Rgb(146, 131, 116),   // gray
            diff_hunk: Color::Rgb(211, 134, 155),      // purple
            tab_active: Color::Rgb(131, 165, 152),     // aqua
            tab_inactive: Color::Rgb(146, 131, 116),   // gray
            key_fg: Color::Rgb(29, 32, 33),            // bg hard
            key_bg: Color::Rgb(250, 189, 47),          // yellow
            key_desc: Color::Rgb(235, 219, 178),       // fg
            stats_added: Color::Rgb(184, 187, 38),     // green
            stats_removed: Color::Rgb(251, 73, 52),    // red
            section_header: Color::Rgb(211, 134, 155), // purple
            separator: Color::Rgb(60, 56, 54),         // bg2
        }
    }

    /// Catppuccin Mocha — https://github.com/catppuccin/catppuccin
    pub fn catppuccin() -> Self {
        Self {
            border: Color::Rgb(88, 91, 112),            // overlay0
            border_dim: Color::Rgb(49, 50, 68),         // surface0
            text: Color::Rgb(205, 214, 244),            // text
            text_dim: Color::Rgb(108, 112, 134),        // overlay1 (muted)
            text_accent: Color::Rgb(137, 180, 250),     // blue
            pr_number: Color::Rgb(249, 226, 175),       // yellow
            pr_author: Color::Rgb(166, 227, 161),       // green
            pr_draft: Color::Rgb(250, 179, 135),        // peach
            selection_bg: Color::Rgb(49, 50, 68),       // surface0
            selection_fg: Color::Rgb(205, 214, 244),    // text
            diff_added_fg: Color::Rgb(166, 227, 161),   // green
            diff_added_bg: Color::Rgb(28, 42, 34),      // dark green tint
            diff_removed_fg: Color::Rgb(243, 139, 168), // red
            diff_removed_bg: Color::Rgb(42, 26, 34),    // dark red tint
            diff_context: Color::Rgb(88, 91, 112),      // overlay0
            diff_hunk: Color::Rgb(203, 166, 247),       // mauve
            tab_active: Color::Rgb(137, 180, 250),      // blue
            tab_inactive: Color::Rgb(88, 91, 112),      // overlay0
            key_fg: Color::Rgb(17, 17, 27),             // crust
            key_bg: Color::Rgb(249, 226, 175),          // yellow
            key_desc: Color::Rgb(205, 214, 244),        // text
            stats_added: Color::Rgb(166, 227, 161),     // green
            stats_removed: Color::Rgb(243, 139, 168),   // red
            section_header: Color::Rgb(203, 166, 247),  // mauve
            separator: Color::Rgb(49, 50, 68),          // surface0
        }
    }

    /// Nord — https://www.nordtheme.com
    pub fn nord() -> Self {
        Self {
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
        }
    }

    /// Dracula — https://draculatheme.com
    pub fn dracula() -> Self {
        Self {
            border: Color::Rgb(98, 114, 164),          // comment
            border_dim: Color::Rgb(68, 71, 90),        // current line (darker)
            text: Color::Rgb(248, 248, 242),           // foreground
            text_dim: Color::Rgb(98, 114, 164),        // comment
            text_accent: Color::Rgb(139, 233, 253),    // cyan
            pr_number: Color::Rgb(255, 184, 108),      // orange
            pr_author: Color::Rgb(80, 250, 123),       // green
            pr_draft: Color::Rgb(255, 184, 108),       // orange
            selection_bg: Color::Rgb(68, 71, 90),      // current line
            selection_fg: Color::Rgb(248, 248, 242),   // foreground
            diff_added_fg: Color::Rgb(80, 250, 123),   // green
            diff_added_bg: Color::Rgb(22, 46, 30),     // dark green tint
            diff_removed_fg: Color::Rgb(255, 85, 85),  // red
            diff_removed_bg: Color::Rgb(46, 20, 20),   // dark red tint
            diff_context: Color::Rgb(98, 114, 164),    // comment
            diff_hunk: Color::Rgb(189, 147, 249),      // purple
            tab_active: Color::Rgb(139, 233, 253),     // cyan
            tab_inactive: Color::Rgb(98, 114, 164),    // comment
            key_fg: Color::Rgb(40, 42, 54),            // background
            key_bg: Color::Rgb(255, 184, 108),         // orange
            key_desc: Color::Rgb(248, 248, 242),       // foreground
            stats_added: Color::Rgb(80, 250, 123),     // green
            stats_removed: Color::Rgb(255, 85, 85),    // red
            section_header: Color::Rgb(189, 147, 249), // purple
            separator: Color::Rgb(68, 71, 90),         // current line
        }
    }

    /// Rosé Pine — https://rosepinetheme.com (Main/dark variant)
    pub fn rosepine() -> Self {
        Self {
            border: Color::Rgb(64, 61, 82),             // overlay
            border_dim: Color::Rgb(38, 35, 58),         // surface
            text: Color::Rgb(224, 222, 244),            // text
            text_dim: Color::Rgb(110, 106, 134),        // muted
            text_accent: Color::Rgb(156, 207, 216),     // foam
            pr_number: Color::Rgb(246, 193, 119),       // gold
            pr_author: Color::Rgb(49, 116, 143),        // pine (teal)
            pr_draft: Color::Rgb(246, 193, 119),        // gold
            selection_bg: Color::Rgb(38, 35, 58),       // surface
            selection_fg: Color::Rgb(224, 222, 244),    // text
            diff_added_fg: Color::Rgb(156, 207, 216),   // foam
            diff_added_bg: Color::Rgb(22, 37, 42),      // dark foam tint
            diff_removed_fg: Color::Rgb(235, 111, 146), // love (pink-red)
            diff_removed_bg: Color::Rgb(42, 22, 32),    // dark love tint
            diff_context: Color::Rgb(110, 106, 134),    // muted
            diff_hunk: Color::Rgb(196, 167, 231),       // iris (purple)
            tab_active: Color::Rgb(156, 207, 216),      // foam
            tab_inactive: Color::Rgb(110, 106, 134),    // muted
            key_fg: Color::Rgb(25, 23, 36),             // base
            key_bg: Color::Rgb(246, 193, 119),          // gold
            key_desc: Color::Rgb(224, 222, 244),        // text
            stats_added: Color::Rgb(156, 207, 216),     // foam
            stats_removed: Color::Rgb(235, 111, 146),   // love
            section_header: Color::Rgb(196, 167, 231),  // iris
            separator: Color::Rgb(38, 35, 58),          // surface
        }
    }

    // ── Convenience style builders ────────────────────────────────────────────

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
}
