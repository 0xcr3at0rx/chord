use crate::core::constants::interpolate_color;
use crate::core::dsp::DspState;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

pub struct VisualizerConfig<'a> {
    pub theme: &'a crate::core::constants::Theme,
    pub dsp: &'a DspState,
    pub is_playing: bool,
    pub width: u16,
    pub height: u16,
    pub sample_rate: f32,
}

pub struct Visualizer<'a> {
    pub config: VisualizerConfig<'a>,
}

impl<'a> Visualizer<'a> {
    pub fn new(config: VisualizerConfig<'a>) -> Self {
        Self { config }
    }

    pub fn render(&self) -> Vec<Line<'static>> {
        let width = self.config.width as usize;
        let height = self.config.height as usize;

        if !self.config.is_playing {
            return (0..height)
                .map(|_| Line::from(" ".repeat(width)))
                .collect();
        }

        let amplitude = self.config.dsp.amplitude;
        let vol = (amplitude * 18.0).clamp(0.1, 5.0);
        let beat_pulse = if self.config.dsp.is_beat { 1.3 } else { 1.0 };

        let note_pos = get_note_position(&self.config.dsp.spectrum, self.config.sample_rate);
        let note_color = interpolate_color(
            self.config.theme.accent,
            self.config.theme.critical,
            note_pos,
        );

        let color_beat = interpolate_color(note_color, Color::White, 0.5);
        let color_mid = note_color;
        let color_tail = interpolate_color(note_color, Color::Black, 0.6);
        let char_beat = if self.config.dsp.is_beat { "█" } else { "●" };

        let waveform_len = (self.config.dsp.waveform.len() - 1) as f32;
        let mut wave_ys = Vec::with_capacity(width);
        for i in 0..width {
            let norm_x = i as f32 / width as f32;
            let wave_idx = (norm_x * waveform_len) as usize;
            let sample = self
                .config
                .dsp
                .waveform
                .get(wave_idx)
                .cloned()
                .unwrap_or(0.0);
            wave_ys.push(sample * 0.45 * vol * beat_pulse + 0.5);
        }

        (0..height)
            .map(|y_row| {
                let mut spans = Vec::new();
                let norm_y = (height as f32 - 1.0 - y_row as f32) / height as f32;
                let axis_dist = (norm_y - 0.5).abs();

                let mut current_text = String::with_capacity(width);
                let mut current_color = Color::Reset;

                for i in 0..width {
                    let dist = (norm_y - wave_ys[i]).abs();

                    let (char_str, color) = if dist < 0.015 * vol {
                        (char_beat, color_beat)
                    } else if dist < 0.04 * vol {
                        ("○", color_mid)
                    } else if dist < 0.08 * vol {
                        ("·", color_tail)
                    } else if axis_dist < 0.005 {
                        ("─", self.config.theme.dim)
                    } else {
                        (" ", Color::Reset)
                    };

                    if color == current_color {
                        current_text.push_str(char_str);
                    } else {
                        if !current_text.is_empty() {
                            spans.push(Span::styled(current_text.clone(), Style::default().fg(current_color)));
                        }
                        current_text.clear();
                        current_text.push_str(char_str);
                        current_color = color;
                    }
                }
                
                if !current_text.is_empty() {
                    spans.push(Span::styled(current_text, Style::default().fg(current_color)));
                }

                Line::from(spans)
            })
            .collect()
    }
}

pub fn render_visualizer(config: VisualizerConfig) -> Vec<Line<'static>> {
    let viz = Visualizer::new(config);
    viz.render()
}

fn get_note_position(spectrum: &[f32], sample_rate: f32) -> f64 {
    if spectrum.is_empty() {
        return 0.0;
    }

    let mut max_val = 0.0;
    let mut max_idx = 0;
    for (i, &val) in spectrum.iter().enumerate() {
        if val > max_val {
            max_val = val;
            max_idx = i;
        }
    }

    if max_val < 0.01 {
        return 0.0;
    }

    let fft_size = (spectrum.len() * 2) as f32;
    let freq = max_idx as f32 * sample_rate / fft_size;

    if freq < 20.0 {
        return 0.0;
    }

    let n = 12.0 * (freq / 440.0).log2();
    let note_idx = ((n.round() as i32 % 12) + 12) % 12;
    note_idx as f64 / 12.0
}
