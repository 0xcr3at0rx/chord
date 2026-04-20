use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub enum VisualizerMode {
    #[default]
    Bar,
    BarDot,
    Rain,
    Wave,
    Retro,
    Glitch,
    Noise,
    None,
}

impl VisualizerMode {
    pub fn next(&self) -> Self {
        match self {
            Self::Bar => Self::BarDot,
            Self::BarDot => Self::Rain,
            Self::Rain => Self::Wave,
            Self::Wave => Self::Retro,
            Self::Retro => Self::Glitch,
            Self::Glitch => Self::Noise,
            Self::Noise => Self::None,
            Self::None => Self::Bar,
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

use crate::core::dsp::DspState;

pub fn render_visualizer(
    is_playing: bool,
    width: u16,
    height: u16,
    raw_time: f64,
    dsp: &DspState,
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

    let time = raw_time * 0.1; // Slow down for cinematic feel
    let amplitude = dsp.amplitude as f64;
    let vol = (amplitude * 14.0).clamp(0.05, 4.0);
    let beat_warp = if dsp.is_beat { 1.05 } else { 1.0 };

    // Braille utility for 2x4 density
    let get_braille = |grid: [[bool; 2]; 4]| -> char {
        let mut code = 0;
        if grid[0][0] { code |= 1; }
        if grid[1][0] { code |= 2; }
        if grid[2][0] { code |= 4; }
        if grid[0][1] { code |= 8; }
        if grid[1][1] { code |= 16; }
        if grid[2][1] { code |= 32; }
        if grid[3][0] { code |= 64; }
        if grid[3][1] { code |= 128; }
        std::char::from_u32(0x2800 + code).unwrap_or(' ')
    };

    for y_row in 0..height {
        let mut spans = Vec::new();
        let norm_y = (height as f64 - 1.0 - y_row as f64) / height as f64;

        for i in 0..width {
            let x = i as f64;
            let norm_x = x / width as f64;

            // GPU-style Shader Logic using sub-pixels (Braille)
            let mut braille_grid = [[false; 2]; 4];
            let mut active_count = 0;
            let mut avg_hue = 0.0;

            for sy in 0..4 {
                for sx in 0..2 {
                    let sub_x = norm_x + (sx as f64 / (width as f64 * 2.0));
                    let sub_y = norm_y + (sy as f64 / (height as f64 * 4.0));
                    
                    let (active, hue) = match mode {
                        VisualizerMode::Wave => {
                            let sample_idx = (sub_x * (dsp.waveform.len() as f64 - 1.0)) as usize;
                            let sample = dsp.waveform.get(sample_idx).cloned().unwrap_or(0.0) as f64;
                            let wave = sample * 0.5 * vol + 0.5;
                            let dist = (sub_y - wave).abs();
                            (dist < 0.02 * vol, (time * 0.1 + sub_x).sin() * 0.5 + 0.5)
                        }
                        VisualizerMode::Bar => {
                            let band_idx = (sub_x * (crate::core::dsp::NUM_BANDS as f64 - 1.0)) as usize;
                            let h = (dsp.bands.get(band_idx).cloned().unwrap_or(0.0) as f64 * 3.5 * vol).min(1.0);
                            let peak = (dsp.peaks.get(band_idx).cloned().unwrap_or(0.0) as f64 * 3.5 * vol).min(1.0);
                            let is_peak = (sub_y - peak).abs() < 0.01;
                            (sub_y < h || is_peak, sub_y / h.max(0.1))
                        }
                        VisualizerMode::BarDot => {
                            let band_idx = (sub_x * (crate::core::dsp::NUM_BANDS as f64 - 1.0)) as usize;
                            let h = (dsp.bands.get(band_idx).cloned().unwrap_or(0.0) as f64 * 4.0 * vol).min(1.0);
                            let dist = (sub_y - h).abs();
                            (dist < 0.015 * vol, sub_x)
                        }
                        VisualizerMode::Rain => {
                            let band_idx = (sub_x * (crate::core::dsp::NUM_BANDS as f64 - 1.0)) as usize;
                            let energy = dsp.bands.get(band_idx).cloned().unwrap_or(0.0) as f64;
                            let speed = 0.4 + (sub_x * 7.7).fract() * 0.2 + energy * 2.0;
                            let drop_pos = (time * speed + (sub_x * 0.5)) % 1.4;
                            let head_y = 1.3 - drop_pos;
                            let dist = (sub_y - head_y).abs();
                            let is_splash = sub_y < 0.05 && head_y < 0.05;
                            (sub_y <= head_y && dist < 0.4 || is_splash, 1.0 - dist / 0.4)
                        }
                        VisualizerMode::Retro => {
                            let chroma_idx = (sub_x * 12.0) as usize % 12;
                            let chroma_val = dsp.chromagram[chroma_idx] as f64;
                            let grid = (sub_x * 40.0).sin() * (sub_y * 20.0).cos();
                            (grid.abs() < chroma_val * 0.5 * beat_warp, chroma_val)
                        }
                        VisualizerMode::Glitch => {
                            let noise = (sub_x * 50.0 + time * 5.0).sin() * (sub_y * 30.0 + time).cos();
                            (noise.abs() > 0.98 - (amplitude * 0.1), sub_x)
                        }
                        VisualizerMode::Noise => {
                            let n = (sub_x * 10.0 + sub_y * 10.0 + time * 2.0).sin().abs();
                            (n < vol * 0.4, n)
                        }
                        _ => (false, 0.0),
                    };

                    if active {
                        braille_grid[3 - sy][sx] = true;
                        active_count += 1;
                        avg_hue += hue;
                    }
                }
            }

            if active_count > 0 {
                let color = match mode {
                    VisualizerMode::Bar => {
                        let h = avg_hue / active_count as f64;
                        interpolate_color(theme.accent_dim, theme.accent, h.min(1.0))
                    }
                    VisualizerMode::Rain => {
                        let d = avg_hue / active_count as f64;
                        interpolate_color(theme.bg, theme.accent, d.clamp(0.0, 1.0))
                    }
                    _ => {
                        let h = avg_hue / active_count as f64;
                        interpolate_color(theme.accent, theme.critical, h.clamp(0.0, 1.0))
                    }
                };
                spans.push(Span::styled(get_braille(braille_grid).to_string(), Style::default().fg(color)));
            } else {
                // Subtle background glow (GPU shader style)
                let glow = (norm_x * 10.0 + time).sin().abs() * (norm_y * 5.0).cos().abs();
                if glow > 0.995 {
                    spans.push(Span::styled("·", Style::default().fg(theme.status_bg)));
                } else {
                    spans.push(Span::raw(" "));
                }
            }
        }
        lines.push(Line::from(spans));
    }
    lines
}
