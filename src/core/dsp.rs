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
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub is_beat: bool,
}

impl Default for DspState {
    fn default() -> Self {
        Self {
            waveform: vec![0.0; FFT_SIZE],
            spectrum: vec![0.0; FFT_SIZE / 2],
            amplitude: 0.0,
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            is_beat: false,
        }
    }
}

pub struct AudioAnalyzer {
    fft_processor: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    pub state: Arc<RwLock<DspState>>,
    sample_rate: f32,
    
    // Pre-allocated buffers to avoid allocations in the audio path
    fft_input: Vec<f32>,
    fft_output: Vec<realfft::num_complex::Complex<f32>>,

    // Adaptive Beat Detection
    energy_history: Vec<f32>,
    energy_idx: usize,
    energy_sum: f32,
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
            sample_rate: 48000.0,
            fft_input: vec![0.0; FFT_SIZE],
            fft_output,
            energy_history: vec![0.0; 64],
            energy_idx: 0,
            energy_sum: 0.0,
            energy_avg: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
    }

    pub fn process_samples(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        let current_amplitude;
        let samples_len_inv = 1.0 / samples.len() as f32;

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
            
            current_amplitude = sum * samples_len_inv;
            
            // Fixed-point EMA: state.amplitude = (state.amplitude * 0.8) + (current_amplitude * 0.2)
            // Scale by 2^16 (65536) and use i64 for safety
            // 0.8 * 65536 approx 52429
            // 0.2 * 65536 approx 13107
            if current_amplitude.is_finite() {
                let amp_fixed = (state.amplitude.clamp(0.0, 1e6) * 65536.0) as i64;
                let cur_amp_fixed = (current_amplitude.clamp(0.0, 1e6) * 65536.0) as i64;
                let next_amp_fixed = (amp_fixed * 52429 + cur_amp_fixed * 13107) >> 16;
                state.amplitude = next_amp_fixed as f32 * 0.0000152587890625; // Multiply by 1/65536
            } else {
                state.amplitude = current_amplitude;
            }
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
                let spec_half_len = FFT_SIZE >> 1; 
                for i in 0..spec_half_len {
                    let c = self.fft_output[i];
                    state.spectrum[i] = (c.re * c.re + c.im * c.im).sqrt();
                }

                // Energy band calculation
                let bin_resolution = self.sample_rate / (FFT_SIZE as f32);
                let bass_cutoff = (250.0 / bin_resolution) as usize;
                let mid_cutoff = (4000.0 / bin_resolution) as usize;
                let spec_len = state.spectrum.len();
                let bass_end = bass_cutoff.min(spec_len);
                let mid_end = mid_cutoff.min(spec_len);

                let bass_sum: f32 = state.spectrum[..bass_end].iter().sum();
                let mid_sum: f32 = state.spectrum[bass_end..mid_end].iter().sum();
                let treble_sum: f32 = state.spectrum[mid_end..].iter().sum();

                // Fixed-point smoothing for bands
                if bass_sum.is_finite() && mid_sum.is_finite() && treble_sum.is_finite() {
                    let bass_fixed = (state.bass.clamp(0.0, 1e6) * 65536.0) as i64;
                    let cur_bass_fixed = (bass_sum.clamp(0.0, 1e6) * 65536.0) as i64;
                    state.bass = ((bass_fixed * 52429 + cur_bass_fixed * 13107) >> 16) as f32 * 0.0000152587890625;

                    let mid_fixed = (state.mid.clamp(0.0, 1e6) * 65536.0) as i64;
                    let cur_mid_fixed = (mid_sum.clamp(0.0, 1e6) * 65536.0) as i64;
                    state.mid = ((mid_fixed * 52429 + cur_mid_fixed * 13107) >> 16) as f32 * 0.0000152587890625;

                    let treble_fixed = (state.treble.clamp(0.0, 1e6) * 65536.0) as i64;
                    let cur_treble_fixed = (treble_sum.clamp(0.0, 1e6) * 65536.0) as i64;
                    state.treble = ((treble_fixed * 52429 + cur_treble_fixed * 13107) >> 16) as f32 * 0.0000152587890625;
                } else {
                    state.bass = bass_sum;
                    state.mid = mid_sum;
                    state.treble = treble_sum;
                }
            }
        }
    }

    fn detect_beat_adaptive(&mut self, amplitude: f32) -> bool {
        // Use fixed-point for history to avoid precision drift
        let amp_fixed = (amplitude.clamp(0.0, 1e6) * 65536.0) as i64;
        let old_fixed = (self.energy_history[self.energy_idx] * 65536.0) as i64;
        
        let sum_fixed = (self.energy_sum * 65536.0) as i64;
        let next_sum_fixed = sum_fixed - old_fixed + amp_fixed;
        
        self.energy_history[self.energy_idx] = amplitude;
        self.energy_sum = next_sum_fixed as f32 * 0.0000152587890625;
        
        self.energy_idx = (self.energy_idx + 1) & 63;
        
        // Fast reciprocal multiply (1/64)
        self.energy_avg = self.energy_sum * 0.015625;
        
        // Branchless-style comparison
        (amplitude > self.energy_avg * 1.6) & (amplitude > 0.05)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xor_abs() {
        // Happy path
        assert_eq!(xor_abs(1.5), 1.5);
        assert_eq!(xor_abs(-1.5), 1.5);
        
        // Edge cases
        assert_eq!(xor_abs(0.0), 0.0);
        assert_eq!(xor_abs(-0.0), 0.0);
        
        // Extreme values
        assert_eq!(xor_abs(std::f32::INFINITY), std::f32::INFINITY);
        assert_eq!(xor_abs(std::f32::NEG_INFINITY), std::f32::INFINITY);
        
        // NaN behavior (sign bit cleared)
        let nan_abs = xor_abs(std::f32::NAN);
        assert!(nan_abs.is_nan());
        assert!(nan_abs.is_sign_positive());
        
        let neg_nan_abs = xor_abs(-std::f32::NAN);
        assert!(neg_nan_abs.is_nan());
        assert!(neg_nan_abs.is_sign_positive());
    }

    #[test]
    fn test_audio_analyzer_empty_samples() {
        let mut analyzer = AudioAnalyzer::new();
        analyzer.process_samples(&[]);
        
        let state = analyzer.state.read().unwrap();
        assert_eq!(state.amplitude, 0.0);
        assert!(!state.is_beat);
    }

    #[test]
    fn test_audio_analyzer_small_buffer() {
        let mut analyzer = AudioAnalyzer::new();
        let samples = vec![0.5; 100]; // Smaller than FFT_SIZE
        analyzer.process_samples(&samples);
        
        let state = analyzer.state.read().unwrap();
        // Amplitude should be updated (EMA logic: 0.0 * 0.8 + 0.5 * 0.2 = 0.1)
        assert!((state.amplitude - 0.1).abs() < 1e-3);
        assert!(!state.is_beat);
        
        // Check waveform copy (only first 100 samples copied)
        assert_eq!(state.waveform[0..100], vec![0.5; 100]);
        // Rest should be default 0.0
        assert_eq!(state.waveform[100..200], vec![0.0; 100]);
    }

    #[test]
    fn test_audio_analyzer_exact_fft_size() {
        let mut analyzer = AudioAnalyzer::new();
        let samples = vec![1.0; FFT_SIZE];
        analyzer.process_samples(&samples);
        
        let state = analyzer.state.read().unwrap();
        // EMA logic: 0.0 * 0.8 + 1.0 * 0.2 = 0.2
        assert!((state.amplitude - 0.2).abs() < 1e-3);
        
        // Check waveform copy
        assert_eq!(state.waveform, samples);
    }

    #[test]
    fn test_audio_analyzer_large_buffer() {
        let mut analyzer = AudioAnalyzer::new();
        let samples = vec![0.5; FFT_SIZE + 1000];
        analyzer.process_samples(&samples);
        
        let state = analyzer.state.read().unwrap();
        assert!((state.amplitude - 0.1).abs() < 1e-3);
        
        // Only first FFT_SIZE elements should be copied into waveform
        assert_eq!(state.waveform, vec![0.5; FFT_SIZE]);
    }

    #[test]
    fn test_audio_analyzer_all_zeroes() {
        let mut analyzer = AudioAnalyzer::new();
        let samples = vec![0.0; FFT_SIZE * 2];
        analyzer.process_samples(&samples);
        
        let state = analyzer.state.read().unwrap();
        assert_eq!(state.amplitude, 0.0);
        assert!(!state.is_beat);
        
        for &s in state.spectrum.iter() {
            assert_eq!(s, 0.0);
        }
    }

    #[test]
    fn test_detect_beat_adaptive() {
        let mut analyzer = AudioAnalyzer::new();
        
        // Feed low energy to build history
        for _ in 0..65 {
            analyzer.detect_beat_adaptive(0.01);
        }
        
        // Check that a sudden spike causes a beat
        let is_beat = analyzer.detect_beat_adaptive(0.8);
        assert!(is_beat);
        
        // Check that a sustained high energy does not continuously trigger beats (requires spike > 1.6x avg)
        for _ in 0..65 {
            analyzer.detect_beat_adaptive(0.8);
        }
        let is_beat_sustained = analyzer.detect_beat_adaptive(0.8);
        assert!(!is_beat_sustained);
    }

    #[test]
    fn test_simd_amplitude_calculation() {
        // Create an unaligned size to test chunks and remainder
        let mut analyzer = AudioAnalyzer::new();
        let mut samples = vec![0.0; 13]; // 13 is not divisible by 8
        for i in 0..13 {
            samples[i] = if i % 2 == 0 { 1.0 } else { -1.0 };
        }
        
        analyzer.process_samples(&samples);
        
        let state = analyzer.state.read().unwrap();
        // All absolute values are 1.0, so avg is 1.0
        // EMA: 0.0 * 0.8 + 1.0 * 0.2 = 0.2
        assert!((state.amplitude - 0.2).abs() < 1e-3);
    }

    #[test]
    fn test_spectrum_peaks_mixed_sines() {
        let mut analyzer = AudioAnalyzer::new();
        let sr = 44100.0;
        let f1 = 440.0;
        let f2 = 1200.0;
        let mut samples = vec![0.0; FFT_SIZE];
        for i in 0..FFT_SIZE {
            let t = i as f32 / sr;
            samples[i] = (2.0 * std::f32::consts::PI * f1 * t).sin() * 0.5 
                       + (2.0 * std::f32::consts::PI * f2 * t).sin() * 0.3;
        }
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        
        let bin1 = (f1 * FFT_SIZE as f32 / sr).round() as usize;
        let bin2 = (f2 * FFT_SIZE as f32 / sr).round() as usize;
        
        // Check that peaks exist near the expected bins
        assert!(state.spectrum[bin1] > 0.1);
        assert!(state.spectrum[bin2] > 0.05);
    }

    #[test]
    fn test_spectrum_peaks_three_sines() {
        let mut analyzer = AudioAnalyzer::new();
        let sr = 44100.0;
        let freqs = [300.0, 1000.0, 3000.0];
        let mut samples = vec![0.0; FFT_SIZE];
        for i in 0..FFT_SIZE {
            let t = i as f32 / sr;
            for &f in &freqs {
                samples[i] += (2.0 * std::f32::consts::PI * f * t).sin() * 0.2;
            }
        }
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        
        for &f in &freqs {
            let bin = (f * FFT_SIZE as f32 / sr).round() as usize;
            assert!(state.spectrum[bin] > 0.01, "Missing peak at {} Hz", f);
        }
    }

    #[test]
    fn test_spectrum_sine_with_noise() {
        let mut analyzer = AudioAnalyzer::new();
        let sr = 44100.0;
        let f = 500.0;
        let mut samples = vec![0.0; FFT_SIZE];
        for i in 0..FFT_SIZE {
            let t = i as f32 / sr;
            // Deterministic "noise"
            let noise = ((i * 1103515245 + 12345) % 65536) as f32 / 65536.0 - 0.5;
            samples[i] = (2.0 * std::f32::consts::PI * f * t).sin() * 0.5 + noise * 0.1;
        }
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        let bin = (f * FFT_SIZE as f32 / sr).round() as usize;
        assert!(state.spectrum[bin] > 0.1);
    }

    #[test]
    fn test_spectrum_close_frequencies() {
        let mut analyzer = AudioAnalyzer::new();
        let sr = 44100.0;
        let f1 = 440.0;
        let f2 = 460.0; // Very close
        let mut samples = vec![0.0; FFT_SIZE];
        for i in 0..FFT_SIZE {
            let t = i as f32 / sr;
            samples[i] = (2.0 * std::f32::consts::PI * f1 * t).sin() * 0.5 
                       + (2.0 * std::f32::consts::PI * f2 * t).sin() * 0.5;
        }
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        let bin = (450.0 * FFT_SIZE as f32 / sr).round() as usize;
        assert!(state.spectrum[bin] > 0.1);
    }

    #[test]
    fn test_sudden_silence_ema_decay() {
        let mut analyzer = AudioAnalyzer::new();
        // Loud noise
        let samples = vec![1.0; FFT_SIZE];
        analyzer.process_samples(&samples);
        
        let mut last_amplitude = {
            let state = analyzer.state.read().unwrap();
            assert!(state.amplitude > 0.0);
            state.amplitude
        };

        // Silence
        let silence = vec![0.0; FFT_SIZE];
        for _ in 0..50 {
            analyzer.process_samples(&silence);
            let state = analyzer.state.read().unwrap();
            assert!(state.amplitude <= last_amplitude);
            last_amplitude = state.amplitude;
        }
        assert!(last_amplitude < 0.001);
    }

    #[test]
    fn test_alternating_loud_silent() {
        let mut analyzer = AudioAnalyzer::new();
        let loud = vec![1.0; FFT_SIZE];
        let silent = vec![0.0; FFT_SIZE];
        
        for _ in 0..10 {
            analyzer.process_samples(&loud);
            let amp_loud = analyzer.state.read().unwrap().amplitude;
            analyzer.process_samples(&silent);
            let amp_silent = analyzer.state.read().unwrap().amplitude;
            assert!(amp_loud > amp_silent);
        }
    }

    #[test]
    fn test_ema_response_to_constant_signal() {
        let mut analyzer = AudioAnalyzer::new();
        let samples = vec![0.5; FFT_SIZE];
        for _ in 0..100 {
            analyzer.process_samples(&samples);
        }
        let state = analyzer.state.read().unwrap();
        // EMA should converge to the signal amplitude (0.5)
        assert!((state.amplitude - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_high_volume_clipping() {
        let mut analyzer = AudioAnalyzer::new();
        let samples = vec![2.0; FFT_SIZE]; // "Clipped" or high volume
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        // EMA: 0.0 * 0.8 + 2.0 * 0.2 = 0.4
        assert!((state.amplitude - 0.4).abs() < 1e-4);
    }

    #[test]
    fn test_extremely_high_volume() {
        let mut analyzer = AudioAnalyzer::new();
        let samples = vec![1e6; FFT_SIZE];
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        assert!(state.amplitude > 1e5);
    }

    #[test]
    fn test_infinity_in_signal() {
        let mut analyzer = AudioAnalyzer::new();
        let mut samples = vec![0.0; FFT_SIZE];
        samples[0] = std::f32::INFINITY;
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        assert!(state.amplitude.is_infinite());
    }

    #[test]
    fn test_impulse_near_start() {
        let mut analyzer = AudioAnalyzer::new();
        let mut samples = vec![0.0; FFT_SIZE];
        samples[1] = 1.0; // Index 0 is zeroed by Hann window
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        let total_energy: f32 = state.spectrum.iter().sum();
        assert!(total_energy > 0.0);
    }

    #[test]
    fn test_impulse_at_middle() {
        let mut analyzer = AudioAnalyzer::new();
        let mut samples = vec![0.0; FFT_SIZE];
        samples[FFT_SIZE / 2] = 1.0;
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        let total_energy: f32 = state.spectrum.iter().sum();
        assert!(total_energy > 0.0);
    }

    #[test]
    fn test_multiple_impulses() {
        let mut analyzer = AudioAnalyzer::new();
        let mut samples = vec![0.0; FFT_SIZE];
        for i in (0..FFT_SIZE).step_by(100) {
            samples[i] = 1.0;
        }
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        assert!(state.amplitude > 0.0);
    }

    #[test]
    fn test_long_processing_stability_noise() {
        let mut analyzer = AudioAnalyzer::new();
        for j in 0..1000 {
            let samples: Vec<f32> = (0..FFT_SIZE).map(|i| {
                (((i + j) * 1103515245 + 12345) % 65536) as f32 / 65536.0
            }).collect();
            analyzer.process_samples(&samples);
        }
        let state = analyzer.state.read().unwrap();
        assert!(!state.amplitude.is_nan());
        assert!(!state.spectrum[0].is_nan());
    }

    #[test]
    fn test_long_processing_silence() {
        let mut analyzer = AudioAnalyzer::new();
        let silence = vec![0.0; FFT_SIZE];
        for _ in 0..1000 {
            analyzer.process_samples(&silence);
        }
        let state = analyzer.state.read().unwrap();
        assert_eq!(state.amplitude, 0.0);
    }

    #[test]
    fn test_long_processing_alternating_signals() {
        let mut analyzer = AudioAnalyzer::new();
        let s1 = vec![0.1; FFT_SIZE];
        let s2 = vec![0.9; FFT_SIZE];
        for i in 0..1000 {
            if i % 2 == 0 {
                analyzer.process_samples(&s1);
            } else {
                analyzer.process_samples(&s2);
            }
        }
        let state = analyzer.state.read().unwrap();
        assert!(state.amplitude > 0.1 && state.amplitude < 0.9);
    }

    #[test]
    fn test_cumulative_error_check() {
        let mut analyzer = AudioAnalyzer::new();
        let samples = vec![0.5; FFT_SIZE];
        for _ in 0..5000 {
            analyzer.process_samples(&samples);
        }
        let state = analyzer.state.read().unwrap();
        // Should have stabilized exactly at 0.5 or very close
        assert!((state.amplitude - 0.5).abs() < 0.0001);
    }

    #[test]
    fn test_different_sample_frequencies_behavior() {
        let mut analyzer = AudioAnalyzer::new();
        // Case 1: 44.1kHz, 1kHz sine
        let f = 1000.0;
        let mut samples = vec![0.0; FFT_SIZE];
        for i in 0..FFT_SIZE {
            samples[i] = (2.0 * std::f32::consts::PI * f * i as f32 / 44100.0).sin();
        }
        analyzer.process_samples(&samples);
        let bin_441 = (f * FFT_SIZE as f32 / 44100.0).round() as usize;
        let val_441 = analyzer.state.read().unwrap().spectrum[bin_441];

        // Case 2: 48kHz, 1kHz sine
        for i in 0..FFT_SIZE {
            samples[i] = (2.0 * std::f32::consts::PI * f * i as f32 / 48000.0).sin();
        }
        analyzer.process_samples(&samples);
        let bin_480 = (f * FFT_SIZE as f32 / 48000.0).round() as usize;
        let val_480 = analyzer.state.read().unwrap().spectrum[bin_480];

        assert!(val_441 > 0.1);
        assert!(val_480 > 0.1);
        assert_ne!(bin_441, bin_480);
    }

    #[test]
    fn test_nyquist_frequency() {
        let mut analyzer = AudioAnalyzer::new();
        let mut samples = vec![0.0; FFT_SIZE];
        for i in 0..FFT_SIZE {
            samples[i] = if i % 2 == 0 { 1.0 } else { -1.0 };
        }
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        // Nyquist is at the last bin
        assert!(state.spectrum[FFT_SIZE / 2 - 1] > 0.1);
    }

    #[test]
    fn test_dc_component() {
        let mut analyzer = AudioAnalyzer::new();
        let samples = vec![1.0; FFT_SIZE];
        analyzer.process_samples(&samples);
        let state = analyzer.state.read().unwrap();
        // DC is at bin 0
        assert!(state.spectrum[0] > 0.1);
    }

    #[test]
    fn test_one_sample_buffer() {
        let mut analyzer = AudioAnalyzer::new();
        analyzer.process_samples(&[1.0]);
        let state = analyzer.state.read().unwrap();
        assert!(state.amplitude > 0.0);
        assert_eq!(state.waveform[0], 1.0);
    }

    #[test]
    fn test_seven_samples_buffer() {
        let mut analyzer = AudioAnalyzer::new();
        analyzer.process_samples(&[1.0; 7]);
        let state = analyzer.state.read().unwrap();
        assert!(state.amplitude > 0.0);
        assert_eq!(state.waveform[0..7], [1.0; 7]);
    }

    #[test]
    fn test_nine_samples_buffer() {
        let mut analyzer = AudioAnalyzer::new();
        analyzer.process_samples(&[1.0; 9]);
        let state = analyzer.state.read().unwrap();
        assert!(state.amplitude > 0.0);
        assert_eq!(state.waveform[0..9], [1.0; 9]);
    }

    #[test]
    fn test_1023_samples_buffer() {
        let mut analyzer = AudioAnalyzer::new();
        analyzer.process_samples(&[0.5; 1023]);
        let state = analyzer.state.read().unwrap();
        assert!(state.amplitude > 0.0);
        assert_eq!(state.waveform[1022], 0.5);
        assert_eq!(state.waveform[1023], 0.0);
    }

    #[test]
    fn test_fft_consistency() {
        let mut analyzer = AudioAnalyzer::new();
        let samples = vec![0.5; FFT_SIZE];
        analyzer.process_samples(&samples);
        let spec1 = analyzer.state.read().unwrap().spectrum.clone();
        
        analyzer.process_samples(&samples);
        let spec2 = analyzer.state.read().unwrap().spectrum.clone();
        
        assert_eq!(spec1, spec2);
    }

    #[test]
    fn test_fft_reset_behavior() {
        let mut analyzer = AudioAnalyzer::new();
        let samples1 = vec![0.1; FFT_SIZE];
        let samples2 = vec![0.9; FFT_SIZE];
        
        analyzer.process_samples(&samples1);
        let spec1 = analyzer.state.read().unwrap().spectrum.clone();
        
        analyzer.process_samples(&samples2);
        let spec2 = analyzer.state.read().unwrap().spectrum.clone();
        
        assert_ne!(spec1, spec2);
    }

    #[test]
    fn test_thread_safety_read_while_process() {
        use std::thread;
        let analyzer = Arc::new(RwLock::new(AudioAnalyzer::new()));
        let analyzer_clone = Arc::clone(&analyzer);
        
        let t1 = thread::spawn(move || {
            for _ in 0..100 {
                let mut a = analyzer_clone.write().unwrap();
                a.process_samples(&vec![0.5; FFT_SIZE]);
            }
        });
        
        let analyzer_clone2 = Arc::clone(&analyzer);
        let t2 = thread::spawn(move || {
            for _ in 0..200 {
                let a = analyzer_clone2.read().unwrap();
                let _amp = a.state.read().unwrap().amplitude;
            }
        });
        
        t1.join().unwrap();
        t2.join().unwrap();
    }

    #[test]
    fn test_thread_safety_concurrent_analyzers() {
        use std::thread;
        let mut threads = vec![];
        for _ in 0..4 {
            threads.push(thread::spawn(|| {
                let mut analyzer = AudioAnalyzer::new();
                for _ in 0..100 {
                    analyzer.process_samples(&vec![0.1; FFT_SIZE]);
                }
            }));
        }
        for t in threads {
            t.join().unwrap();
        }
    }

    #[test]
    fn test_rapid_state_access() {
        use std::thread;
        let mut analyzer = AudioAnalyzer::new();
        let state = Arc::clone(&analyzer.state);
        
        let t1 = thread::spawn(move || {
            for _ in 0..500 {
                let _s = state.read().unwrap();
            }
        });
        
        for _ in 0..100 {
            analyzer.process_samples(&vec![0.2; FFT_SIZE]);
        }
        
        t1.join().unwrap();
    }

    #[test]
    fn test_amplitude_ema_large_step_up() {
        let mut analyzer = AudioAnalyzer::new();
        // Start with silence
        analyzer.process_samples(&vec![0.0; FFT_SIZE]);
        assert_eq!(analyzer.state.read().unwrap().amplitude, 0.0);
        
        // Sudden loud sound
        analyzer.process_samples(&vec![1.0; FFT_SIZE]);
        let amp1 = analyzer.state.read().unwrap().amplitude;
        // EMA: 0.0 * 0.8 + 1.0 * 0.2 = 0.2
        assert!((amp1 - 0.2).abs() < 1e-3);
        
        analyzer.process_samples(&vec![1.0; FFT_SIZE]);
        let amp2 = analyzer.state.read().unwrap().amplitude;
        // EMA: 0.2 * 0.8 + 1.0 * 0.2 = 0.16 + 0.2 = 0.36
        assert!((amp2 - 0.36).abs() < 1e-3);
    }
}
