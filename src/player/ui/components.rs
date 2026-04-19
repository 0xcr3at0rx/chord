use ratatui::{
    style::Style,
    text::Span,
};
use std::time::Duration;
use crate::player::ui::theme::THEME;

pub fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// Renders a single-row high-density visualizer.
pub fn render_visualizer(is_playing: bool, width: u16, seed: u64, volume: f64) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    
    // Density-based Braille patterns
    let patterns = [" ", "⠂", "⠒", "⠖", "⠶", "⠷", "⠿", "⡿", "⣿"];
    
    if !is_playing {
        for i in 0..width {
            let x = i as f64;
            let norm_x = x / width as f64;
            let env = (-(norm_x - 0.5).powi(2) * 12.0).exp();
            let wave = (x * 0.2 + seed as f64 / 1000.0).sin().abs();
            let idx = if wave * env > 0.1 { 1 } else { 0 };
            spans.push(Span::styled(patterns[idx], Style::default().fg(THEME.status_bg)));
        }
        return spans;
    }

    let time = seed as f64 / 100.0;
    let vol_factor = volume.max(0.2);
    
    for i in 0..width {
        let x = i as f64;
        let norm_x = x / width as f64;
        
        // Triple-wave interference
        let w1 = (time * 1.5 + x * 0.2).sin();
        let w2 = (time * 2.8 - x * 0.4).cos() * 0.6;
        let w3 = (time * 5.0 + x * 0.8).sin() * 0.3;
        
        let combined = w1 + w2 + w3;
        
        // Calculate "slope" (approximate derivative) for reactive coloring
        let next_x = (i + 1) as f64;
        let nw1 = (time * 1.5 + next_x * 0.2).sin();
        let nw2 = (time * 2.8 - next_x * 0.4).cos() * 0.6;
        let nw3 = (time * 5.0 + next_x * 0.8).sin() * 0.3;
        let n_combined = nw1 + nw2 + nw3;
        let slope = (n_combined - combined).abs() * 5.0;
        
        // Focus energy in center
        let envelope = (-(norm_x - 0.5).powi(2) * 8.0).exp();
        
        let val = (combined.abs() * 4.0 + slope) * vol_factor * envelope;
        let idx = (val.round() as usize).min(patterns.len() - 1);
        
        // Color based on slope intensity
        let color = if slope > 1.2 {
            THEME.accent
        } else {
            THEME.accent_dim
        };

        spans.push(Span::styled(patterns[idx], Style::default().fg(color)));
    }
    
    spans
}
