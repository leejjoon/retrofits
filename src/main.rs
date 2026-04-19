use retrofits::app::App;
use retrofits::fits;
use retrofits::ui;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use ratatui_image::picker::Picker;
use std::{io, path::PathBuf, time::Duration};

/// RetroFITS — A high-performance FITS image viewer for the terminal.
#[derive(Parser, Debug)]
#[command(name = "retrofits", version, about)]
struct Cli {
    /// Path to the FITS file to view.
    file: PathBuf,

    /// Protocol override (kitty, sixel, iterm2, halfblocks).
    #[arg(short, long)]
    protocol: Option<String>,

    /// FITS extension to load (index or EXTNAME).
    #[arg(short, long)]
    ext: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load FITS parsing
    let fits_image = fits::load_fits(&cli.file, cli.ext.as_deref())?;

    // Initialize Ratatui terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Query terminal for image protocol capabilities
    let mut picker = Picker::from_query_stdio()?;

    // Store the guessed best protocol
    let guessed_protocol = picker.protocol_type();

    // Check if the terminal is Ghostty
    let is_ghostty = std::env::var("TERM_PROGRAM")
        .map(|v| v.to_lowercase() == "ghostty")
        .unwrap_or(false)
        || std::env::var("TERM")
            .map(|v| v.to_lowercase().contains("ghostty"))
            .unwrap_or(false);

    // Default to Halfblocks if no protocol is explicitly requested via CLI
    let default_protocol = if is_ghostty {
        ratatui_image::picker::ProtocolType::Kitty
    } else {
        ratatui_image::picker::ProtocolType::Halfblocks
    };

    // Override protocol if specified
    if let Some(proto_str) = cli.protocol {
        let p = match proto_str.to_lowercase().as_str() {
            "kitty" => ratatui_image::picker::ProtocolType::Kitty,
            "sixel" => ratatui_image::picker::ProtocolType::Sixel,
            "iterm2" => ratatui_image::picker::ProtocolType::Iterm2,
            "halfblocks" => ratatui_image::picker::ProtocolType::Halfblocks,
            _ => ratatui_image::picker::ProtocolType::Halfblocks,
        };
        picker.set_protocol_type(p);
    } else {
        picker.set_protocol_type(default_protocol);
    }

    // Create the App state
    let filename = cli
        .file
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut app = App::new(
        std::sync::Arc::new(fits_image),
        &mut picker,
        filename,
        guessed_protocol,
    )?;

    // Main event loop
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    // Initial render request
    app.queue_render();

    // Initial draw
    terminal.draw(|f| ui::draw(f, app))?;

    while app.running {
        let mut should_draw = false;

        // 1. Check for incoming render thread frames
        if app.try_update_protocol() {
            should_draw = true;
        }

        // 2. Poll for terminal events with a timeout
        // This acts as our event loop wait. A shorter timeout makes input feel snappier,
        // while a longer one saves CPU.
        if event::poll(Duration::from_millis(5))? {
            match event::read()? {
                Event::Key(key) => {
                    app.handle_key(key);
                    should_draw = true;
                }
                Event::Resize(_w, _h) => {
                    // Force a redraw on resize
                    should_draw = true;
                }
                _ => {}
            }
        }

        // 3. Redraw only if something changed
        if should_draw {
            terminal.draw(|f| ui::draw(f, app))?;
        } else {
            // Optional: Small sleep to avoid spinning if poll() returns immediately
            // but for now 5ms duration on poll is our rate limiter.
        }
    }
    Ok(())
}
