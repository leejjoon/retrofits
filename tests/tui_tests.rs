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

fn key(code: crossterm::event::KeyCode) -> crossterm::event::KeyEvent {
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    KeyEvent {
        code,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    }
}

fn test_app() -> App {
    let fits_image = fits::load_fits(&example_fits_path(), None).unwrap();
    let mut picker = Picker::halfblocks();
    App::new(
        std::sync::Arc::new(fits_image),
        &mut picker,
        "test.fits".to_string(),
        ratatui_image::picker::ProtocolType::Halfblocks,
    )
    .unwrap()
}

#[test]
fn test_popup_open_close_cycles() {
    use crossterm::event::KeyCode;
    use retrofits::app::InputMode;

    let mut app = test_app();

    // Each (open key, close key) pair must round-trip back to Normal mode
    // without quitting the app.
    for (open, close) in [
        (KeyCode::Char('?'), KeyCode::Esc),
        (KeyCode::Char('w'), KeyCode::Char('w')),
        (KeyCode::Char('e'), KeyCode::Char('q')),
        (KeyCode::Char('v'), KeyCode::Char('q')),
        (KeyCode::Char('P'), KeyCode::Char('P')),
        (KeyCode::Char('m'), KeyCode::Esc),
    ] {
        app.handle_key(key(open));
        assert!(
            !matches!(app.input_mode, InputMode::Normal),
            "{:?} should open a popup",
            open
        );
        app.handle_key(key(close));
        assert!(
            matches!(app.input_mode, InputMode::Normal),
            "{:?} should close the popup",
            close
        );
        assert!(app.running, "closing a popup must not quit the app");
    }
}

#[test]
fn test_keymap_dispatch_matches_keys() {
    use crossterm::event::KeyCode;
    use retrofits::keymap::{lookup, Action};

    // Every key documented in the keymap must resolve to its action.
    assert_eq!(lookup(key(KeyCode::Char('q'))).unwrap().action, Action::Quit);
    assert_eq!(
        lookup(key(KeyCode::Char('s'))).unwrap().action,
        Action::CycleStretch
    );
    assert!(lookup(key(KeyCode::Char('x'))).is_none());

    // Dispatch through the App: 's' cycles stretch.
    let mut app = test_app();
    let before = app.stretch;
    app.handle_key(key(KeyCode::Char('s')));
    assert_ne!(app.stretch, before);
}

#[test]
fn test_global_keys_work_in_summary() {
    use crossterm::event::KeyCode;
    use retrofits::app::InputMode;

    let mut app = test_app();
    app.handle_key(key(KeyCode::Char('w')));
    assert!(matches!(app.input_mode, InputMode::Summary));

    // Global action (zoom) works while summary is open...
    let before = app.zoom;
    app.handle_key(key(KeyCode::Char('+')));
    assert!(app.zoom > before);
    assert!(matches!(app.input_mode, InputMode::Summary));

    // ...but non-global actions (open help) are ignored.
    app.handle_key(key(KeyCode::Char('h')));
    assert!(matches!(app.input_mode, InputMode::Summary));
}

#[test]
fn test_extension_picker_navigation() {
    use crossterm::event::KeyCode;
    use retrofits::app::InputMode;

    let mut app = test_app();
    let ext_count = app.fits.extensions.len();
    app.handle_key(key(KeyCode::Char('e')));

    // Navigation clamps to the list bounds.
    for _ in 0..ext_count + 5 {
        app.handle_key(key(KeyCode::Down));
    }
    if let InputMode::SelectingExtension { state } = &app.input_mode {
        assert_eq!(state.selected(), Some(ext_count - 1));
    } else {
        panic!("expected extension picker mode");
    }
    for _ in 0..ext_count + 5 {
        app.handle_key(key(KeyCode::Up));
    }
    if let InputMode::SelectingExtension { state } = &app.input_mode {
        assert_eq!(state.selected(), Some(0));
    } else {
        panic!("expected extension picker mode");
    }

    // Enter loads (or no-ops on non-image) and returns to Normal.
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.input_mode, InputMode::Normal));
    assert!(app.running);
}

#[test]
fn test_vim_keys_and_esc_behavior() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use retrofits::app::InputMode;

    let mut app = test_app();

    // Esc must NOT quit from normal mode; q must.
    app.handle_key(key(KeyCode::Esc));
    assert!(app.running, "Esc must not quit");
    assert!(matches!(app.input_mode, InputMode::Normal));

    // h pans left (fine), H pans left coarse (4x the fine step).
    let start = app.center.0;
    app.handle_key(key(KeyCode::Char('h')));
    let fine = start - app.center.0;
    assert!(fine > 0.0, "h must pan left");
    let mid = app.center.0;
    let mut shift_h = key(KeyCode::Char('H'));
    shift_h.modifiers = KeyModifiers::SHIFT;
    app.handle_key(shift_h);
    let coarse = mid - app.center.0;
    assert!((coarse - 4.0 * fine).abs() < 1e-6, "H must pan 4x farther");

    // Shift+Left is the same coarse pan.
    let before = app.center.0;
    let mut shift_left = key(KeyCode::Left);
    shift_left.modifiers = KeyModifiers::SHIFT;
    app.handle_key(shift_left);
    assert!((before - app.center.0 - coarse).abs() < 1e-6);
}

#[test]
fn test_zoom_out_below_one() {
    use crossterm::event::KeyCode;

    let mut app = test_app();
    assert_eq!(app.zoom, 1.0);
    app.handle_key(key(KeyCode::Char('-')));
    assert!(app.zoom < 1.0, "zoom-out below fit must be allowed");
    // Repeated zoom-out clamps at MIN_ZOOM.
    for _ in 0..20 {
        app.handle_key(key(KeyCode::Char('-')));
    }
    assert_eq!(app.zoom, retrofits::app::MIN_ZOOM);
    // Reset restores 1.0.
    app.handle_key(key(KeyCode::Char('r')));
    assert_eq!(app.zoom, 1.0);
}

#[test]
fn test_protocol_picker() {
    use crossterm::event::KeyCode;
    use ratatui_image::picker::ProtocolType;
    use retrofits::app::InputMode;

    let mut app = test_app();
    app.handle_key(key(KeyCode::Char('P')));
    assert!(matches!(app.input_mode, InputMode::SelectingProtocol { .. }));
    // Move from Halfblocks (0) to Sixel (1) and apply.
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.input_mode, InputMode::Normal));
    assert_eq!(app.protocol_type, ProtocolType::Sixel);
}
