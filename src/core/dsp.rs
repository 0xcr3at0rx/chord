use std::sync::Arc;
use realfft::{RealFftPlanner, RealToComplex};
use std::sync::RwLock;

pub const FFT_SIZE: usize = 2048;

#[derive(Debug, Clone, Default)]
pub struct DspState {
    pub bins: Vec<f32>,
    pub waveform: Vec<f32>,
    pub amplitude: f32,
    pub is_beat: bool,
    pub chromagram: [f32; 12],
}

pub struct AudioAnalyzer {
    fft_processor: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    pub state: Arc<RwLock<DspState>>,
    
    // Kalman filters for smoothing bins
    kalman_states: Vec<f32>,
    kalman_p: Vec<f32>,
}

impl AudioAnalyzer {
    pub fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_processor = planner.plan_fft_forward(FFT_SIZE);
        
        // Hann window
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE as f32 - 1.0)).cos()))
            .collect();

        Self {
            fft_processor,
            window,
            state: Arc::new(RwLock::new(DspState::default())),
            kalman_states: vec![0.0; FFT_SIZE / 2 + 1],
            kalman_p: vec![1.0; FFT_SIZE / 2 + 1],
        }
    }

    pub fn process_samples(&mut self, samples: &[f32]) {
        if samples.is_empty() { return; }

        // Update waveform (last FFT_SIZE samples)
        {
            let mut state = self.state.write().unwrap();
            state.waveform = samples.iter().take(FFT_SIZE).cloned().collect();
            state.amplitude = samples.iter().map(|s| s.abs()).sum::<f32>() / samples.len() as f32;
        }

        // FFT Processing
        if samples.len() >= FFT_SIZE {
            let mut input = samples[..FFT_SIZE].to_vec();
            // Apply window
            for (i, s) in input.iter_mut().enumerate() {
                *s *= self.window[i];
            }

            let mut output = self.fft_processor.make_output_vec();
            if let Ok(_) = self.fft_processor.process(&mut input, &mut output) {
                let mut bins: Vec<f32> = output.iter().map(|c| c.norm()).collect();
                
                // Apply Kalman smoothing
                self.apply_kalman(&mut bins);

                // Chromagram calculation (simplified)
                let chromagram = self.calculate_chromagram(&bins);

                // Onset detection (simplified energy-based)
                let is_beat = self.detect_beat(&bins);

                let mut state = self.state.write().unwrap();
                state.bins = bins;
                state.chromagram = chromagram;
                state.is_beat = is_beat;
            }
        }
    }

    fn apply_kalman(&mut self, bins: &mut Vec<f32>) {
        let q = 0.1; // Process noise
        let r = 0.5; // Measurement noise

        for i in 0..bins.len().min(self.kalman_states.len()) {
            // Prediction
            let p = self.kalman_p[i] + q;
            
            // Update
            let k = p / (p + r);
            self.kalman_states[i] = self.kalman_states[i] + k * (bins[i] - self.kalman_states[i]);
            self.kalman_p[i] = (1.0 - k) * p;
            
            bins[i] = self.kalman_states[i];
        }
    }

    fn calculate_chromagram(&self, bins: &[f32]) -> [f32; 12] {
        let mut chroma = [0.0f32; 12];
        let sample_rate = 44100.0; // Assumption, should be passed
        
        for (i, &mag) in bins.iter().enumerate() {
            let freq = i as f32 * sample_rate / FFT_SIZE as f32;
            if freq > 20.0 && freq < 4000.0 {
                let midi = 69.0 + 12.0 * (freq / 440.0).log2();
                let note = (midi.round() as i32 % 12).abs() as usize;
                chroma[note % 12] += mag;
            }
        }
        
        // Normalize
        let max = chroma.iter().fold(0.0f32, |a, &b| a.max(b));
        if max > 0.0 {
            for val in chroma.iter_mut() {
                *val /= max;
            }
        }
        chroma
    }

    fn detect_beat(&self, bins: &[f32]) -> bool {
        // Very simple onset detection: check low frequency energy
        let low_energy: f32 = bins.iter().take(10).sum();
        low_energy > 5.0 // Threshold should be adaptive (Particle filter candidate)
    }
}
