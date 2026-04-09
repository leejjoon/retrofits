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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load FITS parsing
    let fits_image = fits::load_fits(&cli.file)?;

    // Initialize Ratatui terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Query terminal for image protocol capabilities
    let mut picker = Picker::from_query_stdio()?;

    // Check if the terminal is Ghostty
    let is_ghostty = std::env::var("TERM_PROGRAM").map(|v| v.to_lowercase() == "ghostty").unwrap_or(false)
        || std::env::var("TERM").map(|v| v.to_lowercase().contains("ghostty")).unwrap_or(false);

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
    } else if is_ghostty && picker.protocol_type() == ratatui_image::picker::ProtocolType::Halfblocks {
        // Ghostty supports Kitty protocol, but terminal querying sometimes fails or times out.
        picker.set_protocol_type(ratatui_image::picker::ProtocolType::Kitty);
    }


    // Create the App state
    let mut app = App::new(std::sync::Arc::new(fits_image), &mut picker)?;

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

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    // Initial render request
    app.queue_render();

    while app.running {
        app.try_update_protocol();
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(16))? { // Roughly 60fps responsiveness
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }
    }
    Ok(())
}
