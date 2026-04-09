use std::path::PathBuf;
use retrofits::app::App;
use retrofits::fits;
use ratatui_image::picker::Picker;

fn example_fits_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example_fits/18109J000.fits")
}

#[test]
fn test_viewport_zoom_and_pan() {
    let fits_image = fits::load_fits(&example_fits_path()).unwrap();
    let original_w = fits_image.width;
    let original_h = fits_image.height;
    
    let mut picker = Picker::halfblocks();
    let mut app = App::new(std::sync::Arc::new(fits_image), &mut picker).unwrap();
    
    // Default viewport
    let (x, y, w, h) = app.compute_viewport();
    assert_eq!(x, 0);
    assert_eq!(y, 0);
    assert_eq!(w, original_w);
    assert_eq!(h, original_h);

    use crossterm::event::{KeyEvent, KeyCode, KeyModifiers, KeyEventKind, KeyEventState};

    let zoom_in_event = KeyEvent {
        code: KeyCode::Char('+'),
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };
    app.handle_key(zoom_in_event);
    
    assert!(app.zoom > 1.0);
    let (_, _, w_zoom, h_zoom) = app.compute_viewport();
    assert!(w_zoom < original_w);
    assert!(h_zoom < original_h);

    let pan_right_event = KeyEvent {
        code: KeyCode::Right,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };
    app.handle_key(pan_right_event);
    assert!(app.offset.0 > 0);
    
    let pan_down_event = KeyEvent {
        code: KeyCode::Down,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };
    app.handle_key(pan_down_event);
    assert!(app.offset.1 > 0);

    let reset_event = KeyEvent {
        code: KeyCode::Char('r'),
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };
    app.handle_key(reset_event);
    assert_eq!(app.zoom, 1.0);
    assert_eq!(app.offset, (0, 0));
}
