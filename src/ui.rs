use crate::app::{App, InputMode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

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

    // Status bar
    let max_filename_len = 20;
    let filename_chars: Vec<char> = app.filename.chars().collect();
    let filename = if filename_chars.len() > max_filename_len {
        let truncated: String = filename_chars.into_iter().take(max_filename_len - 3).collect();
        format!("{}...", truncated)
    } else {
        app.filename.clone()
    };

    let proto_str = match app.protocol_type {
        ratatui_image::picker::ProtocolType::Halfblocks => "Halfblocks",
        ratatui_image::picker::ProtocolType::Sixel => "Sixel",
        ratatui_image::picker::ProtocolType::Kitty => "Kitty",
        ratatui_image::picker::ProtocolType::Iterm2 => "iTerm2",
    };

    // Stretch symbol
    let stretch_sym = match app.stretch {
        crate::stretch::StretchFunction::Linear => "➖",
        crate::stretch::StretchFunction::Logarithmic => "📈",
        crate::stretch::StretchFunction::Asinh => "〰️",
    };

    // Colormap symbol (just generic)
    let cmap_sym = "🎨";

    // Zoom symbol
    let zoom_sym = "🔍";

    let status_text = format!(
        " [{}] {} {:.2}x {} {} | p: {} | z: {} | w: summary | h: help | q: quit ",
        filename, zoom_sym, app.zoom, stretch_sym, cmap_sym, proto_str, app.cut_mode
    );

    let status_bar = Paragraph::new(Span::raw(status_text))
        .style(Style::default().bg(Color::Blue).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(status_bar, chunks[1]);

    if let InputMode::Help { scroll } = app.input_mode {
        let area = centered_rect(50, 60, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title(" Help / Keybindings ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let help_lines = vec![
            ratatui::text::Line::from(vec![Span::styled(
                "Navigation:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            ratatui::text::Line::from("  Arrow Keys / j,k,l : Pan image"), // removed 'h' from pan
            ratatui::text::Line::from("  + / i              : Zoom in"),
            ratatui::text::Line::from("  - / o              : Zoom out"),
            ratatui::text::Line::from("  r                  : Reset zoom and center"),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(vec![Span::styled(
                "Image Controls:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            ratatui::text::Line::from(
                "  s                  : Cycle stretch function (Linear, Log, Asinh)",
            ),
            ratatui::text::Line::from("  c                  : Cycle colormap"),
            ratatui::text::Line::from(
                "  z                  : Cycle cut mode (MinMax, ZScale, Custom)",
            ),
            ratatui::text::Line::from("  m                  : Set custom cut points (manual)"),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(vec![Span::styled(
                "App Controls:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            ratatui::text::Line::from(
                "  p                  : Cycle image protocol (Halfblocks, Sixel, Kitty, iTerm2)",
            ),
            ratatui::text::Line::from("  w                  : Toggle summary window"),
            ratatui::text::Line::from("  h                  : Toggle help window"),
            ratatui::text::Line::from("  q / Esc            : Quit application / Close popups"),
        ];

        let paragraph = Paragraph::new(help_lines).block(block).scroll((scroll, 0));

        f.render_widget(paragraph, area);
    }

    if app.input_mode == InputMode::Summary {
        let area = centered_rect(50, 60, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title(" Viewport Summary ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

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
                Span::raw(match app.stretch {
                    crate::stretch::StretchFunction::Linear => "Linear",
                    crate::stretch::StretchFunction::Logarithmic => "Logarithmic",
                    crate::stretch::StretchFunction::Asinh => "Asinh",
                }),
            ]),
            ratatui::text::Line::from(vec![
                Span::styled(
                    "Colormap (c): ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(match app.colormap {
                    crate::colormap::ColormapName::Grayscale => "Grayscale",
                    crate::colormap::ColormapName::Viridis => "Viridis",
                    crate::colormap::ColormapName::Plasma => "Plasma",
                    crate::colormap::ColormapName::Inferno => "Inferno",
                    crate::colormap::ColormapName::Magma => "Magma",
                }),
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

        let guessed_proto_name = match app.guessed_protocol {
            ratatui_image::picker::ProtocolType::Halfblocks => "Halfblocks",
            ratatui_image::picker::ProtocolType::Sixel => "Sixel",
            ratatui_image::picker::ProtocolType::Kitty => "Kitty",
            ratatui_image::picker::ProtocolType::Iterm2 => "iTerm2",
        };

        let proto_name = match app.protocol_type {
            ratatui_image::picker::ProtocolType::Halfblocks => "Halfblocks",
            ratatui_image::picker::ProtocolType::Sixel => "Sixel",
            ratatui_image::picker::ProtocolType::Kitty => "Kitty",
            ratatui_image::picker::ProtocolType::Iterm2 => "iTerm2",
        };

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

        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(paragraph, area);
    }

    // Handle Manual Cut Popup
    if app.input_mode == InputMode::EditingBlackPoint
        || app.input_mode == InputMode::EditingWhitePoint
    {
        let area = centered_rect(60, 25, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title(" Manual Cut Entry ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        f.render_widget(block, area);

        let inner_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .margin(1)
            .split(area);

        let entry_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner_layout[1]);

        let black_style = if app.input_mode == InputMode::EditingBlackPoint {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let white_style = if app.input_mode == InputMode::EditingWhitePoint {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let black_text = if app.input_mode == InputMode::EditingBlackPoint {
            &app.input_buffer
        } else {
            &format!("{:.6}", app.black_point)
        };

        let white_text = if app.input_mode == InputMode::EditingWhitePoint {
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
