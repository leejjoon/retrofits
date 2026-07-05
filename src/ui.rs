use crate::app::{App, InputMode};
use crate::keymap;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

/// Width of the right-hand side panel (summary, extension picker).
const SIDE_PANEL_WIDTH: u16 = 34;
/// Height of the bottom manual-cut strip.
const CUT_PANEL_HEIGHT: u16 = 4;

/// Top-level layout.
///
/// Popups are deliberately NOT overlaid on the image: overlaying a ratatui
/// `Clear` on cells occupied by a terminal graphics protocol (Kitty/Sixel)
/// leaves stale pixels when the popup closes, which historically required
/// per-protocol redraw workarounds (see DEVELOP.md). Instead, panels get
/// their own screen region and the image rect shrinks; the resize is
/// detected below and triggers a natural re-encode.
pub fn draw(f: &mut Frame, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());
    let main = outer[0];

    // Full-screen pages replace the image entirely.
    if let InputMode::Help { scroll } = app.input_mode {
        draw_help_page(f, main, scroll);
        draw_status_bar(f, app, outer[1]);
        return;
    }
    if matches!(app.input_mode, InputMode::HeaderView { .. }) {
        draw_header_page(f, app, main);
        draw_status_bar(f, app, outer[1]);
        return;
    }

    let (image_area, panel_area) = match app.input_mode {
        InputMode::Summary | InputMode::SelectingExtension { .. } => {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(0), Constraint::Length(SIDE_PANEL_WIDTH)])
                .split(main);
            (cols[0], Some(cols[1]))
        }
        InputMode::EditingBlackPoint | InputMode::EditingWhitePoint => {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(CUT_PANEL_HEIGHT)])
                .split(main);
            (rows[0], Some(rows[1]))
        }
        _ => (main, None),
    };

    // Update term size and queue re-render when the image rect changes
    // (terminal resize or a panel opening/closing).
    let new_term_size = (image_area.width, image_area.height);
    if app.term_size != new_term_size {
        app.term_size = new_term_size;
        app.queue_render();
    }

    // Main image area
    let image_widget = StatefulImage::default().resize(Resize::Scale(None));
    f.render_stateful_widget(image_widget, image_area, &mut app.protocol);

    if let Some(panel) = panel_area {
        if matches!(app.input_mode, InputMode::Summary) {
            draw_summary_panel(f, app, panel);
        } else if matches!(app.input_mode, InputMode::SelectingExtension { .. }) {
            draw_extension_panel(f, app, panel);
        } else {
            draw_cut_panel(f, app, panel);
        }
    }

    draw_status_bar(f, app, outer[1]);
}

fn draw_help_page(f: &mut Frame, area: Rect, scroll: u16) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let block = Block::default()
        .title(" Help / Keybindings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(help_lines())
        .block(block)
        .scroll((scroll, 0));
    f.render_widget(paragraph, rows[0]);

    let footer = Paragraph::new(Span::styled(
        " j/k:scroll  Esc/q/h:close ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(footer, rows[1]);
}

/// Byte ranges of case-insensitive occurrences of `query_lower` in `text`.
/// ASCII-only case folding keeps byte offsets valid for slicing.
fn match_ranges(text: &str, query_lower: &str) -> Vec<(usize, usize)> {
    if query_lower.is_empty() {
        return Vec::new();
    }
    let hay = text.to_ascii_lowercase();
    let mut ranges = Vec::new();
    let mut start = 0;
    while let Some(pos) = hay[start..].find(query_lower) {
        let s = start + pos;
        ranges.push((s, s + query_lower.len()));
        start = s + query_lower.len();
    }
    ranges
}

/// Split `text` into spans, highlighting every occurrence of the query.
fn highlighted_line(text: String, query_lower: &str, is_current: bool) -> Line<'static> {
    let highlight = if is_current {
        Style::default()
            .bg(Color::LightYellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::Yellow).fg(Color::Black)
    };
    let mut spans = Vec::new();
    let mut cursor = 0;
    for (s, e) in match_ranges(&text, query_lower) {
        if s > cursor {
            spans.push(Span::raw(text[cursor..s].to_string()));
        }
        spans.push(Span::styled(text[s..e].to_string(), highlight));
        cursor = e;
    }
    if cursor < text.len() {
        spans.push(Span::raw(text[cursor..].to_string()));
    }
    Line::from(spans)
}

/// One header card as a styled line (used for non-matching lines).
fn header_entry_line(entry: &crate::fits::HeaderEntry) -> Line<'static> {
    use crate::fits::HeaderEntry;
    match entry {
        HeaderEntry::Value {
            keyword,
            value,
            comment,
        } => {
            let mut spans = vec![
                Span::styled(
                    format!("{:<8}", keyword),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("= "),
                Span::raw(value.clone()),
            ];
            if let Some(c) = comment {
                spans.push(Span::styled(
                    format!(" / {}", c),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            Line::from(spans)
        }
        HeaderEntry::Comment(s) => Line::from(Span::styled(
            format!("COMMENT {}", s),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )),
        HeaderEntry::History(s) => Line::from(Span::styled(
            format!("HISTORY {}", s),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )),
        HeaderEntry::Blank => Line::from(""),
    }
}

/// Full-screen, scrollable, searchable FITS header page (less-style).
fn draw_header_page(f: &mut Frame, app: &App, area: Rect) {
    let (scroll, search) = match &app.input_mode {
        InputMode::HeaderView { scroll, search } => (*scroll, search),
        _ => return,
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let query_lower = search.query.to_ascii_lowercase();
    let current_line = search.matches.get(search.current).copied();
    let lines: Vec<Line> = app
        .fits
        .header
        .0
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_match = !query_lower.is_empty() && search.matches.binary_search(&i).is_ok();
            if is_match {
                highlighted_line(entry.display_text(), &query_lower, Some(i) == current_line)
            } else {
                header_entry_line(entry)
            }
        })
        .collect();

    let block = Block::default()
        .title(format!(" FITS Header \u{2014} {} ", app.filename))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll.min(u16::MAX as usize) as u16, 0));
    f.render_widget(paragraph, rows[0]);

    // Footer: search prompt / match status / key hints.
    let footer: Line = if search.input_active {
        Line::from(vec![
            Span::styled("/", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(search.query.clone()),
            Span::styled("\u{2588}", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "   Enter:confirm  Esc:cancel",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else if !search.query.is_empty() {
        if search.matches.is_empty() {
            Line::from(Span::styled(
                format!(" Pattern not found: {}   /:search  Esc/q:close ", search.query),
                Style::default().fg(Color::Red),
            ))
        } else {
            Line::from(vec![
                Span::styled(
                    format!(
                        " [{}/{} matches for '{}'] ",
                        search.current + 1,
                        search.matches.len(),
                        search.query
                    ),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    " n/N:next/prev  /:search  Esc/q:close ",
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        }
    } else {
        Line::from(Span::styled(
            " j/k:scroll  g/G:top/bottom  PgUp/PgDn:page  /:search  Esc/q:close ",
            Style::default().fg(Color::DarkGray),
        ))
    };
    f.render_widget(Paragraph::new(footer), rows[1]);
}

fn draw_summary_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Summary ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut text = vec![
        Line::from(vec![Span::styled("File: ", bold), Span::raw(&app.filename)]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Dimensions: ", bold),
            Span::raw(format!("{} x {}", app.fits.width, app.fits.height)),
        ]),
        Line::from(vec![
            Span::styled("Zoom: ", bold),
            Span::raw(format!("{:.2}x", app.zoom)),
        ]),
        Line::from(vec![
            Span::styled("Center: ", bold),
            Span::raw(format!("({:.1}, {:.1})", app.center.0, app.center.1)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Stretch (s): ", bold),
            Span::raw(app.stretch.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Colormap (c): ", bold),
            Span::raw(app.colormap.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Cut Mode (z): ", bold),
            Span::raw(format!("{}", app.cut_mode)),
        ]),
        Line::from(vec![
            Span::styled("Data Range: ", bold),
            Span::raw(format!("[{:.4}, {:.4}]", app.black_point, app.white_point)),
        ]),
        Line::from(""),
    ];

    let proto_name = crate::app::protocol_name(app.protocol_type);
    let mut proto_line = vec![Span::styled("Protocol (p): ", bold), Span::raw(proto_name)];
    if app.protocol_type != ratatui_image::picker::ProtocolType::Halfblocks
        && app.protocol_type != app.guessed_protocol
    {
        proto_line.push(Span::styled(
            format!(
                " (may be unsupported - best guess is {})",
                crate::app::protocol_name(app.guessed_protocol)
            ),
            Style::default().fg(Color::Yellow),
        ));
    }
    text.push(Line::from(proto_line));

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        " [Esc/q/w] Close ",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}

fn draw_extension_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .title(" Extensions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let list_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let current = app.fits.current_extension;
    let items: Vec<ListItem> = app
        .fits
        .extensions
        .iter()
        .enumerate()
        .map(|(i, ext)| {
            let style = if !ext.is_image {
                Style::default().fg(Color::DarkGray)
            } else if i == current {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            let marker = if i == current { "\u{25cf}" } else { " " };
            let name = if ext.name.is_empty() {
                String::new()
            } else {
                format!(" [{}]", ext.name)
            };
            let item_text = format!(
                "{}{:>2} {:<8}{} {}",
                marker,
                i,
                ext.kind.to_string(),
                name,
                ext.describe()
            );
            ListItem::new(Line::from(Span::styled(item_text, style)))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    if let InputMode::SelectingExtension { state } = &mut app.input_mode {
        f.render_stateful_widget(list, list_layout[0], state);
    }

    let hint = Paragraph::new(Span::styled(
        " Enter:load  Esc/q/e:close ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(hint, list_layout[1]);
}

fn draw_cut_panel(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(1)])
        .split(area);

    let entry_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    let editing_black = matches!(app.input_mode, InputMode::EditingBlackPoint);

    let active = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let inactive = Style::default();

    let black_text = if editing_black {
        app.input_buffer.clone()
    } else {
        format!("{:.6}", app.black_point)
    };
    let white_text = if !editing_black {
        app.input_buffer.clone()
    } else {
        format!("{:.6}", app.white_point)
    };

    let black_input = Paragraph::new(black_text).block(
        Block::default()
            .title(" Black Point (Low Cut) ")
            .borders(Borders::ALL)
            .border_style(if editing_black { active } else { inactive }),
    );
    let white_input = Paragraph::new(white_text).block(
        Block::default()
            .title(" White Point (High Cut) ")
            .borders(Borders::ALL)
            .border_style(if !editing_black { active } else { inactive }),
    );

    f.render_widget(black_input, entry_chunks[0]);
    f.render_widget(white_input, entry_chunks[1]);

    let help_text = Paragraph::new(" [Enter] Apply  [Tab/Arrows] Switch  [Esc/q] Close ")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help_text, rows[1]);
}

/// Compact float formatting for the status bar: scientific notation for
/// very large/small magnitudes, fixed-point otherwise.
fn fmt_cut(v: f32) -> String {
    let a = v.abs();
    if a != 0.0 && (a >= 100_000.0 || a < 0.01) {
        format!("{:.2e}", v)
    } else {
        format!("{:.2}", v)
    }
}

/// Three-segment status line:
/// `file [ext] dims │ zoom stretch colormap cut[b,w] │ protocol hints`
///
/// While a transient message is live it replaces the middle segment, in a
/// color matching its severity. Segments are dropped right-to-left when the
/// terminal is too narrow.
fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    use crate::app::{protocol_name, Severity};

    let base = Style::default().bg(Color::Blue).fg(Color::White);

    // Left segment: filename [ext] dims
    let max_filename_len = 20;
    let filename_chars: Vec<char> = app.filename.chars().collect();
    let filename = if filename_chars.len() > max_filename_len {
        let truncated: String = filename_chars
            .into_iter()
            .take(max_filename_len - 3)
            .collect();
        format!("{}...", truncated)
    } else {
        app.filename.clone()
    };
    let ext_str = if app.fits.extensions.len() > 1 {
        let ext = &app.fits.extensions[app.fits.current_extension];
        if ext.name.is_empty() {
            format!(" [{}]", ext.index)
        } else {
            format!(" [{}:{}]", ext.index, ext.name)
        }
    } else {
        String::new()
    };
    let left = format!(
        " {}{} {}\u{d7}{} ",
        filename, ext_str, app.fits.width, app.fits.height
    );

    // Middle segment: viewport / display state, or a live message.
    let (middle, middle_style) = match &app.message {
        Some(msg) => {
            let style = match msg.severity {
                Severity::Info => base.add_modifier(Modifier::BOLD),
                Severity::Warn => base.fg(Color::Yellow).add_modifier(Modifier::BOLD),
                Severity::Error => Style::default()
                    .bg(Color::Red)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            };
            (format!(" {} ", msg.text), style)
        }
        None => (
            format!(
                " {:.2}x  {}  {}  {}[{},{}] ",
                app.zoom,
                app.stretch,
                app.colormap,
                app.cut_mode,
                fmt_cut(app.black_point),
                fmt_cut(app.white_point)
            ),
            base,
        ),
    };

    // Right segment: protocol (+ mismatch marker) and key hints.
    let proto_warn = app.protocol_type != ratatui_image::picker::ProtocolType::Halfblocks
        && app.protocol_type != app.guessed_protocol;
    let right = format!(
        " {}{}  h:help q:quit ",
        protocol_name(app.protocol_type),
        if proto_warn { "!" } else { "" }
    );

    // Assemble, dropping segments right-to-left if the bar is too narrow.
    let width = area.width as usize;
    let sep = Span::styled("\u{2502}", base.fg(Color::LightBlue));
    let mut spans = vec![
        Span::styled(left.clone(), base),
        sep.clone(),
        Span::styled(middle.clone(), middle_style),
    ];
    let used = left.chars().count() + 1 + middle.chars().count();
    if used + 1 + right.chars().count() <= width {
        spans.push(sep);
        if proto_warn {
            let (proto_part, rest) = right.split_at(right.find("  ").unwrap_or(right.len()));
            spans.push(Span::styled(proto_part.to_string(), base.fg(Color::Yellow)));
            spans.push(Span::styled(rest.to_string(), base));
        } else {
            spans.push(Span::styled(right, base));
        }
    } else if used > width {
        // Too narrow even for left+middle: keep the middle (most dynamic).
        spans = vec![Span::styled(middle, middle_style)];
    }

    let status_bar = Paragraph::new(Line::from(spans)).style(base);
    f.render_widget(status_bar, area);
}

/// Build the help window content from the keymap, grouped by category, so
/// the displayed shortcuts always match the actual bindings.
fn help_lines() -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut last_category = "";
    for binding in keymap::KEYMAP {
        if binding.category != last_category {
            if !last_category.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                format!("{}:", binding.category),
                Style::default().add_modifier(Modifier::BOLD),
            )));
            last_category = binding.category;
        }
        let keys = binding
            .keys
            .iter()
            .map(keymap::key_name)
            .collect::<Vec<_>>()
            .join(" / ");
        lines.push(Line::from(format!("  {:<18} : {}", keys, binding.help)));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_ranges_basic() {
        assert_eq!(match_ranges("NAXIS1  = 2056", "naxis"), vec![(0, 5)]);
        assert_eq!(match_ranges("ABAB", "ab"), vec![(0, 2), (2, 4)]);
        assert_eq!(match_ranges("no hit here", "xyz"), Vec::<(usize, usize)>::new());
        assert_eq!(match_ranges("anything", ""), Vec::<(usize, usize)>::new());
    }

    #[test]
    fn test_match_ranges_case_insensitive() {
        assert_eq!(match_ranges("TELESCOP= 'Kepler'", "kepler"), vec![(11, 17)]);
    }

    #[test]
    fn test_highlighted_line_spans_reassemble() {
        let text = "EXPTIME = 300.0 / exposure time".to_string();
        let line = highlighted_line(text.clone(), "time", false);
        // Splitting into spans must not lose or duplicate any text.
        let rebuilt: String = line.spans.iter().map(|s| s.content.as_ref()).collect::<String>();
        assert_eq!(rebuilt, text);
        // Two case-insensitive occurrences of "time" -> two highlighted spans.
        let highlighted = line
            .spans
            .iter()
            .filter(|s| s.style.bg == Some(Color::Yellow))
            .count();
        assert_eq!(highlighted, 2);
    }
}
