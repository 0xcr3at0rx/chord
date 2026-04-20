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
    seed: u64,
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

    let time = seed as f64 / 600.0; // Ultra-slow, cinematic movement
    let amplitude = dsp.amplitude as f64;
    let vol = (amplitude * 16.0).clamp(0.05, 4.0);
    let beat_warp = if dsp.is_beat { 1.08 } else { 1.0 };

    for y_row in 0..height {
        let mut spans = Vec::new();
        let norm_y = (height as f64 - 1.0 - y_row as f64) / height as f64;
        let mid_y = 0.5;

        for i in 0..width {
            let x = i as f64;
            let norm_x = x / width as f64;
            let _envelope = (-(norm_x - 0.5).powi(2) * 6.0).exp();

            let (is_active, char_idx, shapes, custom_color) = match mode {
                VisualizerMode::Wave => {
                    let sample_idx = (norm_x * (dsp.waveform.len() as f64 - 1.0)) as usize;
                    let sample = dsp.waveform.get(sample_idx).cloned().unwrap_or(0.0) as f64;
                    let wave = sample * 0.6 * vol + mid_y;
                    let dist = (norm_y - wave).abs();
                    let hue_shift = (time * 0.15 + norm_x).sin() * 0.5 + 0.5;
                    let color = interpolate_color(theme.accent, theme.critical, hue_shift);
                    
                    let idx = if dist < 0.005 * vol { 4 } 
                             else if dist < 0.015 * vol { 3 }
                             else if dist < 0.03 * vol { 2 }
                             else { 1 };
                    (
                        dist < 0.05 * vol,
                        idx,
                        [" ", "·", "≈", "≋", "█"],
                        Some(color),
                    )
                }
                VisualizerMode::Bar => {
                    let band_idx = (norm_x * (crate::core::dsp::NUM_BANDS as f64 - 1.0)) as usize;
                    let h = (dsp.bands.get(band_idx).cloned().unwrap_or(0.0) as f64 * 3.2 * vol).min(1.0);
                    let peak = (dsp.peaks.get(band_idx).cloned().unwrap_or(0.0) as f64 * 3.2 * vol).min(1.0);
                    
                    let is_peak = (norm_y - peak).abs() < 0.02;
                    let color = if is_peak {
                        theme.critical
                    } else {
                        interpolate_color(theme.accent_dim, theme.accent, (norm_y / h.max(0.1)).min(1.0))
                    };
                    
                    let fill_idx = if h - norm_y < 0.03 { 2 } 
                                 else if h - norm_y < 0.08 { 3 } 
                                 else { 4 };
                    
                    (
                        norm_y < h || is_peak,
                        if is_peak { 4 } else { fill_idx },
                        [" ", " ", "▄", "▆", "█"],
                        Some(color),
                    )
                }
                VisualizerMode::BarDot => {
                    let band_idx = (norm_x * (crate::core::dsp::NUM_BANDS as f64 - 1.0)) as usize;
                    let h = (dsp.bands.get(band_idx).cloned().unwrap_or(0.0) as f64 * 3.8 * vol).min(1.0);
                    let dist = (norm_y - h).abs();
                    let color = interpolate_color(theme.accent, theme.fg, (1.0 - dist / 0.12).max(0.0));
                    
                    let idx = if dist < 0.015 { 4 }
                             else if dist < 0.04 { 3 }
                             else if dist < 0.07 { 2 }
                             else { 1 };
                    (dist < 0.09, idx, [" ", "·", "•", "●", "⬤"], Some(color))
                }
                VisualizerMode::Rain => {
                    let band_idx = (norm_x * (crate::core::dsp::NUM_BANDS as f64 - 1.0)) as usize;
                    let energy = dsp.bands.get(band_idx).cloned().unwrap_or(0.0) as f64;
                    
                    // Rain falls from top (norm_y=1.0) to bottom (norm_y=0.0)
                    // We use multiple offsets per column for density
                    let rain_time = time * 0.8;
                    let speed = 0.5 + (x % 7.0) * 0.1 + energy * 2.0;
                    let drop_cycle = (rain_time * speed + (x * 0.41)) % 1.5;
                    let head_y = 1.2 - drop_cycle; // Falls from slightly above
                    
                    let dist = (norm_y - head_y).abs();
                    let is_active = norm_y <= head_y && dist < 0.4;
                    
                    // Splashing logic at the bottom
                    let is_splash = norm_y < 0.05 && head_y < 0.1;
                    
                    let color = if is_splash {
                        theme.accent_dim
                    } else {
                        interpolate_color(theme.bg, theme.accent, (1.0 - dist / 0.4).clamp(0.0, 1.0))
                    };
                    
                    let idx = if is_splash {
                        ((time * 20.0 + x).sin().abs() * 2.0 + 1.0) as usize // Splash animation
                    } else if dist < 0.02 { 4 } // Head
                      else if dist < 0.10 { 3 } // Body
                      else if dist < 0.25 { 2 } // Tail
                      else { 1 };
                    
                    (is_active || is_splash, idx, [" ", "·", "│", "┃", "╽"], Some(color))
                }
                VisualizerMode::Retro => {
                    let chroma_idx = (norm_x * 12.0) as usize % 12;
                    let chroma_val = dsp.chromagram[chroma_idx] as f64;
                    let scanline = (norm_y * 10.0 + time * 2.0).sin() * 0.5 + 0.5;
                    let v = ((x * 0.04 + time * 0.2).sin() * 3.0 + (norm_y * 3.0)).floor() % 2.0;
                    let color = interpolate_color(theme.critical, theme.accent, chroma_val * scanline);
                    
                    let idx = if chroma_val > 0.8 { 4 }
                             else if chroma_val > 0.5 { 3 }
                             else { 2 };
                    
                    (v == 0.0 && chroma_val > 0.15 * beat_warp, idx, [" ", " ", "▒", "▓", "█"], Some(color))
                }
                VisualizerMode::Glitch => {
                    let band_idx = (norm_x * (crate::core::dsp::NUM_BANDS as f64 - 1.0)) as usize;
                    let energy = dsp.bands.get(band_idx).cloned().unwrap_or(0.0) as f64;
                    let g = (time * 8.0 + energy * 25.0 + x * 0.5).sin() > 0.985;
                    let glitch_color = if (time * 15.0).cos() > 0.0 { theme.accent } else { theme.critical };
                    
                    let idx = ((time * 5.0 + x).cos().abs() * 4.0) as usize;
                    (g && vol > 0.2, idx.clamp(1, 4), [" ", "▖", "▗", "▘", "▙"], Some(glitch_color))
                }
                VisualizerMode::Noise => {
                    let noise_val = (x * 0.9 + norm_y * 0.9 + time * 10.0).sin();
                    let n = noise_val.abs();
                    let color = interpolate_color(theme.bg, theme.dim, n * amplitude * 5.0);
                    
                    let idx = (n * 4.0) as usize;
                    (
                        n < vol * 0.7,
                        idx.clamp(1, 4),
                        [" ", "░", "▒", "▓", "█"],
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
