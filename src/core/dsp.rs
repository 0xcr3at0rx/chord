use std::sync::Arc;
use realfft::{RealFftPlanner, RealToComplex};
use std::sync::RwLock;

pub const FFT_SIZE: usize = 2048;
pub const NUM_BANDS: usize = 64;

#[derive(Debug, Clone)]
pub struct DspState {
    pub bins: Vec<f32>,
    pub bands: Vec<f32>,
    pub peaks: Vec<f32>,
    pub waveform: Vec<f32>,
    pub amplitude: f32,
    pub is_beat: bool,
    pub chromagram: [f32; 12],
}

impl Default for DspState {
    fn default() -> Self {
        Self {
            bins: vec![0.0; FFT_SIZE / 2],
            bands: vec![0.0; NUM_BANDS],
            peaks: vec![0.0; NUM_BANDS],
            waveform: vec![0.0; FFT_SIZE],
            amplitude: 0.0,
            is_beat: false,
            chromagram: [0.0; 12],
        }
    }
}

pub struct AudioAnalyzer {
    fft_processor: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    pub state: Arc<RwLock<DspState>>,
    
    // Smoothing & Gravity
    kalman_states: Vec<f32>,
    kalman_p: Vec<f32>,
    
    // Adaptive Beat Detection
    energy_history: Vec<f32>,
    energy_avg: f32,
}

impl AudioAnalyzer {
    pub fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_processor = planner.plan_fft_forward(FFT_SIZE);
        
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE as f32 - 1.0)).cos()))
            .collect();

        Self {
            fft_processor,
            window,
            state: Arc::new(RwLock::new(DspState::default())),
            kalman_states: vec![0.0; FFT_SIZE / 2 + 1],
            kalman_p: vec![1.0; FFT_SIZE / 2 + 1],
            energy_history: Vec::with_capacity(43), // ~2 seconds of history at 46ms intervals
            energy_avg: 0.0,
        }
    }

    pub fn process_samples(&mut self, samples: &[f32]) {
        if samples.is_empty() { return; }

        let current_amplitude;
        // Update waveform and amplitude
        {
            let mut state = self.state.write().unwrap();
            state.waveform = samples.iter().take(FFT_SIZE).cloned().collect();
            current_amplitude = samples.iter().map(|s| s.abs()).sum::<f32>() / samples.len() as f32;
            state.amplitude = current_amplitude;
        }

        if samples.len() >= FFT_SIZE {
            let mut input = samples[..FFT_SIZE].to_vec();
            for (i, s) in input.iter_mut().enumerate() {
                *s *= self.window[i];
            }

            let mut output = self.fft_processor.make_output_vec();
            if let Ok(_) = self.fft_processor.process(&mut input, &mut output) {
                let mut bins: Vec<f32> = output.iter().map(|c| c.norm()).collect();
                
                self.apply_kalman(&mut bins);
                
                let new_bands = self.calculate_log_bands(&bins);
                let is_beat = self.detect_beat_adaptive(current_amplitude);
                let chromagram = self.calculate_chromagram(&bins);

                let mut state = self.state.write().unwrap();
                state.bins = bins;
                state.chromagram = chromagram;
                state.is_beat = is_beat;
                
                // Update bands with gravity
                for i in 0..NUM_BANDS {
                    let val = new_bands[i];
                    // Exponential smoothing on the bands themselves
                    state.bands[i] = (state.bands[i] * 0.6) + (val * 0.4);
                    
                    // Slower falloff for peaks
                    state.peaks[i] = (state.peaks[i] * 0.97).max(val);
                }
            }
        }
    }

    fn calculate_log_bands(&self, bins: &[f32]) -> Vec<f32> {
        let mut bands = vec![0.0; NUM_BANDS];
        let num_bins = bins.len();
        
        for i in 0..NUM_BANDS {
            // More aggressive logarithmic scaling to emphasize bass/mids
            let start = (num_bins as f32 * (i as f32 / NUM_BANDS as f32).powf(2.5)).floor() as usize;
            let end = (num_bins as f32 * ((i + 1) as f32 / NUM_BANDS as f32).powf(2.5)).ceil() as usize;
            let end = end.clamp(start + 1, num_bins);
            
            let sum: f32 = bins[start..end].iter().sum();
            bands[i] = sum / (end - start) as f32;
        }
        bands
    }

    fn apply_kalman(&mut self, bins: &mut Vec<f32>) {
        let q = 0.02; // Smoother
        let r = 0.9;  // Smoother

        for i in 0..bins.len().min(self.kalman_states.len()) {
            let p = self.kalman_p[i] + q;
            let k = p / (p + r);
            self.kalman_states[i] = self.kalman_states[i] + k * (bins[i] - self.kalman_states[i]);
            self.kalman_p[i] = (1.0 - k) * p;
            bins[i] = self.kalman_states[i];
        }
    }

    fn calculate_chromagram(&self, bins: &[f32]) -> [f32; 12] {
        let mut chroma = [0.0f32; 12];
        let sample_rate = 44100.0;
        
        for (i, &mag) in bins.iter().enumerate() {
            let freq = i as f32 * sample_rate / FFT_SIZE as f32;
            if freq > 20.0 && freq < 4000.0 {
                let midi = 69.0 + 12.0 * (freq / 440.0).log2();
                let note = (midi.round() as i32 % 12).abs() as usize;
                chroma[note % 12] += mag;
            }
        }
        
        let max = chroma.iter().fold(0.0f32, |a, &b| a.max(b));
        if max > 0.0 {
            for val in chroma.iter_mut() {
                *val /= max;
            }
        }
        chroma
    }

    fn detect_beat_adaptive(&mut self, amplitude: f32) -> bool {
        if self.energy_history.len() >= 43 {
            self.energy_history.remove(0);
        }
        self.energy_history.push(amplitude);
        
        self.energy_avg = self.energy_history.iter().sum::<f32>() / self.energy_history.len() as f32;
        
        // Beat is detected if current amplitude is significantly higher than average
        amplitude > self.energy_avg * 1.5 && amplitude > 0.05
    }
}
