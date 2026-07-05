use crate::colormap::{apply_colormap, ColormapName};
use crate::fits::FitsImage;
use crate::keymap::{self, Action, PanDirection};
use crate::render::{RenderRequest, RenderThread};
use crate::stretch::{auto_stretch_params, compute_stretch, StretchFunction};
use crate::zscale::estimate_zscale;

use crossterm::event::{KeyCode, KeyEvent};
use image::DynamicImage;
use ratatui::widgets::ListState;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use std::sync::Arc;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// A transient, vim-style status-line message.
#[derive(Debug)]
pub struct StatusMessage {
    pub text: String,
    pub severity: Severity,
    pub expires: std::time::Instant,
}

/// Human-readable protocol name, shared by the status bar and summary.
pub fn protocol_name(p: ProtocolType) -> &'static str {
    match p {
        ProtocolType::Halfblocks => "Halfblocks",
        ProtocolType::Sixel => "Sixel",
        ProtocolType::Kitty => "Kitty",
        ProtocolType::Iterm2 => "iTerm2",
    }
}

#[derive(Debug)]
pub enum InputMode {
    Normal,
    EditingBlackPoint,
    EditingWhitePoint,
    Summary,
    Help { scroll: u16 },
    SelectingExtension { state: ListState },
}

pub struct App {
    pub fits: Arc<FitsImage>,
    pub filename: String,
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
    pub guessed_protocol: ProtocolType,
    pub render_thread: RenderThread,
    pub running: bool,
    pub clear_screen_next_frame: bool,
    pub sixel_clear_workaround: bool,
    pub message: Option<StatusMessage>,
}

impl App {
    pub fn new(
        fits: Arc<FitsImage>,
        picker: &mut Picker,
        filename: String,
        guessed_protocol: ProtocolType,
        sixel_clear_workaround: bool,
    ) -> anyhow::Result<Self> {
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
            filename,
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
            guessed_protocol,
            render_thread,
            running: true,
            clear_screen_next_frame: false,
            sixel_clear_workaround,
            message: None,
        };

        app.center = (app.fits.width as f64 / 2.0, app.fits.height as f64 / 2.0);

        // Initial apply of default cut mode (MinMax)
        app.apply_cut_mode();

        Ok(app)
    }

    /// Recompute black/white points from the current cut mode without
    /// queueing a render.
    pub fn compute_cuts(&mut self) {
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
    }

    pub fn apply_cut_mode(&mut self) {
        self.compute_cuts();
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
            protocol_type: self.protocol_type,
            new_fits: None,
        };
        self.render_thread.request(req);
    }

    pub fn queue_render_with_fits(&mut self) {
        let req = RenderRequest {
            stretch: self.stretch,
            colormap: self.colormap,
            black_point: self.black_point,
            white_point: self.white_point,
            zoom: self.zoom,
            center: self.center,
            term_size: self.term_size,
            protocol_type: self.protocol_type,
            new_fits: Some(self.fits.clone()),
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

    /// Show a transient status-line message (vim-style).
    pub fn notify(&mut self, severity: Severity, text: impl Into<String>) {
        let duration = match severity {
            Severity::Info => std::time::Duration::from_millis(2500),
            Severity::Warn | Severity::Error => std::time::Duration::from_secs(5),
        };
        self.message = Some(StatusMessage {
            text: text.into(),
            severity,
            expires: std::time::Instant::now() + duration,
        });
    }

    /// Expire the current status message if its time is up. Returns `true`
    /// if the message was cleared and a redraw is needed.
    pub fn tick_message(&mut self) -> bool {
        if self
            .message
            .as_ref()
            .is_some_and(|m| std::time::Instant::now() >= m.expires)
        {
            self.message = None;
            true
        } else {
            false
        }
    }

    /// Leave the current popup/mode and return to [`InputMode::Normal`].
    ///
    /// Centralizes the redraw handling every popup close needs: ratatui-image
    /// must repaint the cells the popup blanked, so a render is queued; under
    /// Sixel the encoded image may be cached as already-drawn, so the screen
    /// is additionally cleared (see DEVELOP.md).
    pub fn close_mode(&mut self) {
        self.input_mode = InputMode::Normal;
        if self.protocol_type == ProtocolType::Sixel && self.sixel_clear_workaround {
            self.clear_screen_next_frame = true;
        }
        self.queue_render();
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::EditingBlackPoint | InputMode::EditingWhitePoint => {
                self.handle_input_key(key)
            }
            InputMode::Summary => self.handle_summary_key(key),
            InputMode::Help { .. } => self.handle_help_key(key),
            InputMode::SelectingExtension { .. } => self.handle_extension_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        if let Some(binding) = keymap::lookup(key) {
            self.dispatch(binding.action);
        }
    }

    /// Execute a normal-mode action from the keymap.
    pub fn dispatch(&mut self, action: Action) {
        match action {
            Action::Quit => {
                self.running = false;
            }
            Action::CycleProtocol => {
                self.protocol_type = match self.protocol_type {
                    ProtocolType::Halfblocks => ProtocolType::Sixel,
                    ProtocolType::Sixel => ProtocolType::Kitty,
                    ProtocolType::Kitty => ProtocolType::Iterm2,
                    ProtocolType::Iterm2 => ProtocolType::Halfblocks,
                };
                self.notify(
                    Severity::Info,
                    format!("Protocol: {}", protocol_name(self.protocol_type)),
                );
                self.queue_render();
            }
            Action::SetProtocol(p) => {
                self.protocol_type = p;
                self.notify(Severity::Info, format!("Protocol: {}", protocol_name(p)));
                self.queue_render();
            }
            Action::CycleStretch => {
                self.stretch = match self.stretch {
                    StretchFunction::Linear => StretchFunction::Logarithmic,
                    StretchFunction::Logarithmic => StretchFunction::Asinh,
                    StretchFunction::Asinh => StretchFunction::Linear,
                };
                self.queue_render();
            }
            Action::CycleColormap => {
                self.colormap = self.colormap.cycle();
                self.queue_render();
            }
            Action::CycleCutMode => {
                self.cut_mode = match self.cut_mode {
                    CutMode::MinMax => CutMode::ZScale,
                    CutMode::ZScale => CutMode::Custom,
                    CutMode::Custom => CutMode::MinMax,
                };
                self.apply_cut_mode();
            }
            Action::OpenSummary => {
                self.input_mode = InputMode::Summary;
            }
            Action::OpenExtensionPicker => {
                self.input_mode = InputMode::SelectingExtension {
                    state: ListState::default().with_selected(Some(self.fits.current_extension)),
                };
            }
            Action::OpenManualCut => {
                self.input_mode = InputMode::EditingBlackPoint;
                self.input_buffer = self.black_point.to_string();
            }
            Action::OpenHelp => {
                self.input_mode = InputMode::Help { scroll: 0 };
            }
            Action::ZoomIn => {
                self.zoom *= 1.5;
                self.queue_render();
            }
            Action::ZoomOut => {
                self.zoom /= 1.5;
                if self.zoom < 1.0 {
                    self.zoom = 1.0;
                    self.notify(Severity::Info, "Already at minimum zoom (fit)");
                }
                self.queue_render();
            }
            Action::ResetView => {
                self.zoom = 1.0;
                self.center = (self.fits.width as f64 / 2.0, self.fits.height as f64 / 2.0);
                self.queue_render();
            }
            Action::ForceRedraw => {
                self.clear_screen_next_frame = true;
                self.queue_render();
            }
            Action::Pan(dir) => {
                match dir {
                    PanDirection::Left => {
                        let pan = self.fits.width as f64 / self.zoom * 0.5;
                        self.center.0 -= pan.max(1.0);
                    }
                    PanDirection::Right => {
                        let pan = self.fits.width as f64 / self.zoom * 0.5;
                        self.center.0 += pan.max(1.0);
                    }
                    PanDirection::Up => {
                        let pan = self.fits.height as f64 / self.zoom * 0.5;
                        self.center.1 -= pan.max(1.0);
                    }
                    PanDirection::Down => {
                        let pan = self.fits.height as f64 / self.zoom * 0.5;
                        self.center.1 += pan.max(1.0);
                    }
                }
                self.queue_render();
            }
        }
    }

    fn handle_extension_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('e') => {
                self.close_mode();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let InputMode::SelectingExtension { state } = &mut self.input_mode {
                    let i = state.selected().unwrap_or(0);
                    state.select(Some(i.saturating_sub(1)));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let last = self.fits.extensions.len().saturating_sub(1);
                if let InputMode::SelectingExtension { state } = &mut self.input_mode {
                    let i = state.selected().unwrap_or(0);
                    state.select(Some((i + 1).min(last)));
                }
            }
            KeyCode::Enter => {
                let selected = match &self.input_mode {
                    InputMode::SelectingExtension { state } => state.selected().unwrap_or(0),
                    _ => return,
                };
                let ext_info = self.fits.extensions.get(selected).cloned();
                if let Some(ext_info) = ext_info {
                    if ext_info.is_image {
                        if let Ok(new_fits) = crate::fits::load_fits(
                            &self.fits.file_path,
                            Some(&ext_info.index.to_string()),
                        ) {
                            self.fits = Arc::new(new_fits);
                            self.zoom = 1.0;
                            self.center =
                                (self.fits.width as f64 / 2.0, self.fits.height as f64 / 2.0);
                            self.compute_cuts();
                            self.queue_render_with_fits();
                        }
                    }
                }
                self.close_mode();
            }
            _ => {}
        }
    }

    fn handle_summary_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('w') => {
                self.close_mode();
            }
            // Allow image adjustments while the summary window is open.
            _ => {
                if let Some(binding) = keymap::lookup(key) {
                    if binding.global {
                        self.dispatch(binding.action);
                    }
                }
            }
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => {
                self.close_mode();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let InputMode::Help { scroll } = &mut self.input_mode {
                    *scroll = scroll.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let InputMode::Help { scroll } = &mut self.input_mode {
                    *scroll = scroll.saturating_add(1);
                }
            }
            _ => {}
        }
    }

    /// Apply the current input buffer to whichever cut point is being edited.
    fn apply_input_buffer(&mut self) -> bool {
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
            true
        } else {
            false
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if self.apply_input_buffer() {
                    self.cut_mode = CutMode::Custom;
                    self.queue_render();
                } else {
                    self.notify(
                        Severity::Error,
                        format!("Invalid number: '{}'", self.input_buffer),
                    );
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.close_mode();
            }
            KeyCode::Tab | KeyCode::Up | KeyCode::Down => {
                // Switch between fields, tentatively apply current input if valid
                self.apply_input_buffer();
                if matches!(self.input_mode, InputMode::EditingBlackPoint) {
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
                if c.is_ascii_digit() || c == '.' || c == '-' {
                    self.input_buffer.push(c);
                }
            }
            _ => {}
        }
    }
}
