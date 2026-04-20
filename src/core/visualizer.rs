use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use crate::core::dsp::DspState;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub enum VisualizerMode {
    #[default]
    Wave,
    None,
}

pub struct Visualizer<'a> {
    pub theme: &'a crate::core::constants::Theme,
    pub dsp: &'a DspState,
    pub time: f64,
    pub is_playing: bool,
}

impl<'a> Visualizer<'a> {
    pub fn new(theme: &'a crate::core::constants::Theme, dsp: &'a DspState, time: f64, is_playing: bool) -> Self {
        Self { theme, dsp, time, is_playing }
    }

    pub fn render(&self, width: u16, height: u16, mode: VisualizerMode) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if mode == VisualizerMode::None || !self.is_playing {
            for _ in 0..height {
                lines.push(Line::from(" "));
            }
            return lines;
        }

        let time = self.time * 0.05; // Even slower for elegance
        let amplitude = self.dsp.amplitude as f64;
        let vol = (amplitude * 15.0).clamp(0.05, 5.0);
        let beat_pulse = if self.dsp.is_beat { 1.15 } else { 1.0 };

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

                let mut braille_grid = [[false; 2]; 4];
                let mut active_count = 0;
                let mut avg_hue = 0.0;

                // Background Depth Layer (Starfield)
                let star_noise = (norm_x * 60.0 + time * 0.1).sin() * (norm_y * 40.0).cos();
                let is_star = star_noise > 0.9985;

                for sy in 0..4 {
                    for sx in 0..2 {
                        let sub_x = norm_x + (sx as f64 / (width as f64 * 2.0));
                        let sub_y = norm_y + (sy as f64 / (height as f64 * 4.0));
                        
                        // Advanced Wave Shader
                        // Combines Time-Domain Waveform with multiple oscillators
                        let sample_idx = (sub_x * (self.dsp.waveform.len() as f64 - 1.0)) as usize;
                        let raw_sample = self.dsp.waveform.get(sample_idx).cloned().unwrap_or(0.0) as f64;
                        
                        // Primary Oscilloscope Line
                        let wave_main = raw_sample * 0.4 * vol * beat_pulse + 0.5;
                        
                        // Secondary Harmonic Glow
                        let wave_harm = (sub_x * 10.0 + time).sin() * 0.1 * vol + wave_main;
                        
                        // Tertiary Deep Pulse
                        let wave_base = (sub_x * 3.0 - time * 0.5).cos() * 0.05 * vol + 0.5;

                        let dist_main = (sub_y - wave_main).abs();
                        let dist_harm = (sub_y - wave_harm).abs();
                        let dist_base = (sub_y - wave_base).abs();

                        let is_wave = dist_main < 0.012 * vol || dist_harm < 0.02 * vol || dist_base < 0.01 * vol;
                        let hue = (sub_x + time * 0.2).sin() * 0.5 + 0.5;

                        if is_wave {
                            braille_grid[3 - sy][sx] = true;
                            active_count += 1;
                            avg_hue += hue;
                        }
                    }
                }

                if active_count > 0 {
                    let h = avg_hue / active_count as f64;
                    let color = interpolate_color(self.theme.accent, self.theme.critical, h.clamp(0.0, 1.0));
                    spans.push(Span::styled(get_braille(braille_grid).to_string(), Style::default().fg(color)));
                } else if is_star {
                    spans.push(Span::styled(".", Style::default().fg(self.theme.dim)));
                } else {
                    // Soft background glow
                    let glow = (norm_x * 5.0 + time).sin().abs() * (norm_y * 3.0).cos().abs();
                    if glow > 0.997 {
                        spans.push(Span::styled("·", Style::default().fg(self.theme.status_bg)));
                    } else {
                        spans.push(Span::raw(" "));
                    }
                }
            }
            lines.push(Line::from(spans));
        }
        lines
    }
}

pub fn render_visualizer(
    is_playing: bool,
    width: u16,
    height: u16,
    time: f64,
    dsp: &DspState,
    theme: &crate::core::constants::Theme,
    mode: VisualizerMode,
) -> Vec<Line<'static>> {
    let viz = Visualizer::new(theme, dsp, time, is_playing);
    viz.render(width, height, mode)
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
