use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use retrofits::fits;
use retrofits::render::{RenderRequest, RenderThread};
use retrofits::colormap::ColormapName;
use retrofits::stretch::StretchFunction;
use ratatui_image::picker::Picker;

fn example_fits_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example_fits/18109J000.fits")
}

#[test]
fn test_render_thread() {
    let fits_image = fits::load_fits(&example_fits_path()).unwrap();
    let fits_arc = Arc::new(fits_image);
    let picker = Picker::halfblocks();

    let render_thread = RenderThread::new(fits_arc, picker);

    let req = RenderRequest {
        stretch: StretchFunction::Asinh,
        colormap: ColormapName::Grayscale,
        black_point: 0.0,
        white_point: 100.0,
        zoom: 1.0,
        offset: (0, 0),
        term_size: (80, 24),
    };

    render_thread.request(req);

    // Wait for response
    let mut received = false;
    for _ in 0..50 {
        if let Some(_) = render_thread.try_recv() {
            received = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(received, "Render thread did not return a response in time");
}
