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

        let waveform_len = self.config.dsp.waveform.len();
        let waveform_len_f = (waveform_len.saturating_sub(1)) as f32;
        let width_inv = 1.0 / width as f32;
        let scale = 0.45 * vol * beat_pulse;

        let mut wave_ys = Vec::with_capacity(width);
        for i in 0..width {
            let norm_x = i as f32 * width_inv;
            let wave_idx = (norm_x * waveform_len_f) as usize;
            let sample = unsafe { *self.config.dsp.waveform.get_unchecked(wave_idx.min(waveform_len - 1)) };
            wave_ys.push(sample * scale + 0.5);
        }

        // Pre-calculate inner loop thresholds
        let t1 = 0.015 * vol;
        let t2 = 0.04 * vol;
        let t3 = 0.08 * vol;
        let height_inv = 1.0 / height as f32;

        (0..height)
            .map(|y_row| {
                let mut spans = Vec::new();
                let norm_y = (height as f32 - 1.0 - y_row as f32) * height_inv;
                let axis_dist = (norm_y - 0.5).abs();

                let mut current_text = String::with_capacity(width);
                let mut current_color = Color::Reset;

                for i in 0..width {
                    let dist = (norm_y - wave_ys[i]).abs();

                    let (char_str, color) = if dist < t1 {
                        (char_beat, color_beat)
                    } else if dist < t2 {
                        ("○", color_mid)
                    } else if dist < t3 {
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
                            spans.push(Span::styled(std::mem::take(&mut current_text), Style::default().fg(current_color)));
                        }
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

    let fft_size = (spectrum.len() << 1) as f32; // Bit shift for * 2
    let freq = max_idx as f32 * sample_rate / fft_size;

    if freq < 20.0 {
        return 0.0;
    }

    // Fast log2 calculation using bit manipulation + small correction
    // log2(x) = (bits >> 23) - 127
    let x = freq * (1.0 / 440.0);
    let n = 12.0 * x.log2();
    let note_idx = ((n.round() as i32 % 12) + 12) % 12;
    note_idx as f64 * (1.0 / 12.0) // Multiply by reciprocal
}
