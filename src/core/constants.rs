use crossterm::event::KeyCode;
use ratatui::style::Color;

// --- APPLICATION CONSTANTS ---
pub const DEFAULT_TICK_RATE_MS: u64 = 33;

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

pub const KEY_NEXT_TRACK_1: KeyCode = KeyCode::Char('l');
pub const KEY_NEXT_TRACK_2: KeyCode = KeyCode::Char('L');
pub const KEY_PREV_TRACK_1: KeyCode = KeyCode::Char('h');
pub const KEY_PREV_TRACK_2: KeyCode = KeyCode::Char('H');

pub const KEY_VOL_UP: KeyCode = KeyCode::Char('p');
pub const KEY_VOL_DOWN: KeyCode = KeyCode::Char('o');

pub const KEY_SEARCH_MODE: KeyCode = KeyCode::Char('/');
pub const KEY_PLAYLIST_MODE: KeyCode = KeyCode::Tab;
pub const KEY_REFRESH: KeyCode = KeyCode::Char('r');
pub const KEY_RADIO_MODE: KeyCode = KeyCode::Char('r'); // Used with Ctrl

pub const KEY_LIST_UP: KeyCode = KeyCode::Up;
pub const KEY_LIST_DOWN: KeyCode = KeyCode::Down;
pub const KEY_LIST_UP_VIM: KeyCode = KeyCode::Char('k');
pub const KEY_LIST_DOWN_VIM: KeyCode = KeyCode::Char('j');
pub const KEY_CONFIRM: KeyCode = KeyCode::Enter;

pub fn color_to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (200, 0, 0),
        Color::Green => (0, 200, 0),
        Color::Yellow => (200, 200, 0),
        Color::Blue => (0, 0, 200),
        Color::Magenta => (200, 0, 200),
        Color::Cyan => (0, 200, 200),
        Color::White => (200, 200, 200),
        Color::Gray => (100, 100, 100),
        Color::DarkGray => (50, 50, 50),
        Color::LightRed => (255, 100, 100),
        Color::LightGreen => (100, 255, 100),
        Color::LightYellow => (255, 255, 100),
        Color::LightBlue => (100, 100, 255),
        Color::LightMagenta => (255, 100, 255),
        Color::LightCyan => (100, 255, 255),
        _ => (150, 150, 150),
    }
}

pub fn interpolate_color(c1: Color, c2: Color, t: f64) -> Color {
    let (r1, g1, b1) = color_to_rgb(c1);
    let (r2, g2, b2) = color_to_rgb(c2);
    
    // Convert t [0.0, 1.0] to fixed point 8.8 (0 to 256)
    let t_fixed = (t.clamp(0.0, 1.0) * 256.0) as i32;
    let t_inv = 256 - t_fixed;

    // Fixed point interpolation: (c1 * (256 - t) + c2 * t) >> 8
    Color::Rgb(
        ((r1 as i32 * t_inv + r2 as i32 * t_fixed) >> 8) as u8,
        ((g1 as i32 * t_inv + g2 as i32 * t_fixed) >> 8) as u8,
        ((b1 as i32 * t_inv + b2 as i32 * t_fixed) >> 8) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_to_rgb() {
        assert_eq!(color_to_rgb(Color::Rgb(10, 20, 30)), (10, 20, 30));
        assert_eq!(color_to_rgb(Color::Black), (0, 0, 0));
        assert_eq!(color_to_rgb(Color::White), (200, 200, 200));
        assert_eq!(color_to_rgb(Color::Reset), (150, 150, 150));
    }

    #[test]
    fn test_interpolate_color() {
        let c1 = Color::Rgb(0, 0, 0);
        let c2 = Color::Rgb(255, 255, 255);
        
        // Midpoint
        assert_eq!(interpolate_color(c1, c2, 0.5), Color::Rgb(127, 127, 127));
        
        // Start
        assert_eq!(interpolate_color(c1, c2, 0.0), Color::Rgb(0, 0, 0));
        
        // End
        assert_eq!(interpolate_color(c1, c2, 1.0), Color::Rgb(255, 255, 255));
        
        // Clamping
        assert_eq!(interpolate_color(c1, c2, -1.0), Color::Rgb(0, 0, 0));
        assert_eq!(interpolate_color(c1, c2, 2.0), Color::Rgb(255, 255, 255));
    }
}
