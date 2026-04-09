use crate::colormap::{apply_colormap, ColormapName};
use crate::fits::FitsImage;
use crate::stretch::{auto_stretch_params, compute_stretch, StretchFunction};
use crate::render::{RenderRequest, RenderThread};

use std::sync::Arc;
use crossterm::event::KeyEvent;
use image::DynamicImage;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

pub struct App {
    pub fits: Arc<FitsImage>,
    pub stretch: StretchFunction,
    pub colormap: ColormapName,
    pub black_point: f32,
    pub white_point: f32,
    pub zoom: f64,
    pub offset: (i32, i32),
    pub term_size: (u16, u16),
    pub protocol: StatefulProtocol,
    pub protocol_type: ProtocolType,
    pub render_thread: RenderThread,
    pub running: bool,
}

impl App {
    pub fn new(fits: Arc<FitsImage>, picker: &mut Picker) -> anyhow::Result<Self> {
        let (black_point, white_point) = auto_stretch_params(fits.data.view());
        let stretch = StretchFunction::Asinh;
        let colormap = ColormapName::Grayscale;

        let stretched = compute_stretch(fits.data.view(), stretch, black_point, white_point);
        let rgba = apply_colormap(stretched.view(), colormap);
        
        let dyn_img = DynamicImage::ImageRgba8(rgba);
        let protocol = picker.new_resize_protocol(dyn_img);
        let protocol_type = picker.protocol_type();
        
        let render_thread = RenderThread::new(fits.clone(), picker.clone());

        Ok(Self {
            fits,
            stretch,
            colormap,
            black_point,
            white_point,
            zoom: 1.0,
            offset: (0, 0),
            term_size: (80, 24),
            protocol,
            protocol_type,
            render_thread,
            running: true,
        })
    }

    pub fn queue_render(&mut self) {
        let req = RenderRequest {
            stretch: self.stretch,
            colormap: self.colormap,
            black_point: self.black_point,
            white_point: self.white_point,
            zoom: self.zoom,
            offset: self.offset,
            term_size: self.term_size,
        };
        self.render_thread.request(req);
    }
    
    pub fn try_update_protocol(&mut self) {
        if let Some(new_protocol) = self.render_thread.try_recv() {
            self.protocol = new_protocol;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;
        
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.running = false;
            }
            KeyCode::Char('s') => {
                self.stretch = match self.stretch {
                    StretchFunction::Linear => StretchFunction::Logarithmic,
                    StretchFunction::Logarithmic => StretchFunction::Asinh,
                    StretchFunction::Asinh => StretchFunction::Linear,
                };
                self.queue_render();
            }
            KeyCode::Char('c') => {
                self.colormap = self.colormap.cycle();
                self.queue_render();
            }
            KeyCode::Char('+') | KeyCode::Char('i') => {
                self.zoom *= 1.5;
                self.queue_render();
            }
            KeyCode::Char('-') | KeyCode::Char('o') => {
                self.zoom /= 1.5;
                if self.zoom < 1.0 { self.zoom = 1.0; }
                self.queue_render();
            }
            KeyCode::Char('r') => {
                self.zoom = 1.0;
                self.offset = (0, 0);
                self.queue_render();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                let pan = (self.fits.width as f64 / self.zoom * 0.1) as i32;
                self.offset.0 -= pan.max(1);
                self.queue_render();
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let pan = (self.fits.width as f64 / self.zoom * 0.1) as i32;
                self.offset.0 += pan.max(1);
                self.queue_render();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let pan = (self.fits.height as f64 / self.zoom * 0.1) as i32;
                self.offset.1 -= pan.max(1);
                self.queue_render();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let pan = (self.fits.height as f64 / self.zoom * 0.1) as i32;
                self.offset.1 += pan.max(1);
                self.queue_render();
            }
            _ => {}
        }
    }
}
