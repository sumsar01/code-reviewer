# UI Patterns — Layout & Theming Reference

> Extracted from `code-reviewer`. Use this as a blueprint when building a new ratatui TUI app.

---

## Theme System

### The `Theme` struct

Define a single `Theme` struct with **named semantic slots** for every UI element. Render functions never hardcode colors — they always ask the theme. Pass `&Theme` down to every render function as a parameter.

```rust
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct Theme {
    // ── Chrome ────────────────────────────────────────────────────────────────
    pub border: Color,       // Primary border color (blocks, panels)
    pub border_dim: Color,   // Dimmer border (inner / secondary panels)

    // ── Text ─────────────────────────────────────────────────────────────────
    pub text: Color,         // Normal body text
    pub text_dim: Color,     // Secondary / muted text (timestamps, labels)
    pub text_accent: Color,  // Accent text (paths, branch names)

    // ── List / Selection ─────────────────────────────────────────────────────
    pub selection_bg: Color,
    pub selection_fg: Color,

    // ── Diff ──────────────────────────────────────────────────────────────────
    pub diff_added_fg: Color,
    pub diff_added_bg: Color,   // Full-row background tint
    pub diff_removed_fg: Color,
    pub diff_removed_bg: Color,
    pub diff_context: Color,    // Unchanged lines
    pub diff_hunk: Color,       // @@ hunk header lines

    // ── Tabs ─────────────────────────────────────────────────────────────────
    pub tab_active: Color,
    pub tab_inactive: Color,

    // ── Key hint badges ───────────────────────────────────────────────────────
    pub key_fg: Color,    // Dark fg so the badge bg is readable
    pub key_bg: Color,    // Badge background (usually a warm accent)
    pub key_desc: Color,  // Description text next to badge

    // ── Stats ─────────────────────────────────────────────────────────────────
    pub stats_added: Color,
    pub stats_removed: Color,

    // ── Misc ─────────────────────────────────────────────────────────────────
    pub section_header: Color,  // Section headings in overlays
    pub separator: Color,       // ── separator lines
}
```

### Convenience style builders

Add methods on `Theme` so render code stays clean:

```rust
impl Theme {
    pub fn border_style(&self) -> Style { Style::default().fg(self.border) }
    pub fn border_dim_style(&self) -> Style { Style::default().fg(self.border_dim) }
    pub fn text_style(&self) -> Style { Style::default().fg(self.text) }
    pub fn text_dim_style(&self) -> Style { Style::default().fg(self.text_dim) }
    pub fn text_accent_style(&self) -> Style { Style::default().fg(self.text_accent) }

    pub fn selection_style(&self) -> Style {
        Style::default().bg(self.selection_bg).fg(self.selection_fg)
    }

    pub fn diff_added_style(&self) -> Style {
        Style::default().fg(self.diff_added_fg).bg(self.diff_added_bg)
    }
    pub fn diff_removed_style(&self) -> Style {
        Style::default().fg(self.diff_removed_fg).bg(self.diff_removed_bg)
    }
    pub fn diff_context_style(&self) -> Style { Style::default().fg(self.diff_context) }
    pub fn diff_hunk_style(&self) -> Style {
        Style::default().fg(self.diff_hunk).add_modifier(Modifier::BOLD)
    }

    pub fn tab_active_style(&self) -> Style {
        Style::default()
            .fg(self.tab_active)
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::UNDERLINED)
    }
    pub fn tab_inactive_style(&self) -> Style { Style::default().fg(self.tab_inactive) }

    pub fn key_badge_style(&self) -> Style {
        Style::default().fg(self.key_fg).bg(self.key_bg).add_modifier(Modifier::BOLD)
    }
    pub fn key_desc_style(&self) -> Style { Style::default().fg(self.key_desc) }

    pub fn section_header_style(&self) -> Style {
        Style::default().fg(self.section_header).add_modifier(Modifier::BOLD)
    }
    pub fn separator_style(&self) -> Style { Style::default().fg(self.separator) }
}
```

### Theme registry

```rust
/// (id, display name) — order is the order shown in the picker
pub const ALL_THEMES: &[(&str, &str)] = &[
    ("tokyonight", "Tokyo Night"),
    ("gruvbox",    "Gruvbox Dark"),
    ("catppuccin", "Catppuccin Mocha"),
    ("nord",       "Nord"),
    ("dracula",    "Dracula"),
    ("rosepine",   "Rosé Pine"),
];

impl Theme {
    /// Resolve by id, falling back to the default on unknown names.
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "tokyonight" => Self::tokyonight(),
            "gruvbox"    => Self::gruvbox(),
            // ...
            _            => Self::tokyonight(),
        }
    }

    /// Index in ALL_THEMES (for the picker cursor).
    pub fn index_of(name: &str) -> usize {
        let lower = name.to_lowercase();
        ALL_THEMES.iter().position(|(id, _)| *id == lower.as_str()).unwrap_or(0)
    }
}
```

### Theme constructor pattern

Use `Color::Rgb(r, g, b)` throughout. Add inline comments with the official palette name so the file is self-documenting:

```rust
pub fn tokyonight() -> Self {
    Self {
        border:       Color::Rgb(86, 95, 137),   // storm border
        border_dim:   Color::Rgb(54, 58, 79),    // dim border
        text:         Color::Rgb(192, 202, 245),  // fg
        text_dim:     Color::Rgb(86, 95, 137),   // comment
        text_accent:  Color::Rgb(122, 162, 247),  // blue
        // ... etc
    }
}
```

### Config persistence

Store the theme name in the app config (TOML):

```toml
[ui]
theme = "tokyonight"
```

```rust
#[derive(Deserialize, Serialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String { "tokyonight".to_string() }
```

---

## Layout Patterns

### Screen routing

Single top-level `render` function dispatches to screens, then draws overlays on top:

```rust
pub fn render(f: &mut Frame, app: &mut App) {
    let t = app.theme.clone();

    match &app.screen {
        Screen::Main   => main_screen::render(f, app, &t),
        Screen::Detail => detail_screen::render(f, app, &t),
    }

    // Overlays drawn last (on top of everything)
    if app.show_help         { help::render(f, &t); }
    if app.show_theme_picker { theme_picker::render(f, app, &t); }
}
```

### Standard 3-row screen (list / main view)

```
┌──────────────────────┐
│ header  (2 rows)     │  app name, repo, active filter badge
├──────────────────────┤
│ content (min/flex)   │  scrollable list or main widget
├──────────────────────┤
│ status bar (1 row)   │  key hint badges
└──────────────────────┘
```

```rust
let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(2),  // header
        Constraint::Min(0),     // content
        Constraint::Length(1),  // status bar
    ])
    .split(area);
```

### Standard 4-row screen (detail / tabbed view)

```
┌──────────────────────┐
│ item header (5 rows) │  bordered block with title, meta, stats
├──────────────────────┤
│ tabs       (2 rows)  │  Tab A │ Tab B │ Tab C
├──────────────────────┤
│ content    (flex)    │  swapped by active tab
├──────────────────────┤
│ status bar (1 row)   │  key hint badges
└──────────────────────┘
```

```rust
let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(5),  // item header
        Constraint::Length(2),  // tabs
        Constraint::Min(0),     // content
        Constraint::Length(1),  // status bar
    ])
    .split(area);
```

### Responsive two-panel layout

Switch between unified and side-by-side based on terminal width:

```rust
const SPLIT_THRESHOLD: u16 = 160;

if area.width >= SPLIT_THRESHOLD {
    render_side_by_side(f, data, area, t);
} else {
    render_unified(f, data, area, t);
}

// Side-by-side: 50/50 horizontal split
let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
    .split(area);
```

---

## Overlay Patterns

### Percentage-based centered popup (e.g. help)

```rust
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}

// Usage:
let area = centered_rect(60, 70, f.area());
f.render_widget(Clear, area);   // ← always Clear before drawing overlay
```

### Fixed-size centered popup (e.g. picker)

```rust
fn picker_rect(r: Rect) -> Rect {
    let height = 12u16;
    let width  = 36u16;

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(r.height.saturating_sub(height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(r.width.saturating_sub(width) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vert[1])[1]
}
```

---

## Widget Patterns

### Status bar with key hint badges

Consistent across all screens. Key label in a colored badge, description text alongside, separated by `·`:

```rust
fn render_statusbar(f: &mut Frame, area: Rect, t: &Theme) {
    let hints: &[(&str, &str)] = &[
        ("j/k",   "navigate"),
        ("Enter", "open"),
        ("q",     "quit"),
    ];

    let mut spans = vec![Span::raw(" ")];
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", t.text_dim_style()));
        }
        spans.push(Span::styled(format!(" {key} "), t.key_badge_style()));
        spans.push(Span::styled(format!(" {desc}"), t.key_desc_style()));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
```

### Tab bar

```rust
let titles: Vec<Line> = tab_names.iter().enumerate().map(|(i, name)| {
    if i == selected {
        Line::from(Span::styled(*name, t.tab_active_style()))
    } else {
        Line::from(Span::styled(*name, t.tab_inactive_style()))
    }
}).collect();

let tabs = Tabs::new(titles)
    .select(selected)
    .block(Block::default().borders(Borders::BOTTOM).border_style(t.border_dim_style()))
    .highlight_style(t.tab_active_style())
    .divider(Span::styled(" │ ", t.border_dim_style()));
```

### Bordered block with accent title

```rust
Block::default()
    .borders(Borders::ALL)
    .border_style(t.border_style())
    .title(Span::styled(
        " Title ",
        Style::default().fg(t.text_accent).add_modifier(Modifier::BOLD),
    ))
```

### List rows with per-column styling

Build each row as a `Line` with multiple `Span`s, each with its own style. For selected rows, apply `selection_style()` as the base and override fg per span:

```rust
let base = if selected { t.selection_style() } else { Style::default() };

Line::from(vec![
    Span::styled(id_col,    base.fg(t.pr_number).add_modifier(Modifier::BOLD)),
    Span::styled(" ",       base),
    Span::styled(title_col, base.fg(t.text)),
    Span::styled(meta_col,  base.fg(t.text_dim)),
])
```

### Title truncation

```rust
fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        chars[..max_chars.saturating_sub(1)].iter().collect::<String>() + "…"
    }
}
```

### Diff line rendering

```rust
let (prefix, style) = match line.kind {
    Added   => ("+", t.diff_added_style()),    // fg + bg tint
    Removed => ("-", t.diff_removed_style()),  // fg + bg tint
    Context => (" ", t.diff_context_style()),  // fg only
};
lines.push(Line::from(Span::styled(format!("{prefix}{}", line.content), style)));
```

### Section separator in scrollable content

```rust
lines.push(Line::from(Span::styled(
    " ".to_string() + &"─".repeat(48),
    t.separator_style(),
)));
```

### Loading / error state guard

Apply consistently at the top of every render function before touching data:

```rust
pub fn render(f: &mut Frame, app: &App, area: Rect, t: &Theme) {
    match &app.load_state {
        LoadState::Loading => {
            let p = Paragraph::new("  Loading…")
                .style(t.text_dim_style())
                .block(Block::default().borders(Borders::ALL).border_style(t.border_style()));
            f.render_widget(p, area);
            return;
        }
        LoadState::Error(e) => {
            let p = Paragraph::new(format!("  Error: {e}"))
                .style(t.diff_removed_style())
                .block(Block::default().borders(Borders::ALL).border_style(t.border_style()));
            f.render_widget(p, area);
            return;
        }
        LoadState::Idle => {}
    }
    // ... normal render
}
```

---

## Theme Picker UX

- Opening the picker saves the current theme as `theme_picker_original`
- Navigating (j/k) applies the theme **live** immediately
- `Enter` commits: writes the id to config
- `Esc` reverts to `theme_picker_original`

```rust
KeyCode::Char('j') => {
    self.theme_picker_cursor += 1;
    self.theme = Theme::from_name(ALL_THEMES[self.theme_picker_cursor].0);
}
KeyCode::Enter => {
    self.config.ui.theme = ALL_THEMES[self.theme_picker_cursor].0.to_string();
    self.show_theme_picker = false;
}
KeyCode::Esc => {
    if let Some(original) = self.theme_picker_original.take() {
        self.theme = original;  // revert
    }
    self.show_theme_picker = false;
}
```

---

## File Structure

```
src/
├── app.rs              # App state, event loop, key handling
├── config.rs           # Config load/save (includes theme name)
└── ui/
    ├── mod.rs          # Top-level render dispatcher + overlay routing
    ├── theme.rs        # Theme struct, ALL_THEMES registry, constructors
    ├── theme_picker.rs # Theme picker overlay
    ├── help.rs         # Help overlay
    ├── <screen>.rs     # One file per screen/panel
    └── ...
```

---

## Checklist for a New App

- [ ] Define `Theme` struct with named semantic slots
- [ ] Add convenience `*_style()` methods on `Theme`
- [ ] Define `ALL_THEMES` registry + `from_name` + `index_of`
- [ ] Persist theme name in config (default to a sensible theme)
- [ ] Top-level `render` draws screen then overlays
- [ ] Every screen: header + content + status bar layout
- [ ] Status bar uses `key_badge_style()` + `·` separators
- [ ] Overlays: `Clear` widget first, then draw
- [ ] Theme picker: live preview on navigate, revert on Esc
- [ ] Loading/error guards at top of every data-dependent render fn
- [ ] List rows: `selection_style()` as base, override fg per span
