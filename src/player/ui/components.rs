use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::time::Duration;

pub fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VisualizerMode {
    Wave,
    Bars,
    Blocks,
    Pulse,
    Dots,
    Matrix,
    Noise,
    Interference,
    Orbit,
    Particles,
}

impl Default for VisualizerMode {
    fn default() -> Self {
        Self::Wave
    }
}

impl VisualizerMode {
    pub fn next(&self) -> Self {
        match self {
            Self::Wave => Self::Bars,
            Self::Bars => Self::Blocks,
            Self::Blocks => Self::Pulse,
            Self::Pulse => Self::Dots,
            Self::Dots => Self::Matrix,
            Self::Matrix => Self::Noise,
            Self::Noise => Self::Interference,
            Self::Interference => Self::Orbit,
            Self::Orbit => Self::Particles,
            Self::Particles => Self::Wave,
        }
    }
}

/// Renders a single-row high-density visualizer.
pub fn render_visualizer(
    is_playing: bool,
    width: u16,
    seed: u64,
    volume: f64,
    theme: &crate::config::Theme,
    mode: VisualizerMode,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let patterns = [" ", "⠂", "⠒", "⠖", "⠶", "⠷", "⠿", "⡿", "⣿"];
    
    if !is_playing {
        for _ in 0..width {
            spans.push(Span::styled(" ", Style::default()));
        }
        return spans;
    }

    let time = seed as f64 / 100.0;
    let vol_factor = volume.max(0.1);
    
    for i in 0..width {
        let x = i as f64;
        let norm_x = x / width as f64;
        let envelope = (-(norm_x - 0.5).powi(2) * 6.0).exp();
        
        let val = match mode {
            VisualizerMode::Wave => {
                let w1 = (time * 1.5 + x * 0.2).sin();
                let w2 = (time * 2.8 - x * 0.4).cos() * 0.6;
                (w1 + w2).abs() * 4.0
            }
            VisualizerMode::Bars => {
                let h = (time * 2.0 + (x * 0.5).sin()).sin().abs();
                h * 8.0
            }
            VisualizerMode::Blocks => {
                let h = (time * 3.0 + x * 0.1).cos().abs() * (x * 0.3).sin().abs();
                h * 12.0
            }
            VisualizerMode::Pulse => {
                let p = (time * 4.0).sin().abs() * envelope;
                p * 8.0
            }
            VisualizerMode::Dots => {
                if (x + time * 10.0) as u64 % 5 == 0 { 6.0 } else { 0.0 }
            }
            VisualizerMode::Matrix => {
                let r = (x * 123.456 + time).sin().abs();
                if r > 0.8 { r * 8.0 } else { 0.0 }
            }
            VisualizerMode::Noise => {
                let n = (x * time * 0.001).sin().abs() * (x * 7.7).cos().abs();
                n * 8.0
            }
            VisualizerMode::Interference => {
                let w1 = (time * 1.2 + x * 0.3).sin();
                let w2 = (time * 0.8 - x * 0.2).cos();
                (w1 * w2).abs() * 8.0
            }
            VisualizerMode::Orbit => {
                let o = (time + (x * 0.1)).sin() * (time * 0.5).cos();
                o.abs() * 10.0
            }
            VisualizerMode::Particles => {
                let p = ((x - (time * 20.0) % width as f64).abs() < 2.0) as u64 as f64;
                p * 8.0
            }
        };

        let intensity = val * vol_factor * envelope;
        let idx = (intensity.round() as usize).min(patterns.len() - 1);
        
        let color = if intensity > 3.0 {
            theme.accent
        } else if intensity > 1.0 {
            theme.accent_dim
        } else {
            theme.dim
        };

        spans.push(Span::styled(patterns[idx], Style::default().fg(color)));
    }
    
    spans
}

pub fn render_radio_art(
    is_playing: bool,
    is_starting: bool,
    width: u16,
    height: u16,
    time_ms: u64,
    station_name: &str,
    theme: &crate::config::Theme,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let time = time_ms as f64 / 400.0;
    
    let station_hash: f64 = station_name.bytes().map(|b| b as u64).sum::<u64>() as f64;
    let base_freq = 1.0 + (station_hash % 5.0) * 0.2;
    
    let center_x = width as f64 / 2.0;
    let center_y = height as f64 / 2.0;
    
    let chars = [" ", "·", "⠤", "⠶", "⡾", "⣿"];
    
    for y in 0..height {
        let mut spans = Vec::new();
        for x in 0..width {
            let dx = (x as f64 - center_x) * 1.8; // Aspect ratio adjustment
            let dy = y as f64 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);
            
            if is_starting {
                let spin = (angle + time * 4.0).sin();
                let ring = (dist - 4.0).abs();
                let intensity = if ring < 2.0 && spin > 0.0 { spin * (2.0 - ring) / 2.0 } else { 0.0 };
                
                let idx = (intensity * chars.len() as f64) as usize;
                let color = if intensity > 0.5 { theme.accent } else { theme.accent_dim };
                spans.push(Span::styled(chars[idx.clamp(0, chars.len() - 1)], Style::default().fg(color)));
            } else if is_playing {
                let w1 = (dist - time * 3.0 * base_freq).sin();
                let w2 = (angle * 3.0 + time).cos() * 0.5;
                let w3 = (dx * 0.5 + time * 2.0).sin() * 0.3;
                
                let combined = (w1 + w2 + w3) * (1.0 - dist / 15.0).max(0.0);
                let intensity = combined.max(0.0);
                
                let idx = (intensity * chars.len() as f64) as usize;
                let color = if intensity > 0.8 {
                    theme.accent
                } else if intensity > 0.4 {
                    theme.accent_dim
                } else {
                    theme.dim
                };
                spans.push(Span::styled(chars[idx.clamp(0, chars.len() - 1)], Style::default().fg(color)));
            } else {
                let val = (dist * 0.5 + station_hash).sin();
                if val > 0.9 {
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
