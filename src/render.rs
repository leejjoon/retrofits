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
    pub protocol_type: ratatui_image::picker::ProtocolType,
    pub new_fits: Option<Arc<FitsImage>>,
    /// Crosshair position in image pixel coordinates (row-flipped screen
    /// orientation). Drawn into the rendered RGBA so it works identically
    /// on every graphics protocol without overlaying terminal cells.
    pub crosshair: Option<(usize, usize)>,
}

/// The visible part of the image in image pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Compute the visible crop of an `img_w` x `img_h` image for the given
/// zoom/center and terminal geometry. Shared by the render thread and by
/// crosshair/mouse hit-testing so the two can never disagree.
pub fn viewport(
    img_w: usize,
    img_h: usize,
    zoom: f64,
    center: (f64, f64),
    term_size: (u16, u16),
    font_size: (u16, u16),
) -> ViewRect {
    let (img_w_f, img_h_f) = (img_w as f64, img_h as f64);

    // Fallback if font size is 0
    let font_w = if font_size.0 > 0 {
        font_size.0 as f64
    } else {
        10.0
    };
    let font_h = if font_size.1 > 0 {
        font_size.1 as f64
    } else {
        20.0
    };

    let term_phys_w = (term_size.0 as f64 * font_w).max(1.0);
    let term_phys_h = (term_size.1 as f64 * font_h).max(1.0);

    let scale_to_fit = (term_phys_w / img_w_f).min(term_phys_h / img_h_f);
    let scale_factor = scale_to_fit * zoom;

    // Determine rect size in FITS original pixels
    let crop_w = ((term_phys_w / scale_factor).min(img_w_f) as usize).max(1);
    let crop_h = ((term_phys_h / scale_factor).min(img_h_f) as usize).max(1);

    // We want the viewport centered on `center`.
    let start_x = center.0 - (crop_w as f64 / 2.0);
    let start_y = center.1 - (crop_h as f64 / 2.0);

    let max_x = img_w.saturating_sub(crop_w) as f64;
    let max_y = img_h.saturating_sub(crop_h) as f64;

    // Clamp start coordinates safely inside image bounds
    let start_x = start_x.clamp(0.0, max_x.max(0.0));
    let start_y = start_y.clamp(0.0, max_y.max(0.0));

    let x = start_x.round() as usize;
    let y = start_y.round() as usize;

    ViewRect {
        x,
        y,
        width: (x + crop_w).min(img_w) - x,
        height: (y + crop_h).min(img_h) - y,
    }
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
            let mut current_fits = fits;
            // Keep the latest request
            while let Ok(request) = req_rx.recv() {
                // Drain any additional pending requests to only process the latest one
                // This debounces rapid keyboard inputs. A fits swap carried by a
                // drained (skipped) request must still be applied, otherwise a
                // later fits-less request would render the stale image.
                let mut latest = request;
                let mut pending_fits = latest.new_fits.take();
                while let Ok(next) = req_rx.try_recv() {
                    latest = next;
                    if let Some(fits) = latest.new_fits.take() {
                        pending_fits = Some(fits);
                    }
                }

                if let Some(new_fits) = pending_fits {
                    current_fits = new_fits;
                }

                let protocol = process_frame(&current_fits, &mut picker, latest);
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

fn process_frame(fits: &FitsImage, picker: &mut Picker, req: RenderRequest) -> StatefulProtocol {
    // Update the picker's protocol type based on the request
    picker.set_protocol_type(req.protocol_type);

    // 1. Compute viewport based on terminal layout
    let vr = viewport(
        fits.width,
        fits.height,
        req.zoom,
        req.center,
        req.term_size,
        picker.font_size(),
    );

    // 2. Extract viewport
    use ndarray::s;
    let viewport_data = fits
        .data
        .slice(s![vr.y..vr.y + vr.height, vr.x..vr.x + vr.width]);

    // 3. Stretch & Colormap
    let stretched = compute_stretch(viewport_data, req.stretch, req.black_point, req.white_point);
    let mut rgba = apply_colormap(stretched.view(), req.colormap);

    // 3b. Composite the crosshair into the RGBA (protocol-agnostic overlay).
    if let Some((cx, cy)) = req.crosshair {
        draw_crosshair(
            &mut rgba,
            &vr,
            cx,
            cy,
            req.term_size,
            picker.font_size(),
            req.protocol_type,
        );
    }
    let dyn_img = DynamicImage::ImageRgba8(rgba);

    // 4. Encode (time-consuming part blocking the ratatui-image Picker)
    picker.new_resize_protocol(dyn_img)
}

/// Draw crosshair lines through image pixel (cx, cy) into the cropped RGBA.
///
/// Thickness is computed per axis so the line survives the downscale to the
/// terminal: pixel protocols aim for ~2 screen pixels; Halfblocks has only
/// 1x2 samples per character cell, so the line must cover about one sample
/// (a full column / half a row of a cell) to remain visible.
#[allow(clippy::too_many_arguments)]
fn draw_crosshair(
    rgba: &mut image::RgbaImage,
    vr: &ViewRect,
    cx: usize,
    cy: usize,
    term_size: (u16, u16),
    font_size: (u16, u16),
    protocol_type: ratatui_image::picker::ProtocolType,
) {
    if cx < vr.x || cx >= vr.x + vr.width || cy < vr.y || cy >= vr.y + vr.height {
        return;
    }
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);
    let cols = term_size.0.max(1) as f64;
    let rows = term_size.1.max(1) as f64;

    let (thick_x, thick_y) = if protocol_type == ratatui_image::picker::ProtocolType::Halfblocks {
        // One terminal sample: a cell column horizontally, half a cell row
        // vertically.
        ((w as f64 / cols).ceil(), (h as f64 / (rows * 2.0)).ceil())
    } else {
        // ~2 screen pixels at the displayed scale.
        let font_w = if font_size.0 > 0 { font_size.0 } else { 10 } as f64;
        let font_h = if font_size.1 > 0 { font_size.1 } else { 20 } as f64;
        let fit = ((cols * font_w) / w as f64).min((rows * font_h) / h as f64);
        let t = (2.0 / fit.min(1.0)).ceil();
        (t, t)
    };
    let thick_x = (thick_x as usize).max(1);
    let thick_y = (thick_y as usize).max(1);

    let color = image::Rgba([255u8, 80, 80, 255]);
    let lx = cx - vr.x;
    let ly = cy - vr.y;
    // Vertical line
    for x in lx.saturating_sub(thick_x / 2)..(lx + thick_x.div_ceil(2)).min(w) {
        for y in 0..h {
            rgba.put_pixel(x as u32, y as u32, color);
        }
    }
    // Horizontal line
    for y in ly.saturating_sub(thick_y / 2)..(ly + thick_y.div_ceil(2)).min(h) {
        for x in 0..w {
            rgba.put_pixel(x as u32, y as u32, color);
        }
    }
}
