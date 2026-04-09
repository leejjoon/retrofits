use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use image::DynamicImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

use crate::colormap::{apply_colormap, ColormapName};
use crate::fits::FitsImage;
use crate::stretch::{compute_stretch, StretchFunction};

/// A request to the render thread to process a new frame.
#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub stretch: StretchFunction,
    pub colormap: ColormapName,
    pub black_point: f32,
    pub white_point: f32,
    pub zoom: f64,
    pub center: (f64, f64),
    pub term_size: (u16, u16),
}

/// The response from the render thread containing the processed protocol state.
pub enum RenderResponse {
    Done(StatefulProtocol),
}

pub struct RenderThread {
    tx: Sender<RenderRequest>,
    rx: Receiver<RenderResponse>,
    handle: Option<JoinHandle<()>>,
}

impl RenderThread {
    pub fn new(fits: Arc<FitsImage>, mut picker: Picker) -> Self {
        let (req_tx, req_rx) = mpsc::channel::<RenderRequest>();
        let (res_tx, res_rx) = mpsc::channel::<RenderResponse>();

        let handle = thread::spawn(move || {
            // Keep the latest request
            while let Ok(request) = req_rx.recv() {
                // Drain any additional pending requests to only process the latest one
                // This debounces rapid keyboard inputs.
                let mut latest = request;
                while let Ok(next) = req_rx.try_recv() {
                    latest = next;
                }

                let protocol = process_frame(&fits, &mut picker, latest);
                if res_tx.send(RenderResponse::Done(protocol)).is_err() {
                    break; // main thread hung up
                }
            }
        });

        Self {
            tx: req_tx,
            rx: res_rx,
            handle: Some(handle),
        }
    }

    /// Sends a request to process a new frame.
    pub fn request(&self, request: RenderRequest) {
        let _ = self.tx.send(request);
    }

    /// Tries to receive the latest processed frame if available.
    pub fn try_recv(&self) -> Option<StatefulProtocol> {
        let mut latest = None;
        while let Ok(response) = self.rx.try_recv() {
            match response {
                RenderResponse::Done(protocol) => latest = Some(protocol),
            }
        }
        latest
    }
}

fn process_frame(
    fits: &FitsImage,
    picker: &mut Picker,
    req: RenderRequest,
) -> StatefulProtocol {
    // 1. Compute viewport based on terminal layout
    let (img_w, img_h) = (fits.width as f64, fits.height as f64);
    let (font_w, font_h) = picker.font_size();
    
    // Fallback if font size is 0
    let font_w = if font_w > 0 { font_w as f64 } else { 10.0 };
    let font_h = if font_h > 0 { font_h as f64 } else { 20.0 };

    let term_phys_w = (req.term_size.0 as f64 * font_w).max(1.0);
    let term_phys_h = (req.term_size.1 as f64 * font_h).max(1.0);

    let scale_to_fit = (term_phys_w / img_w).min(term_phys_h / img_h);
    let scale_factor = scale_to_fit * req.zoom;

    // Determine rect size in FITS original pixels
    let crop_w = ((term_phys_w / scale_factor).min(img_w) as usize).max(1);
    let crop_h = ((term_phys_h / scale_factor).min(img_h) as usize).max(1);

    // We want the viewport centered on `req.center`.
    let start_x = req.center.0 - (crop_w as f64 / 2.0);
    let start_y = req.center.1 - (crop_h as f64 / 2.0);

    let max_x = fits.width.saturating_sub(crop_w) as f64;
    let max_y = fits.height.saturating_sub(crop_h) as f64;
    
    // Clamp start coordinates safely inside image bounds
    let start_x = start_x.clamp(0.0, max_x.max(0.0));
    let start_y = start_y.clamp(0.0, max_y.max(0.0));

    let x = start_x.round() as usize;
    let y = start_y.round() as usize;

    let x_end = (x + crop_w).min(fits.width);
    let y_end = (y + crop_h).min(fits.height);

    // 2. Extract viewport
    use ndarray::s;
    let viewport_data = fits.data.slice(s![y..y_end, x..x_end]);

    // 3. Stretch & Colormap
    let stretched = compute_stretch(viewport_data, req.stretch, req.black_point, req.white_point);
    let rgba = apply_colormap(stretched.view(), req.colormap);
    let dyn_img = DynamicImage::ImageRgba8(rgba);

    // 4. Encode (time-consuming part blocking the ratatui-image Picker)
    picker.new_resize_protocol(dyn_img)
}
