use crate::app::{App, InputMode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Modifier},
    text::Span,
    widgets::{Block, Borders, Paragraph, Clear},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
        ])
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
    let status_text = format!(
        " {}x{} | Zoom: {:.2}x | Center: ({:.1}, {:.1}) | Mode: {} | Cuts: [{:.4}, {:.4}] | s: stretch, c: colormap, z: cycle cut, m: manual popup, q: quit ",
        app.fits.width, app.fits.height, app.zoom, app.center.0, app.center.1, app.cut_mode, app.black_point, app.white_point
    );

    let status_bar = Paragraph::new(Span::raw(status_text))
        .style(Style::default().bg(Color::Blue).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(status_bar, chunks[1]);

    // Handle Manual Cut Popup
    if app.input_mode != InputMode::Normal {
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
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(inner_layout[1]);

        let black_style = if app.input_mode == InputMode::EditingBlackPoint {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let white_style = if app.input_mode == InputMode::EditingWhitePoint {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
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

        let black_input = Paragraph::new(black_text.as_str())
            .block(Block::default().title(" Black Point (Low Cut) ").borders(Borders::ALL).border_style(black_style));
        
        let white_input = Paragraph::new(white_text.as_str())
            .block(Block::default().title(" White Point (High Cut) ").borders(Borders::ALL).border_style(white_style));

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
