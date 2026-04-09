use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
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
        " {}x{} | Zoom: {:.2}x | Center: ({:.0}, {:.0}) | Stretch: {:?} | Colormap: {:?} | Protocol: {:?} | s: stretch, c: colormap, +/-: zoom, q: quit ",
        app.fits.width, app.fits.height, app.zoom, app.center.0, app.center.1, app.stretch, app.colormap, app.protocol_type
    );

    let status_bar = Paragraph::new(Span::raw(status_text))
        .style(Style::default().bg(Color::Blue).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(status_bar, chunks[1]);
}
