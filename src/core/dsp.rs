use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;
use std::sync::RwLock;

pub const FFT_SIZE: usize = 2048;

#[derive(Debug, Clone)]
pub struct DspState {
    pub waveform: Vec<f32>,
    pub spectrum: Vec<f32>,
    pub amplitude: f32,
    pub is_beat: bool,
}

impl Default for DspState {
    fn default() -> Self {
        Self {
            waveform: vec![0.0; FFT_SIZE],
            spectrum: vec![0.0; FFT_SIZE / 2],
            amplitude: 0.0,
            is_beat: false,
        }
    }
}

pub struct AudioAnalyzer {
    fft_processor: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    pub state: Arc<RwLock<DspState>>,

    // Adaptive Beat Detection
    energy_history: Vec<f32>,
    energy_avg: f32,
}

impl AudioAnalyzer {
    pub fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_processor = planner.plan_fft_forward(FFT_SIZE);

        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE as f32 - 1.0)).cos())
            })
            .collect();

        Self {
            fft_processor,
            state: Arc::new(RwLock::new(DspState::default())),
            window,
            energy_history: Vec::with_capacity(43),
            energy_avg: 0.0,
        }
    }

    pub fn process_samples(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        let current_amplitude;
        {
            let mut state = self.state.write().unwrap();
            state.waveform = samples.iter().take(FFT_SIZE).cloned().collect();
            current_amplitude = samples.iter().map(|s| s.abs()).sum::<f32>() / samples.len() as f32;
            state.amplitude = (state.amplitude * 0.8) + (current_amplitude * 0.2);
        }

        if samples.len() >= FFT_SIZE {
            let mut input = samples[..FFT_SIZE].to_vec();
            for (i, s) in input.iter_mut().enumerate() {
                *s *= self.window[i];
            }

            let mut output = self.fft_processor.make_output_vec();
            if self.fft_processor.process(&mut input, &mut output).is_ok() {
                let is_beat = self.detect_beat_adaptive(current_amplitude);
                let spectrum: Vec<f32> = output
                    .iter()
                    .map(|c| (c.re * c.re + c.im * c.im).sqrt())
                    .take(FFT_SIZE / 2)
                    .collect();

                let mut state = self.state.write().unwrap();
                state.is_beat = is_beat;
                state.spectrum = spectrum;
            }
        }
    }

    fn detect_beat_adaptive(&mut self, amplitude: f32) -> bool {
        if self.energy_history.len() >= 43 {
            self.energy_history.remove(0);
        }
        self.energy_history.push(amplitude);
        self.energy_avg =
            self.energy_history.iter().sum::<f32>() / self.energy_history.len() as f32;
        amplitude > self.energy_avg * 1.6 && amplitude > 0.05
    }
}
