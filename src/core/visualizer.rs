use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub enum VisualizerMode {
    #[default]
    Bars,
    BarsDot,
    Rain,
    BarsOutline,
    Bricks,
    Columns,
    ClassicPeak,
    Wave,
    Scatter,
    Flame,
    Retro,
    Pulse,
    Matrix,
    Binary,
    Sakura,
    Firework,
    Bubbles,
    Logo,
    Terrain,
    Glitch,
    Scope,
    Heartbeat,
    Butterfly,
    Lightning,
    Blocks,
    Dots,
    Noise,
    Interference,
    Orbit,
    Particles,
    None,
}

impl VisualizerMode {
    pub fn next(&self) -> Self {
        match self {
            Self::Bars => Self::BarsDot,
            Self::BarsDot => Self::Rain,
            Self::Rain => Self::BarsOutline,
            Self::BarsOutline => Self::Bricks,
            Self::Bricks => Self::Columns,
            Self::Columns => Self::ClassicPeak,
            Self::ClassicPeak => Self::Wave,
            Self::Wave => Self::Scatter,
            Self::Scatter => Self::Flame,
            Self::Flame => Self::Retro,
            Self::Retro => Self::Pulse,
            Self::Pulse => Self::Matrix,
            Self::Matrix => Self::Binary,
            Self::Binary => Self::Sakura,
            Self::Sakura => Self::Firework,
            Self::Firework => Self::Bubbles,
            Self::Bubbles => Self::Logo,
            Self::Logo => Self::Terrain,
            Self::Terrain => Self::Glitch,
            Self::Glitch => Self::Scope,
            Self::Scope => Self::Heartbeat,
            Self::Heartbeat => Self::Butterfly,
            Self::Butterfly => Self::Lightning,
            Self::Lightning => Self::Blocks,
            Self::Blocks => Self::Dots,
            Self::Dots => Self::Noise,
            Self::Noise => Self::Interference,
            Self::Interference => Self::Orbit,
            Self::Orbit => Self::Particles,
            Self::Particles => Self::None,
            Self::None => Self::Bars,
        }
    }
}

fn color_to_rgb(c: Color) -> (u8, u8, u8) {
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

fn interpolate_color(c1: Color, c2: Color, t: f64) -> Color {
    let (r1, g1, b1) = color_to_rgb(c1);
    let (r2, g2, b2) = color_to_rgb(c2);
    let t = t.clamp(0.0, 1.0);
    Color::Rgb(
        (r1 as f64 * (1.0 - t) + r2 as f64 * t) as u8,
        (g1 as f64 * (1.0 - t) + g2 as f64 * t) as u8,
        (b1 as f64 * (1.0 - t) + b2 as f64 * t) as u8,
    )
}

pub fn render_visualizer(
    is_playing: bool,
    width: u16,
    height: u16,
    seed: u64,
    amplitude: f64,
    theme: &crate::core::constants::Theme,
    mode: VisualizerMode,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if mode == VisualizerMode::None || !is_playing {
        for _ in 0..height {
            lines.push(Line::from(" "));
        }
        return lines;
    }

    let time = seed as f64 / 100.0;
    let vol = (amplitude * 22.0).clamp(0.05, 3.5);

    for y_row in 0..height {
        let mut spans = Vec::new();
        let norm_y = (height as f64 - 1.0 - y_row as f64) / height as f64;
        let mid_y = 0.5;

        for i in 0..width {
            let x = i as f64;
            let norm_x = x / width as f64;
            let envelope = (-(norm_x - 0.5).powi(2) * 6.0).exp();

            let (is_active, char_idx, shapes, custom_color) = match mode {
                VisualizerMode::Wave => {
                    let ribbon_width = 0.12 * vol;
                    let wave = (time * 3.5 + x * 0.12).sin() * 0.4 * vol + mid_y;
                    let dist = (norm_y - wave).abs();
                    let hue_shift = (time * 0.5 + norm_x).sin() * 0.5 + 0.5;
                    let color = interpolate_color(theme.accent, theme.critical, hue_shift);
                    (
                        dist < ribbon_width,
                        if dist < ribbon_width * 0.4 { 4 } else { 2 },
                        [" ", "·", "≈", "≋", "█"],
                        Some(color),
                    )
                }
                VisualizerMode::Bars => {
                    let h = ((x * 0.8 + time * 2.5).sin() * 0.35 + 0.5) * vol * envelope;
                    let color = interpolate_color(theme.accent_dim, theme.accent, (norm_y / h.max(0.1)).min(1.0));
                    (
                        norm_y < h,
                        if h - norm_y > 0.2 { 4 } else { 2 },
                        [" ", " ", "▄", "▆", "█"],
                        Some(color),
                    )
                }
                VisualizerMode::Blocks => {
                    let cell_x = (norm_x * 14.0).floor();
                    let cell_y = (norm_y * 7.0).floor();
                    let noise = (cell_x * 0.82 + cell_y * 1.1 + time * 6.0).sin().abs();
                    let color = interpolate_color(theme.accent_dim, theme.accent, noise);
                    (
                        noise < vol * 0.65,
                        if noise < vol * 0.35 { 4 } else { 2 },
                        [" ", "░", "▒", "▓", "█"],
                        Some(color),
                    )
                }
                VisualizerMode::Pulse => {
                    let dx = (norm_x - 0.5) * 2.8;
                    let dy = (norm_y - 0.5) * 2.2;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let ring = (dist - vol * 0.9).abs();
                    let pulse_color = interpolate_color(theme.accent, theme.critical, (time.sin() * 0.5 + 0.5).abs());
                    (
                        ring < 0.25,
                        if ring < 0.1 { 4 } else { 2 },
                        [" ", "·", "o", "O", "◎"],
                        Some(pulse_color),
                    )
                }
                VisualizerMode::Dots => {
                    let sparkle = (x * 0.52 + norm_y * 0.95 + time * 12.0).cos().abs();
                    let is_active = sparkle > (1.25 - vol * 0.65);
                    let color = if is_active {
                        interpolate_color(theme.accent, theme.fg, (sparkle - 0.6) / 0.4)
                    } else { theme.bg };
                    (is_active, 4, [" ", " ", "·", "•", "●"], Some(color))
                }
                VisualizerMode::Matrix => {
                    let speed = 5.0 + vol * 12.0;
                    let trail = (x * 17.0 + time * speed) % 20.0;
                    let dist = ((1.0 - norm_y) * 20.0 - trail).abs();
                    let color = interpolate_color(theme.dim, theme.accent, 1.0 - (dist / 2.5).min(1.0));
                    (
                        dist < 2.5,
                        if dist < 1.0 { 4 } else { 2 },
                        [" ", " ", ".", "1", "0"],
                        Some(color),
                    )
                }
                VisualizerMode::Noise => {
                    let n = (x * 1.8 + norm_y * 1.8 + time * 30.0).sin().abs();
                    let color = interpolate_color(theme.bg, theme.dim, n);
                    (
                        n < vol * 0.85,
                        if n < vol * 0.45 { 4 } else { 2 },
                        [" ", " ", "░", "▒", "▓"],
                        Some(color),
                    )
                }
                VisualizerMode::Interference => {
                    let line1 = (x * 0.18 + norm_y * 0.45 + time * 2.5).sin().abs();
                    let line2 = (x * 0.18 - norm_y * 0.45 - time * 1.8).cos().abs();
                    let val = (line1 * line2) * vol;
                    let color = interpolate_color(theme.accent_dim, theme.critical, val.min(1.0));
                    (
                        val > 0.3,
                        if val > 0.75 { 4 } else { 2 },
                        [" ", "·", "+", "#", "■"],
                        Some(color),
                    )
                }
                VisualizerMode::Orbit => {
                    let a1 = time * 3.5;
                    let a2 = time * -2.5;
                    let r = 0.28 * vol;
                    let p1_x = 0.5 + a1.cos() * r * 1.6;
                    let p1_y = 0.5 + a1.sin() * r;
                    let p2_x = 0.5 + a2.cos() * r * 2.0;
                    let p2_y = 0.5 + a2.sin() * r * 0.9;
                    let d1 = ((norm_x - p1_x).powi(2) * 3.5 + (norm_y - p1_y).powi(2)).sqrt();
                    let d2 = ((norm_x - p2_x).powi(2) * 3.5 + (norm_y - p2_y).powi(2)).sqrt();
                    let color = interpolate_color(theme.accent, theme.critical, (a1.sin() * 0.5 + 0.5).abs());
                    (
                        d1 < 0.18 || d2 < 0.18,
                        if d1 < 0.06 || d2 < 0.06 { 4 } else { 2 },
                        [" ", " ", "·", "o", "O"],
                        Some(color),
                    )
                }
                VisualizerMode::Particles => {
                    let row_y = (norm_y * 10.0).floor();
                    let speed = (row_y * 0.6 + 2.5) * (1.1 + vol);
                    let p = ((x - time * speed) % (width as f64)).abs();
                    let color = interpolate_color(theme.dim, theme.accent, (p / 3.5).min(1.0));
                    (
                        p < 3.5 * vol,
                        if p < 1.2 * vol { 4 } else { 2 },
                        [" ", " ", " ", "·", "•"],
                        Some(color),
                    )
                }
                VisualizerMode::BarsDot => {
                    let h = ((x * 0.5 + time * 1.8).cos().abs()) * vol * envelope;
                    let dist = (norm_y - h).abs();
                    let color = interpolate_color(theme.accent, theme.fg, (1.0 - dist / 0.06).max(0.0));
                    (dist < 0.06, 4, [" ", " ", "·", "•", "●"], Some(color))
                }
                VisualizerMode::Rain => {
                    let drop = (x * 123.456 + time * 6.0) % 1.6;
                    let dist = ((1.0 - norm_y) - drop).abs();
                    let color = interpolate_color(theme.bg, theme.accent_dim, (1.0 - dist / 0.12).min(1.0));
                    (dist < 0.12, 3, [" ", " ", "·", "│", "┃"], Some(color))
                }
                VisualizerMode::BarsOutline => {
                    let h = ((x * 0.35 + time * 1.2).sin().abs()) * vol * envelope;
                    let dist = (norm_y - h).abs();
                    let color = interpolate_color(theme.accent_dim, theme.accent, 1.0 - (dist / 0.12).min(1.0));
                    (dist < 0.12, 4, [" ", " ", "┌", "┐", "█"], Some(color))
                }
                VisualizerMode::Bricks => {
                    let cx = (norm_x * 18.0).floor();
                    let cy = (norm_y * 9.0).floor();
                    let v = ((cx * 0.55 + time).sin() * (cy * 0.55).cos()).abs() * vol;
                    let color = interpolate_color(theme.dim, theme.critical, v.min(1.0));
                    (v > 0.35, 4, [" ", " ", "▞", "▚", "■"], Some(color))
                }
                VisualizerMode::Columns => {
                    let cx = (norm_x * 24.0).floor();
                    let h = ((cx * 0.75 + time).cos().abs()) * vol;
                    let color = interpolate_color(theme.accent, theme.dim, (norm_y / h.max(0.1)).min(1.0));
                    (norm_y < h, 4, [" ", " ", " ", " ", "┃"], Some(color))
                }
                VisualizerMode::ClassicPeak => {
                    let h = ((x * 0.25 + time).sin() * 0.5 + 0.5) * vol;
                    let is_peak = (norm_y - h).abs() < 0.06;
                    let color = if is_peak { theme.critical } else { theme.accent_dim };
                    (
                        is_peak || (norm_y < h * 0.25),
                        4,
                        [" ", " ", " ", " ", "▔"],
                        Some(color),
                    )
                }
                VisualizerMode::Scatter => {
                    let n = (x * 88.8 + norm_y * 99.9 + time * 12.0).sin().abs();
                    let is_active = n > (1.25 - vol * 0.55);
                    let color = interpolate_color(theme.dim, theme.accent, n.min(1.0));
                    (is_active, 4, [" ", " ", " ", " ", "·"], Some(color))
                }
                VisualizerMode::Flame => {
                    let f = ((x * 0.12).sin() + (time * 6.0)).cos().abs() * vol * (1.1 - norm_y);
                    let color = interpolate_color(theme.critical, theme.accent, (norm_y / f.max(0.1)).min(1.0));
                    (norm_y < f, 4, [" ", " ", "░", "▒", "▓"], Some(color))
                }
                VisualizerMode::Retro => {
                    let v = ((x * 0.12 + time).sin() * 6.0 + (norm_y * 6.0)).floor() % 2.0;
                    let color = interpolate_color(theme.critical, theme.accent, norm_x);
                    (v == 0.0 && vol > 0.35, 4, [" ", " ", " ", " ", "〓"], Some(color))
                }
                VisualizerMode::Binary => {
                    let b = ((x * 12.0 + norm_y * 12.0 + time * 6.0).sin().abs() > 0.5) as usize;
                    let color = if b == 1 { theme.accent } else { theme.accent_dim };
                    (
                        vol > 0.45 && (i % 3 == 0),
                        3 + b,
                        [" ", " ", " ", "0", "1"],
                        Some(color),
                    )
                }
                VisualizerMode::Sakura => {
                    let drift = (x * 0.12 + time * 0.8 + norm_y).sin().abs();
                    let color = interpolate_color(theme.accent_dim, theme.fg, drift);
                    (drift > 0.88 && vol > 0.25, 4, [" ", " ", " ", " ", "∗"], Some(color))
                }
                VisualizerMode::Firework => {
                    let p = (time * 2.2).fract();
                    let dx = (norm_x - 0.5).abs();
                    let dy = (norm_y - p).abs();
                    let dist = (dx * dx + dy * dy).sqrt();
                    let is_active = dist < 0.12 * vol * p;
                    let color = interpolate_color(theme.critical, theme.accent, p);
                    (is_active, 4, [" ", " ", " ", " ", "×"], Some(color))
                }
                VisualizerMode::Bubbles => {
                    let bx = (x * 0.55 + time * 1.2).cos().abs();
                    let dist = (norm_y - bx).abs();
                    let color = interpolate_color(theme.dim, theme.accent_dim, (1.0 - dist / 0.12).min(1.0));
                    (dist < 0.12 * vol, 4, [" ", " ", " ", " ", "○"], Some(color))
                }
                VisualizerMode::Logo => (
                    (norm_x - 0.5).abs() < 0.22 && (norm_y - 0.5).abs() < 0.22 * vol,
                    4,
                    [" ", " ", " ", " ", "#"],
                    None,
                ),
                VisualizerMode::Terrain => {
                    let h = ((x * 0.12 + time).sin() * (x * 0.22 - time).cos()).abs() * vol;
                    let color = interpolate_color(theme.dim, theme.accent, (norm_y / h.max(0.1)).min(1.0));
                    (norm_y < h, 4, [" ", " ", " ", " ", "▴"], Some(color))
                }
                VisualizerMode::Glitch => {
                    let g = (time * 25.0).sin() > 0.96;
                    let glitch_color = if (time * 50.0).cos() > 0.0 { theme.accent } else { theme.critical };
                    (g && vol > 0.25, 4, [" ", " ", " ", " ", "▙"], Some(glitch_color))
                }
                VisualizerMode::Scope => {
                    let s = ((x * 0.55 - time * 12.0).sin() * 0.45 * vol) + 0.5;
                    let color = interpolate_color(theme.accent, theme.fg, (time * 2.0).sin().abs());
                    ((norm_y - s).abs() < 0.06, 4, [" ", " ", " ", " ", "─"], Some(color))
                }
                VisualizerMode::Heartbeat => {
                    let beat = (time * 4.5).sin().powi(12);
                    let h = 0.5 + beat * 0.45 * vol;
                    let color = interpolate_color(theme.critical, theme.fg, beat.min(1.0));
                    (
                        (norm_y - h).abs() < 0.12 && (norm_x - 0.5).abs() < 0.12,
                        4,
                        [" ", " ", " ", " ", "■"],
                        Some(color),
                    )
                }
                VisualizerMode::Butterfly => {
                    let wing = (x * 0.12 + time * 1.5).sin() * (norm_y - 0.5);
                    let color = interpolate_color(theme.critical, theme.accent, norm_x);
                    (wing.abs() < 0.12 * vol, 4, [" ", " ", " ", " ", "∞"], Some(color))
                }
                VisualizerMode::Lightning => {
                    let l = (time * 12.0).sin() > 0.985;
                    let color = theme.fg;
                    (
                        l && (norm_x - 0.5).abs() < 0.06 * vol,
                        4,
                        [" ", " ", " ", " ", "ϟ"],
                        Some(color),
                    )
                }
                VisualizerMode::None => (false, 0, [" ", " ", " ", " ", " "], None),
            };

            let mut final_idx = 0;
            let mut color = theme.bg;
            if is_active {
                final_idx = char_idx;
                if let Some(c) = custom_color {
                    color = c;
                } else {
                    color = if char_idx == 4 {
                        theme.accent
                    } else {
                        theme.accent_dim
                    };
                }
            } else {
                let glow = (norm_x * 12.0 + time).sin().abs() * (norm_y * 6.0).cos().abs();
                if glow > 0.992 {
                    final_idx = 1;
                    color = theme.status_bg;
                }
            }
            spans.push(Span::styled(shapes[final_idx.clamp(0, shapes.len()-1)], Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }
    lines
}
