use ratatui::style::Style;
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
    let vol = (amplitude * 18.0).clamp(0.05, 2.5);

    for y_row in 0..height {
        let mut spans = Vec::new();
        let norm_y = (height as f64 - 1.0 - y_row as f64) / height as f64;
        let mid_y = 0.5;

        for i in 0..width {
            let x = i as f64;
            let norm_x = x / width as f64;
            let envelope = (-(norm_x - 0.5).powi(2) * 5.0).exp();

            let (is_active, char_idx, shapes) = match mode {
                VisualizerMode::Wave => {
                    let ribbon_width = 0.15 * vol;
                    let wave = (time * 3.0 + x * 0.1).sin() * 0.35 * vol + mid_y;
                    let dist = (norm_y - wave).abs();
                    (
                        dist < ribbon_width,
                        if dist < ribbon_width * 0.5 { 4 } else { 2 },
                        [" ", "·", "≈", "≋", "█"],
                    )
                }
                VisualizerMode::Bars => {
                    let h = ((x * 1.5 + time * 2.0).sin() * 0.3 + 0.5) * vol * envelope;
                    (
                        norm_y < h,
                        if h - norm_y > 0.15 { 4 } else { 2 },
                        [" ", " ", "▄", "▆", "█"],
                    )
                }
                VisualizerMode::Blocks => {
                    let cell_x = (norm_x * 12.0).floor();
                    let cell_y = (norm_y * 6.0).floor();
                    let noise = (cell_x * 0.77 + cell_y * 0.99 + time * 5.0).sin().abs();
                    (
                        noise < vol * 0.6,
                        if noise < vol * 0.3 { 4 } else { 2 },
                        [" ", "░", "▒", "▓", "█"],
                    )
                }
                VisualizerMode::Pulse => {
                    let dx = (norm_x - 0.5) * 2.5;
                    let dy = (norm_y - 0.5) * 2.0;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let ring = (dist - vol * 0.8).abs();
                    (
                        ring < 0.2,
                        if ring < 0.08 { 4 } else { 2 },
                        [" ", "·", "o", "O", "◎"],
                    )
                }
                VisualizerMode::Dots => {
                    let sparkle = (x * 0.45 + norm_y * 0.88 + time * 10.0).cos().abs();
                    (sparkle > (1.2 - vol * 0.6), 4, [" ", " ", "·", "•", "●"])
                }
                VisualizerMode::Matrix => {
                    let speed = 4.0 + vol * 10.0;
                    let trail = (x * 13.0 + time * speed) % 15.0;
                    let dist = ((1.0 - norm_y) * 15.0 - trail).abs();
                    (
                        dist < 2.0,
                        if dist < 0.8 { 4 } else { 2 },
                        [" ", " ", ".", "1", "0"],
                    )
                }
                VisualizerMode::Noise => {
                    let n = (x * 1.5 + norm_y * 1.5 + time * 25.0).sin().abs();
                    (
                        n < vol * 0.8,
                        if n < vol * 0.4 { 4 } else { 2 },
                        [" ", " ", "░", "▒", "▓"],
                    )
                }
                VisualizerMode::Interference => {
                    let line1 = (x * 0.15 + norm_y * 0.4 + time * 2.0).sin().abs();
                    let line2 = (x * 0.15 - norm_y * 0.4 - time * 1.5).cos().abs();
                    let val = (line1 * line2) * vol;
                    (
                        val > 0.35,
                        if val > 0.7 { 4 } else { 2 },
                        [" ", "·", "+", "#", "■"],
                    )
                }
                VisualizerMode::Orbit => {
                    let a1 = time * 3.0;
                    let a2 = time * -2.2;
                    let r = 0.25 * vol;
                    let p1_x = 0.5 + a1.cos() * r * 1.5;
                    let p1_y = 0.5 + a1.sin() * r;
                    let p2_x = 0.5 + a2.cos() * r * 1.8;
                    let p2_y = 0.5 + a2.sin() * r * 0.8;
                    let d1 = ((norm_x - p1_x).powi(2) * 3.0 + (norm_y - p1_y).powi(2)).sqrt();
                    let d2 = ((norm_x - p2_x).powi(2) * 3.0 + (norm_y - p2_y).powi(2)).sqrt();
                    (
                        d1 < 0.15 || d2 < 0.15,
                        if d1 < 0.05 || d2 < 0.05 { 4 } else { 2 },
                        [" ", " ", "·", "o", "O"],
                    )
                }
                VisualizerMode::Particles => {
                    let row_y = (norm_y * 8.0).floor();
                    let speed = (row_y * 0.5 + 2.0) * (1.0 + vol);
                    let p = ((x - time * speed) % (width as f64)).abs();
                    (
                        p < 3.0 * vol,
                        if p < 1.0 * vol { 4 } else { 2 },
                        [" ", " ", " ", "·", "•"],
                    )
                }
                VisualizerMode::BarsDot => {
                    let h = ((x * 0.4 + time * 1.5).cos().abs()) * vol * envelope;
                    ((norm_y - h).abs() < 0.05, 4, [" ", " ", "·", "•", "●"])
                }
                VisualizerMode::Rain => {
                    let drop = (x * 123.456 + time * 5.0) % 1.5;
                    let dist = ((1.0 - norm_y) - drop).abs();
                    (dist < 0.1, 3, [" ", " ", "·", "│", "┃"])
                }
                VisualizerMode::BarsOutline => {
                    let h = ((x * 0.3 + time).sin().abs()) * vol * envelope;
                    ((norm_y - h).abs() < 0.1, 4, [" ", " ", "┌", "┐", "█"])
                }
                VisualizerMode::Bricks => {
                    let cx = (norm_x * 15.0).floor();
                    let cy = (norm_y * 8.0).floor();
                    let v = ((cx * 0.5 + time).sin() * (cy * 0.5).cos()).abs() * vol;
                    (v > 0.4, 4, [" ", " ", "▞", "▚", "■"])
                }
                VisualizerMode::Columns => {
                    let cx = (norm_x * 20.0).floor();
                    let h = ((cx * 0.7 + time).cos().abs()) * vol;
                    (norm_y < h, 4, [" ", " ", " ", " ", "┃"])
                }
                VisualizerMode::ClassicPeak => {
                    let h = ((x * 0.2 + time).sin() * 0.5 + 0.5) * vol;
                    (
                        (norm_y - h).abs() < 0.05 || (norm_y < h * 0.2),
                        4,
                        [" ", " ", " ", " ", "▔"],
                    )
                }
                VisualizerMode::Scatter => {
                    let n = (x * 77.7 + norm_y * 88.8 + time * 10.0).sin().abs();
                    (n > (1.2 - vol * 0.5), 4, [" ", " ", " ", " ", "·"])
                }
                VisualizerMode::Flame => {
                    let f = ((x * 0.1).sin() + (time * 5.0)).cos().abs() * vol * (1.0 - norm_y);
                    (norm_y < f, 4, [" ", " ", "░", "▒", "▓"])
                }
                VisualizerMode::Retro => {
                    let v = ((x * 0.1 + time).sin() * 5.0 + (norm_y * 5.0)).floor() % 2.0;
                    (v == 0.0 && vol > 0.3, 4, [" ", " ", " ", " ", "〓"])
                }
                VisualizerMode::Binary => {
                    let b = ((x * 10.0 + norm_y * 10.0 + time * 5.0).sin() > 0.0) as usize;
                    (
                        vol > 0.4 && (x as u64).is_multiple_of(3),
                        3 + b,
                        [" ", " ", " ", "0", "1"],
                    )
                }
                VisualizerMode::Sakura => {
                    let drift = (x * 0.1 + time + norm_y).sin().abs();
                    (drift > 0.9 && vol > 0.2, 4, [" ", " ", " ", " ", "∗"])
                }
                VisualizerMode::Firework => {
                    let p = (time * 2.0).fract();
                    let dx = (norm_x - 0.5).abs();
                    let dy = (norm_y - p).abs();
                    (
                        (dx * dx + dy * dy).sqrt() < 0.1 * vol * p,
                        4,
                        [" ", " ", " ", " ", "×"],
                    )
                }
                VisualizerMode::Bubbles => {
                    let bx = (x * 0.5 + time).cos().abs();
                    (
                        (norm_y - bx).abs() < 0.1 * vol,
                        4,
                        [" ", " ", " ", " ", "○"],
                    )
                }
                VisualizerMode::Logo => (
                    (norm_x - 0.5).abs() < 0.2 && (norm_y - 0.5).abs() < 0.2 * vol,
                    4,
                    [" ", " ", " ", " ", "#"],
                ),
                VisualizerMode::Terrain => {
                    let h = ((x * 0.1 + time).sin() * (x * 0.2 - time).cos()).abs() * vol;
                    (norm_y < h, 4, [" ", " ", " ", " ", "▴"])
                }
                VisualizerMode::Glitch => {
                    let g = (time * 20.0).sin() > 0.95;
                    (g && vol > 0.2, 4, [" ", " ", " ", " ", "▙"])
                }
                VisualizerMode::Scope => {
                    let s = ((x * 0.5 - time * 10.0).sin() * 0.4 * vol) + 0.5;
                    ((norm_y - s).abs() < 0.05, 4, [" ", " ", " ", " ", "─"])
                }
                VisualizerMode::Heartbeat => {
                    let beat = (time * 4.0).sin().powi(10);
                    let h = 0.5 + beat * 0.4 * vol;
                    (
                        (norm_y - h).abs() < 0.1 && (norm_x - 0.5).abs() < 0.1,
                        4,
                        [" ", " ", " ", " ", "■"],
                    )
                }
                VisualizerMode::Butterfly => {
                    let wing = (x * 0.1 + time).sin() * (norm_y - 0.5);
                    (wing.abs() < 0.1 * vol, 4, [" ", " ", " ", " ", "∞"])
                }
                VisualizerMode::Lightning => {
                    let l = (time * 10.0).sin() > 0.98;
                    (
                        l && (norm_x - 0.5).abs() < 0.05 * vol,
                        4,
                        [" ", " ", " ", " ", "ϟ"],
                    )
                }
                VisualizerMode::None => (false, 0, [" ", " ", " ", " ", " "]),
            };

            let mut final_idx = 0;
            let mut color = theme.bg;
            if is_active {
                final_idx = char_idx;
                color = if char_idx == 4 {
                    theme.accent
                } else {
                    theme.accent_dim
                };
            } else {
                let glow = (norm_x * 12.0 + time).sin().abs() * (norm_y * 6.0).cos().abs();
                if glow > 0.99 {
                    final_idx = 1;
                    color = theme.status_bg;
                }
            }
            spans.push(Span::styled(shapes[final_idx], Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }
    lines
}
