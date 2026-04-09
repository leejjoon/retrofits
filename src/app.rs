use crate::colormap::{apply_colormap, ColormapName};
use crate::fits::FitsImage;
use crate::stretch::{auto_stretch_params, compute_stretch, StretchFunction};
use crate::render::{RenderRequest, RenderThread};
use crate::zscale::estimate_zscale;

use std::sync::Arc;
use crossterm::event::{KeyEvent, KeyCode};
use image::DynamicImage;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CutMode {
    MinMax,
    ZScale,
    Custom,
}

impl std::fmt::Display for CutMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MinMax => write!(f, "MinMax"),
            Self::ZScale => write!(f, "Z-Scale"),
            Self::Custom => write!(f, "Custom"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    EditingBlackPoint,
    EditingWhitePoint,
}

pub struct App {
    pub fits: Arc<FitsImage>,
    pub stretch: StretchFunction,
    pub colormap: ColormapName,
    pub black_point: f32,
    pub white_point: f32,
    pub zoom: f64,
    pub center: (f64, f64),
    pub term_size: (u16, u16),
    pub cut_mode: CutMode,
    pub zscale_contrast: f32,
    pub custom_black_point: f32,
    pub custom_white_point: f32,
    pub input_mode: InputMode,
    pub input_buffer: String,
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

        let mut app = Self {
            fits,
            stretch,
            colormap,
            black_point,
            white_point,
            zoom: 1.0,
            center: (0.0, 0.0), // placeholder
            term_size: (80, 24),
            cut_mode: CutMode::MinMax,
            zscale_contrast: 0.25,
            custom_black_point: black_point,
            custom_white_point: white_point,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            protocol,
            protocol_type,
            render_thread,
            running: true,
        };
        
        app.center = (app.fits.width as f64 / 2.0, app.fits.height as f64 / 2.0);
        
        // Initial apply of default cut mode (MinMax)
        app.apply_cut_mode();

        Ok(app)
    }

    pub fn apply_cut_mode(&mut self) {
        match self.cut_mode {
            CutMode::MinMax => {
                self.black_point = self.fits.min_value();
                self.white_point = self.fits.max_value();
            }
            CutMode::ZScale => {
                let (vmin, vmax) = estimate_zscale(&self.fits.data, self.zscale_contrast);
                self.black_point = vmin;
                self.white_point = vmax;
            }
            CutMode::Custom => {
                self.black_point = self.custom_black_point;
                self.white_point = self.custom_white_point;
            }
        }
        self.queue_render();
    }

    pub fn queue_render(&mut self) {
        let req = RenderRequest {
            stretch: self.stretch,
            colormap: self.colormap,
            black_point: self.black_point,
            white_point: self.white_point,
            zoom: self.zoom,
            center: self.center,
            term_size: self.term_size,
        };
        self.render_thread.request(req);
    }
    
    pub fn try_update_protocol(&mut self) -> bool {
        if let Some(new_protocol) = self.render_thread.try_recv() {
            self.protocol = new_protocol;
            true
        } else {
            false
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::EditingBlackPoint | InputMode::EditingWhitePoint => self.handle_input_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
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
            KeyCode::Char('z') => {
                self.cut_mode = match self.cut_mode {
                    CutMode::MinMax => CutMode::ZScale,
                    CutMode::ZScale => CutMode::Custom,
                    CutMode::Custom => CutMode::MinMax,
                };
                self.apply_cut_mode();
            }
            KeyCode::Char('m') => {
                self.input_mode = InputMode::EditingBlackPoint;
                self.input_buffer = self.black_point.to_string();
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
                self.center = (self.fits.width as f64 / 2.0, self.fits.height as f64 / 2.0);
                self.queue_render();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                let pan = self.fits.width as f64 / self.zoom * 0.5;
                self.center.0 -= pan.max(1.0);
                self.queue_render();
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let pan = self.fits.width as f64 / self.zoom * 0.5;
                self.center.0 += pan.max(1.0);
                self.queue_render();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let pan = self.fits.height as f64 / self.zoom * 0.5;
                self.center.1 -= pan.max(1.0);
                self.queue_render();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let pan = self.fits.height as f64 / self.zoom * 0.5;
                self.center.1 += pan.max(1.0);
                self.queue_render();
            }
            _ => {}
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if let Ok(val) = self.input_buffer.parse::<f32>() {
                    match self.input_mode {
                        InputMode::EditingBlackPoint => {
                            self.black_point = val;
                            self.custom_black_point = val;
                        }
                        InputMode::EditingWhitePoint => {
                            self.white_point = val;
                            self.custom_white_point = val;
                        }
                        _ => {}
                    }
                    self.cut_mode = CutMode::Custom; 
                    self.queue_render();
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Tab | KeyCode::Up | KeyCode::Down => {
                // Switch between fields, tentatively apply current input if valid
                if let Ok(val) = self.input_buffer.parse::<f32>() {
                    match self.input_mode {
                        InputMode::EditingBlackPoint => {
                            self.black_point = val;
                            self.custom_black_point = val;
                        }
                        InputMode::EditingWhitePoint => {
                            self.white_point = val;
                            self.custom_white_point = val;
                        }
                        _ => {}
                    }
                }
                if self.input_mode == InputMode::EditingBlackPoint {
                    self.input_mode = InputMode::EditingWhitePoint;
                    self.input_buffer = self.white_point.to_string();
                } else {
                    self.input_mode = InputMode::EditingBlackPoint;
                    self.input_buffer = self.black_point.to_string();
                }
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                if c.is_digit(10) || c == '.' || c == '-' {
                    self.input_buffer.push(c);
                }
            }
            _ => {}
        }
    }
}
