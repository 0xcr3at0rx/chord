use crossterm::event::KeyCode;
use ratatui::style::Color;

// --- APPLICATION CONSTANTS ---
pub const DEFAULT_TICK_RATE_MS: u64 = 50;
pub const KEY_DEBOUNCE_MS: u128 = 100;

// --- THEME ---
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

// --- KEYBINDINGS ---
pub const KEY_QUIT: KeyCode = KeyCode::Char('q');
pub const KEY_TOGGLE_PLAYBACK_1: KeyCode = KeyCode::Char(' ');
pub const KEY_TOGGLE_PLAYBACK_2: KeyCode = KeyCode::Char('p');
pub const KEY_NEXT_TRACK_1: KeyCode = KeyCode::Char('l');
pub const KEY_NEXT_TRACK_2: KeyCode = KeyCode::Char('L');
pub const KEY_PREV_TRACK_1: KeyCode = KeyCode::Char('h');
pub const KEY_PREV_TRACK_2: KeyCode = KeyCode::Char('H');

pub const KEY_VOL_UP_1: KeyCode = KeyCode::Char('+');
pub const KEY_VOL_UP_2: KeyCode = KeyCode::Char('=');
pub const KEY_VOL_DOWN: KeyCode = KeyCode::Char('-');

pub const KEY_SEARCH_MODE: KeyCode = KeyCode::Char('/');
pub const KEY_PLAYLIST_MODE: KeyCode = KeyCode::Tab;
pub const KEY_RADIO_MODE: KeyCode = KeyCode::Char('r'); // Used with Ctrl

pub const KEY_LIST_UP: KeyCode = KeyCode::Up;
pub const KEY_LIST_DOWN: KeyCode = KeyCode::Down;
pub const KEY_LIST_UP_VIM: KeyCode = KeyCode::Char('k');
pub const KEY_LIST_DOWN_VIM: KeyCode = KeyCode::Char('j');
pub const KEY_CONFIRM: KeyCode = KeyCode::Enter;
