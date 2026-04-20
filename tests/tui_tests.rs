use ratatui_image::picker::Picker;
use retrofits::app::App;
use retrofits::fits;
use std::path::PathBuf;

fn example_fits_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example_fits/18109J000.fits")
}

#[test]
fn test_app_creation() {
    let fits_image = fits::load_fits(&example_fits_path(), None).unwrap();
    // Use halfblocks picker for tests since it doesn't query the terminal
    let mut picker = Picker::halfblocks();
    let guessed = ratatui_image::picker::ProtocolType::Halfblocks;
    let app = App::new(
        std::sync::Arc::new(fits_image),
        &mut picker,
        "test.fits".to_string(),
        guessed,
        true,
    );

    assert!(app.is_ok());
    let mut app = app.unwrap();
    assert!(app.running);

    // Test cycling stretch
    use retrofits::stretch::StretchFunction;
    assert_eq!(app.stretch, StretchFunction::Asinh);
    app.stretch = StretchFunction::Linear;
    app.queue_render();
    // Shouldn't panic
}

#[test]
fn test_quit_key() {
    let fits_image = fits::load_fits(&example_fits_path(), None).unwrap();
    let mut picker = Picker::halfblocks();
    let guessed = ratatui_image::picker::ProtocolType::Halfblocks;
    let mut app = App::new(
        std::sync::Arc::new(fits_image),
        &mut picker,
        "test.fits".to_string(),
        guessed,
        true,
    )
    .unwrap();

    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let quit_event = KeyEvent {
        code: KeyCode::Char('q'),
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };

    app.handle_key(quit_event);
    assert!(!app.running);
}
