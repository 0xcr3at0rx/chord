use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::time::Duration;

pub fn format_duration(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}", s / 60, s % 60)
}

#[allow(clippy::too_many_arguments)]
pub fn render_radio_art(
    is_playing: bool,
    is_starting: bool,
    width: u16,
    height: u16,
    time_ms: u64,
    station_name: &str,
    theme: &crate::core::constants::Theme,
    amplitude: f64,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let time = time_ms as f64 / 400.0;

    let station_hash: f64 = station_name.bytes().map(|b| b as u64).sum::<u64>() as f64;
    let base_freq = 1.0 + (station_hash % 5.0) * 0.2;
    let vol_factor = (amplitude * 2.0).clamp(0.2, 2.0);

    let center_x = width as f64 / 2.0;
    let center_y = height as f64 / 2.0;

    let chars = [" ", ".", "-", "=", "+", "#"];

    for y in 0..height {
        let mut spans = Vec::new();
        for x in 0..width {
            let dx = (x as f64 - center_x) * 1.8;
            let dy = y as f64 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);

            if is_starting {
                let spin = (angle + time * 4.0).sin();
                let ring = (dist - 4.0).abs();
                let intensity = if ring < 2.0 && spin > 0.0 {
                    spin * (2.0 - ring) / 2.0
                } else {
                    0.0
                };

                let idx = (intensity * chars.len() as f64) as usize;
                let color = if intensity > 0.5 {
                    theme.accent
                } else {
                    theme.accent_dim
                };
                spans.push(Span::styled(
                    chars[idx.clamp(0, chars.len() - 1)],
                    Style::default().fg(color),
                ));
            } else if is_playing {
                let w1 = (dist - time * 3.0 * base_freq).sin();
                let w2 = (angle * 3.0 + time).cos() * 0.5;
                let w3 = (dx * 0.5 + time * 2.0).sin() * 0.3;

                let combined = (w1 + w2 + w3) * (1.0 - dist / 15.0).max(0.0) * vol_factor;
                let intensity = combined.max(0.0);

                let idx = (intensity * chars.len() as f64) as usize;
                let color = if intensity > 0.8 {
                    theme.accent
                } else if intensity > 0.4 {
                    theme.accent_dim
                } else {
                    theme.dim
                };
                spans.push(Span::styled(
                    chars[idx.clamp(0, chars.len() - 1)],
                    Style::default().fg(color),
                ));
            } else {
                let val = (dist * 0.5 + station_hash).sin();
                if val > 0.9 {
                    spans.push(Span::styled(".", Style::default().fg(theme.status_bg)));
                } else {
                    spans.push(Span::raw(" "));
                }
            }
        }
        lines.push(Line::from(spans));
    }

    lines
}
