//! Keybinding definitions.
//!
//! The [`KEYMAP`] table is the single source of truth for normal-mode
//! keybindings: `App::handle_normal_key` dispatches through it and the help
//! window is generated from it, so the two can never drift apart.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui_image::picker::ProtocolType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanDirection {
    Left,
    Right,
    Up,
    Down,
}

/// A user-facing action, decoupled from the key that triggers it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Quit,
    Pan(PanDirection),
    ZoomIn,
    ZoomOut,
    ResetView,
    ForceRedraw,
    CycleStretch,
    CycleColormap,
    CycleCutMode,
    OpenManualCut,
    OpenSummary,
    OpenExtensionPicker,
    OpenHelp,
    CycleProtocol,
    SetProtocol(ProtocolType),
}

pub struct Binding {
    pub keys: &'static [KeyCode],
    pub action: Action,
    pub category: &'static str,
    pub help: &'static str,
    /// Whether this action is also allowed while an info popup (e.g. the
    /// summary window) is open.
    pub global: bool,
}

pub static KEYMAP: &[Binding] = &[
    // Navigation
    Binding {
        keys: &[KeyCode::Left],
        action: Action::Pan(PanDirection::Left),
        category: "Navigation",
        help: "Pan left",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Right, KeyCode::Char('l')],
        action: Action::Pan(PanDirection::Right),
        category: "Navigation",
        help: "Pan right",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Up, KeyCode::Char('k')],
        action: Action::Pan(PanDirection::Up),
        category: "Navigation",
        help: "Pan up",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Down, KeyCode::Char('j')],
        action: Action::Pan(PanDirection::Down),
        category: "Navigation",
        help: "Pan down",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('+'), KeyCode::Char('i')],
        action: Action::ZoomIn,
        category: "Navigation",
        help: "Zoom in",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('-'), KeyCode::Char('o')],
        action: Action::ZoomOut,
        category: "Navigation",
        help: "Zoom out",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('r')],
        action: Action::ResetView,
        category: "Navigation",
        help: "Reset zoom and center",
        global: true,
    },
    // Image controls
    Binding {
        keys: &[KeyCode::Char('s')],
        action: Action::CycleStretch,
        category: "Image Controls",
        help: "Cycle stretch function (Linear, Log, Asinh)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('c')],
        action: Action::CycleColormap,
        category: "Image Controls",
        help: "Cycle colormap",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('z')],
        action: Action::CycleCutMode,
        category: "Image Controls",
        help: "Cycle cut mode (MinMax, ZScale, Custom)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('m')],
        action: Action::OpenManualCut,
        category: "Image Controls",
        help: "Set custom cut points (manual)",
        global: false,
    },
    // App controls
    Binding {
        keys: &[KeyCode::Char('e')],
        action: Action::OpenExtensionPicker,
        category: "App Controls",
        help: "Select FITS extension",
        global: false,
    },
    Binding {
        keys: &[KeyCode::Char('p')],
        action: Action::CycleProtocol,
        category: "App Controls",
        help: "Cycle image protocol (Halfblocks, Sixel, Kitty, iTerm2)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('H')],
        action: Action::SetProtocol(ProtocolType::Halfblocks),
        category: "App Controls",
        help: "Switch to Halfblocks protocol",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('S')],
        action: Action::SetProtocol(ProtocolType::Sixel),
        category: "App Controls",
        help: "Switch to Sixel protocol",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('K')],
        action: Action::SetProtocol(ProtocolType::Kitty),
        category: "App Controls",
        help: "Switch to Kitty protocol",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('I')],
        action: Action::SetProtocol(ProtocolType::Iterm2),
        category: "App Controls",
        help: "Switch to iTerm2 protocol",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('w')],
        action: Action::OpenSummary,
        category: "App Controls",
        help: "Toggle summary window",
        global: false,
    },
    Binding {
        keys: &[KeyCode::Char('h')],
        action: Action::OpenHelp,
        category: "App Controls",
        help: "Toggle help window",
        global: false,
    },
    Binding {
        keys: &[KeyCode::Char('R')],
        action: Action::ForceRedraw,
        category: "App Controls",
        help: "Force clear and redraw the screen",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('q'), KeyCode::Esc],
        action: Action::Quit,
        category: "App Controls",
        help: "Quit application / Close popups",
        global: false,
    },
];

/// Find the binding for a key event, if any.
pub fn lookup(key: KeyEvent) -> Option<&'static Binding> {
    KEYMAP.iter().find(|b| b.keys.contains(&key.code))
}

/// Human-readable name for a key, used in the generated help window.
pub fn key_name(key: &KeyCode) -> String {
    match key {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        other => format!("{:?}", other),
    }
}
