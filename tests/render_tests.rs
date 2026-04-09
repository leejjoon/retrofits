use ratatui_image::picker::Picker;
use retrofits::colormap::ColormapName;
use retrofits::fits;
use retrofits::render::{RenderRequest, RenderThread};
use retrofits::stretch::StretchFunction;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn example_fits_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example_fits/18109J000.fits")
}

#[test]
fn test_render_thread() {
    let fits_image = fits::load_fits(&example_fits_path()).unwrap();
    let fits_arc = Arc::new(fits_image);
    let picker = Picker::halfblocks();

    let render_thread = RenderThread::new(fits_arc.clone(), picker);

    let req = RenderRequest {
        stretch: StretchFunction::Asinh,
        colormap: ColormapName::Grayscale,
        black_point: 0.0,
        white_point: 100.0,
        zoom: 1.0,
        center: (fits_arc.width as f64 / 2.0, fits_arc.height as f64 / 2.0),
        term_size: (80, 24),
        protocol_type: ratatui_image::picker::ProtocolType::Halfblocks,
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
