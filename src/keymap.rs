//! Keybinding definitions.
//!
//! The [`KEYMAP`] table is the single source of truth for normal-mode
//! keybindings: `App::handle_normal_key` dispatches through it and the help
//! window is generated from it, so the two can never drift apart.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
    Pan { dir: PanDirection, coarse: bool },
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
    OpenHeader,
    OpenHelp,
    CycleProtocol,
    OpenProtocolPicker,
    SetProtocol(ProtocolType),
    ToggleCrosshair,
    ToggleMouse,
}

pub struct Binding {
    pub keys: &'static [KeyCode],
    /// Required modifiers. For `Char` keys, SHIFT is ignored during lookup
    /// (an uppercase char already implies it).
    pub mods: KeyModifiers,
    pub action: Action,
    pub category: &'static str,
    pub help: &'static str,
    /// Whether this action is also allowed while an info popup (e.g. the
    /// summary window) is open.
    pub global: bool,
}

pub static KEYMAP: &[Binding] = &[
    // Navigation — vim-style fine pan, Shift/uppercase for coarse pan.
    Binding {
        keys: &[KeyCode::Left, KeyCode::Char('h')],
        mods: KeyModifiers::NONE,
        action: Action::Pan {
            dir: PanDirection::Left,
            coarse: false,
        },
        category: "Navigation",
        help: "Pan left",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Right, KeyCode::Char('l')],
        mods: KeyModifiers::NONE,
        action: Action::Pan {
            dir: PanDirection::Right,
            coarse: false,
        },
        category: "Navigation",
        help: "Pan right",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Up, KeyCode::Char('k')],
        mods: KeyModifiers::NONE,
        action: Action::Pan {
            dir: PanDirection::Up,
            coarse: false,
        },
        category: "Navigation",
        help: "Pan up",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Down, KeyCode::Char('j')],
        mods: KeyModifiers::NONE,
        action: Action::Pan {
            dir: PanDirection::Down,
            coarse: false,
        },
        category: "Navigation",
        help: "Pan down",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('H')],
        mods: KeyModifiers::NONE,
        action: Action::Pan {
            dir: PanDirection::Left,
            coarse: true,
        },
        category: "Navigation",
        help: "Pan left (coarse)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Left],
        mods: KeyModifiers::SHIFT,
        action: Action::Pan {
            dir: PanDirection::Left,
            coarse: true,
        },
        category: "Navigation",
        help: "Pan left (coarse)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('L')],
        mods: KeyModifiers::NONE,
        action: Action::Pan {
            dir: PanDirection::Right,
            coarse: true,
        },
        category: "Navigation",
        help: "Pan right (coarse)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Right],
        mods: KeyModifiers::SHIFT,
        action: Action::Pan {
            dir: PanDirection::Right,
            coarse: true,
        },
        category: "Navigation",
        help: "Pan right (coarse)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('K')],
        mods: KeyModifiers::NONE,
        action: Action::Pan {
            dir: PanDirection::Up,
            coarse: true,
        },
        category: "Navigation",
        help: "Pan up (coarse)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Up],
        mods: KeyModifiers::SHIFT,
        action: Action::Pan {
            dir: PanDirection::Up,
            coarse: true,
        },
        category: "Navigation",
        help: "Pan up (coarse)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('J')],
        mods: KeyModifiers::NONE,
        action: Action::Pan {
            dir: PanDirection::Down,
            coarse: true,
        },
        category: "Navigation",
        help: "Pan down (coarse)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Down],
        mods: KeyModifiers::SHIFT,
        action: Action::Pan {
            dir: PanDirection::Down,
            coarse: true,
        },
        category: "Navigation",
        help: "Pan down (coarse)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('+'), KeyCode::Char('='), KeyCode::Char('i')],
        mods: KeyModifiers::NONE,
        action: Action::ZoomIn,
        category: "Navigation",
        help: "Zoom in",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('-'), KeyCode::Char('o')],
        mods: KeyModifiers::NONE,
        action: Action::ZoomOut,
        category: "Navigation",
        help: "Zoom out (below 1x shrinks the image)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('r')],
        mods: KeyModifiers::NONE,
        action: Action::ResetView,
        category: "Navigation",
        help: "Reset zoom and center",
        global: true,
    },
    // Image controls
    Binding {
        keys: &[KeyCode::Char('s')],
        mods: KeyModifiers::NONE,
        action: Action::CycleStretch,
        category: "Image Controls",
        help: "Cycle stretch function (Linear, Log, Asinh)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('c')],
        mods: KeyModifiers::NONE,
        action: Action::CycleColormap,
        category: "Image Controls",
        help: "Cycle colormap",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('z')],
        mods: KeyModifiers::NONE,
        action: Action::CycleCutMode,
        category: "Image Controls",
        help: "Cycle cut mode (MinMax, ZScale, Custom)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('m')],
        mods: KeyModifiers::NONE,
        action: Action::OpenManualCut,
        category: "Image Controls",
        help: "Set custom cut points (manual)",
        global: false,
    },
    Binding {
        keys: &[KeyCode::Char('x')],
        mods: KeyModifiers::NONE,
        action: Action::ToggleCrosshair,
        category: "Image Controls",
        help: "Crosshair pixel readout (move with h/j/k/l)",
        global: false,
    },
    // App controls
    Binding {
        keys: &[KeyCode::Char('e')],
        mods: KeyModifiers::NONE,
        action: Action::OpenExtensionPicker,
        category: "App Controls",
        help: "Select FITS extension",
        global: false,
    },
    Binding {
        keys: &[KeyCode::Char('v')],
        mods: KeyModifiers::NONE,
        action: Action::OpenHeader,
        category: "App Controls",
        help: "View FITS header (searchable with /)",
        global: false,
    },
    Binding {
        keys: &[KeyCode::Char('w')],
        mods: KeyModifiers::NONE,
        action: Action::OpenSummary,
        category: "App Controls",
        help: "Toggle summary window",
        global: false,
    },
    Binding {
        keys: &[KeyCode::Char('p')],
        mods: KeyModifiers::NONE,
        action: Action::CycleProtocol,
        category: "App Controls",
        help: "Cycle image protocol",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('P')],
        mods: KeyModifiers::NONE,
        action: Action::OpenProtocolPicker,
        category: "App Controls",
        help: "Select image protocol from a list",
        global: false,
    },
    Binding {
        keys: &[KeyCode::Char('?')],
        mods: KeyModifiers::NONE,
        action: Action::OpenHelp,
        category: "App Controls",
        help: "Show help",
        global: false,
    },
    Binding {
        keys: &[KeyCode::Char('M')],
        mods: KeyModifiers::NONE,
        action: Action::ToggleMouse,
        category: "App Controls",
        help: "Toggle mouse (scroll:zoom, drag:pan, click:select)",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('R')],
        mods: KeyModifiers::NONE,
        action: Action::ForceRedraw,
        category: "App Controls",
        help: "Force clear and redraw the screen",
        global: true,
    },
    Binding {
        keys: &[KeyCode::Char('q')],
        mods: KeyModifiers::NONE,
        action: Action::Quit,
        category: "App Controls",
        help: "Quit application",
        global: false,
    },
];

/// Find the binding for a key event, if any.
///
/// For `Char` keys the SHIFT modifier is stripped before comparison since an
/// uppercase character already encodes it; for other keys (arrows etc.) the
/// modifiers must match exactly, which is what allows Shift+Arrow bindings.
pub fn lookup(key: KeyEvent) -> Option<&'static Binding> {
    let ev_mods = match key.code {
        KeyCode::Char(_) => key.modifiers - KeyModifiers::SHIFT,
        _ => key.modifiers,
    };
    KEYMAP
        .iter()
        .find(|b| b.keys.contains(&key.code) && ev_mods == b.mods)
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

/// All key names for a binding, including a modifier prefix.
pub fn binding_keys(binding: &Binding) -> Vec<String> {
    let prefix = if binding.mods.contains(KeyModifiers::SHIFT) {
        "Shift+"
    } else if binding.mods.contains(KeyModifiers::CONTROL) {
        "Ctrl+"
    } else {
        ""
    };
    binding
        .keys
        .iter()
        .map(|k| format!("{}{}", prefix, key_name(k)))
        .collect()
}
