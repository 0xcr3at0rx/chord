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

/// Fast bitwise absolute value for f64
#[inline(always)]
fn xor_abs_f64(f: f64) -> f64 {
    f64::from_bits(f.to_bits() & 0x7FFFFFFFFFFFFFFF)
}

impl<'a> Visualizer<'a> {
    pub fn new(config: VisualizerConfig<'a>) -> Self {
        Self { config }
    }

    pub fn render(&self) -> Vec<Line<'static>> {
        let width = self.config.width;
        let height = self.config.height;

        if !self.config.is_playing {
            return (0..height)
                .map(|_| Line::from(" ".repeat(width as usize)))
                .collect();
        }

        let amplitude = self.config.dsp.amplitude as f64;
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

        let waveform_len = self.config.dsp.waveform.len() as f64 - 1.0;
        let mut wave_ys = Vec::with_capacity(width as usize);
        for i in 0..width {
            let norm_x = i as f64 / width as f64;
            let wave_idx = (norm_x * waveform_len) as usize;
            let sample = self
                .config
                .dsp
                .waveform
                .get(wave_idx)
                .cloned()
                .unwrap_or(0.0) as f64;
            wave_ys.push(sample * 0.45 * vol * beat_pulse + 0.5);
        }

        // Render rows serially to avoid thread overhead causing audio underruns
        (0..height)
            .map(|y_row| {
                let mut spans = Vec::with_capacity(width as usize);
                let norm_y = (height as f64 - 1.0 - y_row as f64) / height as f64;
                let axis_dist = xor_abs_f64(norm_y - 0.5);

                for i in 0..width {
                    let dist = xor_abs_f64(norm_y - wave_ys[i as usize]);

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

                    spans.push(Span::styled(char_str, Style::default().fg(color)));
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
