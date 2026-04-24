use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;
use std::sync::RwLock;
use wide::f32x8;

pub const FFT_SIZE: usize = 2048;

/// Fast bitwise XOR-based absolute value for f32
#[inline(always)]
fn xor_abs(f: f32) -> f32 {
    f32::from_bits(f.to_bits() & 0x7FFFFFFF)
}

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
    
    // Pre-allocated buffers to avoid allocations in the audio path
    fft_input: Vec<f32>,
    fft_output: Vec<realfft::num_complex::Complex<f32>>,

    // Adaptive Beat Detection
    energy_history: Vec<f32>,
    energy_avg: f32,
}

impl AudioAnalyzer {
    pub fn new() -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_processor = planner.plan_fft_forward(FFT_SIZE);
        let fft_output = fft_processor.make_output_vec();

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
            fft_input: vec![0.0; FFT_SIZE],
            fft_output,
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
            
            // Copy waveform efficiently
            let to_copy = std::cmp::min(samples.len(), FFT_SIZE);
            state.waveform[..to_copy].copy_from_slice(&samples[..to_copy]);
            
            // SIMD-accelerated amplitude calculation
            let mut sum_simd = f32x8::ZERO;
            let chunks = samples.chunks_exact(8);
            let remainder = chunks.remainder();
            
            for chunk in chunks {
                let v = f32x8::from(chunk);
                sum_simd += v.abs();
            }
            
            let mut sum = sum_simd.reduce_add();
            for &s in remainder {
                sum += xor_abs(s);
            }
            
            current_amplitude = sum / samples.len() as f32;
            // Exponential moving average for smoothness
            state.amplitude = (state.amplitude * 0.8) + (current_amplitude * 0.2);
        }

        if samples.len() >= FFT_SIZE {
            // SIMD-accelerated windowing
            let s_chunks = samples[..FFT_SIZE].chunks_exact(8);
            let w_chunks = self.window.chunks_exact(8);
            
            for (i, (s_c, w_c)) in s_chunks.zip(w_chunks).enumerate() {
                let s = f32x8::from(s_c);
                let w = f32x8::from(w_c);
                let res = s * w;
                self.fft_input[i * 8..(i + 1) * 8].copy_from_slice(&res.to_array());
            }

            // Process FFT using pre-allocated output buffer
            if self.fft_processor.process(&mut self.fft_input, &mut self.fft_output).is_ok() {
                let is_beat = self.detect_beat_adaptive(current_amplitude);
                
                let mut state = self.state.write().unwrap();
                state.is_beat = is_beat;
                
                // Update spectrum in place to avoid allocation
                for i in 0..FFT_SIZE / 2 {
                    let c = self.fft_output[i];
                    state.spectrum[i] = (c.re * c.re + c.im * c.im).sqrt();
                }
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
        
        // Branchless-style comparison (result is bool)
        (amplitude > self.energy_avg * 1.6) & (amplitude > 0.05)
    }
}
