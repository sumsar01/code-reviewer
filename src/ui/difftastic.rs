use crate::app::{App, LoadState};
use crate::ui::theme::Theme;
use ratatui::{
    layout::Rect,
    style::Style,
    text::Span,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect, t: &Theme) {
    match &app.difft_load_state {
        LoadState::Loading => {
            let p = Paragraph::new("  Running difftastic…")
                .style(t.text_dim_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(t.border_style())
                        .title(Span::styled(
                            " Difftastic ",
                            Style::default().fg(t.text_dim),
                        )),
                );
            f.render_widget(p, area);
            return;
        }
        LoadState::Error(e) => {
            let p = Paragraph::new(format!("  {e}"))
                .style(t.diff_removed_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(t.border_style())
                        .title(Span::styled(
                            " Difftastic ",
                            Style::default().fg(t.text_dim),
                        )),
                );
            f.render_widget(p, area);
            return;
        }
        LoadState::Idle => {}
    }

    if app.difft_files.is_empty() {
        let p = Paragraph::new("  No difftastic output available.").block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .title(Span::styled(
                    " Difftastic ",
                    Style::default().fg(t.text_dim),
                )),
        );
        f.render_widget(p, area);
        return;
    }

    let total_files = app.difft_files.len();
    let cur_file_idx = app.difft_file_cursor.min(total_files.saturating_sub(1));
    let (filename, lines) = &app.difft_files[cur_file_idx];
    let title = format!(" {} ({}/{}) ", filename, cur_file_idx + 1, total_files);

    let p = Paragraph::new(lines.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(t.border_style())
                .title(Span::styled(title, t.text_accent_style())),
        )
        .scroll((app.difft_scroll, 0));
    f.render_widget(p, area);
}

/// Parse ANSI SGR escape sequences from difftastic output into ratatui `Line`s.
///
/// Handles:
///   - `\x1b[0m`       reset
///   - `\x1b[1m`       bold
///   - `\x1b[3m`       italic
///   - `\x1b[4m`       underline
///   - `\x1b[30–37m`   standard foreground colors
///   - `\x1b[90–97m`   bright foreground colors
///   - `\x1b[38;5;Nm`  256-color foreground
///   - `\x1b[38;2;R;G;Bm` true-color foreground
///   - Multiple codes in one sequence (`\x1b[1;32m`)
pub fn parse_ansi_to_lines(input: &str) -> Vec<ratatui::text::Line<'static>> {
    let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();

    for raw_line in input.lines() {
        let spans = parse_ansi_spans(raw_line);
        lines.push(ratatui::text::Line::from(spans));
    }

    lines
}

fn parse_ansi_spans(input: &str) -> Vec<ratatui::text::Span<'static>> {
    let mut spans: Vec<ratatui::text::Span<'static>> = Vec::new();
    let mut current_style = ratatui::style::Style::default();
    let mut text_start = 0;
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[' {
            if i > text_start {
                let text = input[text_start..i].to_string();
                spans.push(ratatui::text::Span::styled(text, current_style));
            }

            let seq_start = i + 2;
            let mut seq_end = seq_start;
            while seq_end < len && !bytes[seq_end].is_ascii_alphabetic() {
                seq_end += 1;
            }

            if seq_end < len && bytes[seq_end] == b'm' {
                let codes_str = &input[seq_start..seq_end];
                current_style = apply_sgr_codes(current_style, codes_str);
            }

            i = seq_end + 1;
            text_start = i;
        } else {
            i += 1;
        }
    }

    if text_start < len {
        let text = input[text_start..].to_string();
        spans.push(ratatui::text::Span::styled(text, current_style));
    }

    if spans.is_empty() {
        spans.push(ratatui::text::Span::raw(String::new()));
    }

    spans
}

fn apply_sgr_codes(mut style: ratatui::style::Style, codes_str: &str) -> ratatui::style::Style {
    use ratatui::style::{Color, Modifier};

    let codes: Vec<&str> = codes_str.split(';').collect();
    let mut idx = 0;

    while idx < codes.len() {
        let code: u32 = codes[idx].parse().unwrap_or(0);
        match code {
            0 => style = ratatui::style::Style::default(),
            1 => style = style.add_modifier(Modifier::BOLD),
            2 => style = style.add_modifier(Modifier::DIM),
            3 => style = style.add_modifier(Modifier::ITALIC),
            4 => style = style.add_modifier(Modifier::UNDERLINED),
            7 => style = style.add_modifier(Modifier::REVERSED),
            9 => style = style.add_modifier(Modifier::CROSSED_OUT),
            22 => style = style.remove_modifier(Modifier::BOLD | Modifier::DIM),
            23 => style = style.remove_modifier(Modifier::ITALIC),
            24 => style = style.remove_modifier(Modifier::UNDERLINED),
            30 => style = style.fg(Color::Black),
            31 => style = style.fg(Color::Red),
            32 => style = style.fg(Color::Green),
            33 => style = style.fg(Color::Yellow),
            34 => style = style.fg(Color::Blue),
            35 => style = style.fg(Color::Magenta),
            36 => style = style.fg(Color::Cyan),
            37 => style = style.fg(Color::White),
            39 => style = style.fg(Color::Reset),
            40 => style = style.bg(Color::Black),
            41 => style = style.bg(Color::Red),
            42 => style = style.bg(Color::Green),
            43 => style = style.bg(Color::Yellow),
            44 => style = style.bg(Color::Blue),
            45 => style = style.bg(Color::Magenta),
            46 => style = style.bg(Color::Cyan),
            47 => style = style.bg(Color::White),
            49 => style = style.bg(Color::Reset),
            90 => style = style.fg(Color::DarkGray),
            91 => style = style.fg(Color::LightRed),
            92 => style = style.fg(Color::LightGreen),
            93 => style = style.fg(Color::LightYellow),
            94 => style = style.fg(Color::LightBlue),
            95 => style = style.fg(Color::LightMagenta),
            96 => style = style.fg(Color::LightCyan),
            97 => style = style.fg(Color::Gray),
            38 => {
                if idx + 1 < codes.len() {
                    match codes[idx + 1] {
                        "5" if idx + 2 < codes.len() => {
                            let n: u8 = codes[idx + 2].parse().unwrap_or(0);
                            style = style.fg(Color::Indexed(n));
                            idx += 2;
                        }
                        "2" if idx + 4 < codes.len() => {
                            let r: u8 = codes[idx + 2].parse().unwrap_or(0);
                            let g: u8 = codes[idx + 3].parse().unwrap_or(0);
                            let b: u8 = codes[idx + 4].parse().unwrap_or(0);
                            style = style.fg(Color::Rgb(r, g, b));
                            idx += 4;
                        }
                        _ => {}
                    }
                }
            }
            48 => {
                if idx + 1 < codes.len() {
                    match codes[idx + 1] {
                        "5" if idx + 2 < codes.len() => {
                            let n: u8 = codes[idx + 2].parse().unwrap_or(0);
                            style = style.bg(Color::Indexed(n));
                            idx += 2;
                        }
                        "2" if idx + 4 < codes.len() => {
                            let r: u8 = codes[idx + 2].parse().unwrap_or(0);
                            let g: u8 = codes[idx + 3].parse().unwrap_or(0);
                            let b: u8 = codes[idx + 4].parse().unwrap_or(0);
                            style = style.bg(Color::Rgb(r, g, b));
                            idx += 4;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        idx += 1;
    }

    style
}
