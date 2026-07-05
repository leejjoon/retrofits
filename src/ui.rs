use crate::app::{App, InputMode};
use crate::keymap;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

/// Clear a centered area, draw a bordered popup frame, and return the inner
/// rect content should be rendered into.
fn popup_frame(f: &mut Frame, title: &str, color: Color, percent_x: u16, percent_y: u16) -> Rect {
    let area = centered_rect(percent_x, percent_y, f.area());
    f.render_widget(Clear, area);
    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color));
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    // Update term size and queue re-render on resize
    let new_term_size = (chunks[0].width, chunks[0].height);
    if app.term_size != new_term_size {
        app.term_size = new_term_size;
        app.queue_render();
    }

    // Main image area
    let image_widget = StatefulImage::default().resize(Resize::Scale(None));
    f.render_stateful_widget(image_widget, chunks[0], &mut app.protocol);

    draw_status_bar(f, app, chunks[1]);

    if let InputMode::Help { scroll } = app.input_mode {
        let inner = popup_frame(f, "Help / Keybindings", Color::Cyan, 50, 60);
        let paragraph = Paragraph::new(help_lines()).scroll((scroll, 0));
        f.render_widget(paragraph, inner);
    }

    if matches!(app.input_mode, InputMode::Summary) {
        let inner = popup_frame(f, "Viewport Summary", Color::Green, 50, 60);

        let mut text = vec![
            ratatui::text::Line::from(vec![
                Span::styled("File: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&app.filename),
            ]),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(vec![
                Span::styled(
                    "Image Dimensions: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("{} x {}", app.fits.width, app.fits.height)),
            ]),
            ratatui::text::Line::from(vec![
                Span::styled("Zoom: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{:.2}x", app.zoom)),
            ]),
            ratatui::text::Line::from(vec![
                Span::styled("Center: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("({:.1}, {:.1})", app.center.0, app.center.1)),
            ]),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(vec![
                Span::styled(
                    "Stretch (s): ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(app.stretch.to_string()),
            ]),
            ratatui::text::Line::from(vec![
                Span::styled(
                    "Colormap (c): ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(app.colormap.to_string()),
            ]),
            ratatui::text::Line::from(vec![
                Span::styled(
                    "Cut Mode (z): ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("{}", app.cut_mode)),
            ]),
            ratatui::text::Line::from(vec![
                Span::styled(
                    "Data Range: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("[{:.4}, {:.4}]", app.black_point, app.white_point)),
            ]),
            ratatui::text::Line::from(""),
        ];

        let guessed_proto_name = crate::app::protocol_name(app.guessed_protocol);
        let proto_name = crate::app::protocol_name(app.protocol_type);

        let proto_status = if app.protocol_type != ratatui_image::picker::ProtocolType::Halfblocks
            && app.protocol_type != app.guessed_protocol
        {
            Span::styled(
                " (May be unsupported - best guess is ",
                Style::default().fg(Color::Yellow),
            )
        } else {
            Span::raw("")
        };

        let proto_status_end = if app.protocol_type
            != ratatui_image::picker::ProtocolType::Halfblocks
            && app.protocol_type != app.guessed_protocol
        {
            Span::styled(
                format!("{})", guessed_proto_name),
                Style::default().fg(Color::Yellow),
            )
        } else {
            Span::raw("")
        };

        text.push(ratatui::text::Line::from(vec![
            Span::styled(
                "Protocol (p): ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(proto_name),
            proto_status,
            proto_status_end,
        ]));

        text.push(ratatui::text::Line::from(""));
        text.push(ratatui::text::Line::from(Span::styled(
            " [Esc/q/w] Close Summary ",
            Style::default().fg(Color::DarkGray),
        )));

        let paragraph = Paragraph::new(text);
        f.render_widget(paragraph, inner);
    }

    // Handle Manual Cut Popup
    if matches!(
        app.input_mode,
        InputMode::EditingBlackPoint | InputMode::EditingWhitePoint
    ) {
        let inner = popup_frame(f, "Manual Cut Entry", Color::Yellow, 60, 25);

        let inner_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(inner);

        let entry_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner_layout[1]);

        let editing_black = matches!(app.input_mode, InputMode::EditingBlackPoint);

        let black_style = if editing_black {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let white_style = if !editing_black {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let black_text = if editing_black {
            &app.input_buffer
        } else {
            &format!("{:.6}", app.black_point)
        };

        let white_text = if !editing_black {
            &app.input_buffer
        } else {
            &format!("{:.6}", app.white_point)
        };

        let black_input = Paragraph::new(black_text.as_str()).block(
            Block::default()
                .title(" Black Point (Low Cut) ")
                .borders(Borders::ALL)
                .border_style(black_style),
        );

        let white_input = Paragraph::new(white_text.as_str()).block(
            Block::default()
                .title(" White Point (High Cut) ")
                .borders(Borders::ALL)
                .border_style(white_style),
        );

        f.render_widget(black_input, entry_chunks[0]);
        f.render_widget(white_input, entry_chunks[1]);

        let help_text = Paragraph::new(" [Enter] Apply  [Tab/Arrows] Switch  [Esc/q] Close ")
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(help_text, inner_layout[2]);
    }

    if matches!(app.input_mode, InputMode::SelectingExtension { .. }) {
        let inner = popup_frame(f, "Select Extension", Color::Cyan, 50, 50);

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

                let name = if ext.name.is_empty() {
                    " ".to_string()
                } else {
                    format!(" [{}]", ext.name)
                };

                let item_text = format!(
                    "{:>3}: {:<11} {}",
                    i,
                    if ext.is_image { "Image" } else { "Table/Other" },
                    name
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
            " [Enter] Load  [Esc/q/e] Close ",
            Style::default().fg(Color::DarkGray),
        ));
        f.render_widget(hint, list_layout[1]);
    }
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

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
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
        .split(popup_layout[1])[1]
}
