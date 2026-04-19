use ratatui::style::Color;
use crossterm::event::KeyCode;

// --- APPLICATION CONSTANTS ---
pub const APP_NAME: &str = "CHORD";
pub const DEFAULT_TICK_RATE_MS: u64 = 50;
pub const KEY_DEBOUNCE_MS: u128 = 100;

// --- THEME CONFIGURATION ---
#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub cursor_bg: Color,
    pub cursor_fg: Color,
    pub accent: Color,
    pub accent_dim: Color,
    pub critical: Color,
    pub dim: Color,
    pub status_bg: Color,
}

pub const THEME: Theme = Theme {
    bg: Color::Rgb(18, 18, 18),
    fg: Color::Rgb(204, 204, 204),
    cursor_bg: Color::Rgb(42, 42, 42),
    cursor_fg: Color::Rgb(221, 221, 221),
    accent: Color::Rgb(27, 253, 156),
    accent_dim: Color::Rgb(102, 178, 178),
    critical: Color::Rgb(186, 9, 89),
    dim: Color::Rgb(122, 122, 122),
    status_bg: Color::Rgb(42, 42, 42),
};

// --- KEYBINDINGS ---
// Global Navigation & Playback
pub const KEY_QUIT: KeyCode = KeyCode::Char('q');
pub const KEY_TOGGLE_PLAYBACK_1: KeyCode = KeyCode::Char(' ');
pub const KEY_TOGGLE_PLAYBACK_2: KeyCode = KeyCode::Char('p');
pub const KEY_NEXT_TRACK_1: KeyCode = KeyCode::Char('l');
pub const KEY_NEXT_TRACK_2: KeyCode = KeyCode::Char('L');
pub const KEY_PREV_TRACK_1: KeyCode = KeyCode::Char('h');
pub const KEY_PREV_TRACK_2: KeyCode = KeyCode::Char('H');

// Volume & Audio
pub const KEY_VOL_UP_1: KeyCode = KeyCode::Char('+');
pub const KEY_VOL_UP_2: KeyCode = KeyCode::Char('=');
pub const KEY_VOL_DOWN: KeyCode = KeyCode::Char('-');
pub const KEY_CYCLE_DEVICE: KeyCode = KeyCode::Char('d');

// Modes
pub const KEY_SEARCH_MODE: KeyCode = KeyCode::Char('/');
pub const KEY_PLAYLIST_MODE: KeyCode = KeyCode::Tab;
pub const KEY_REFRESH: KeyCode = KeyCode::Char('r');
pub const KEY_CONFIG_MODE: KeyCode = KeyCode::Char('c'); // Used with Ctrl

// List Navigation
pub const KEY_LIST_UP: KeyCode = KeyCode::Up;
pub const KEY_LIST_DOWN: KeyCode = KeyCode::Down;
pub const KEY_LIST_UP_VIM: KeyCode = KeyCode::Char('k');
pub const KEY_LIST_DOWN_VIM: KeyCode = KeyCode::Char('j');
pub const KEY_CONFIRM: KeyCode = KeyCode::Enter;
pub const KEY_BACK: KeyCode = KeyCode::Esc;
