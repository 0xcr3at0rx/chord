use anyhow::Result;
use rodio::cpal::traits::{HostTrait, DeviceTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

/// A wrapper for Read types that implements Seek by allowing only forward seeks (and limited backward seeks if we buffer).
/// This is required for rodio::Decoder to work with network streams.
struct StreamingReader<R: Read> {
    inner: R,
    pos: u64,
    header_buffer: Vec<u8>,
    header_read_pos: usize,
}

impl<R: Read> StreamingReader<R> {
    fn new(mut inner: R) -> Self {
        let mut header_buffer = Vec::with_capacity(1048576); // 1MB buffer for headers
        let mut temp_buf = [0u8; 16384];
        for _ in 0..64 {
            // Read up to 1MB in chunks for extremely reliable detection
            match inner.read(&mut temp_buf) {
                Ok(0) => break,
                Ok(n) => header_buffer.extend_from_slice(&temp_buf[..n]),
                Err(_) => break,
            }
        }

        Self {
            inner,
            pos: header_buffer.len() as u64,
            header_buffer,
            header_read_pos: 0,
        }
    }
}

impl<R: Read> Read for StreamingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.header_read_pos < self.header_buffer.len() {
            let n = std::cmp::min(buf.len(), self.header_buffer.len() - self.header_read_pos);
            buf[..n].copy_from_slice(
                &self.header_buffer[self.header_read_pos..self.header_read_pos + n],
            );
            self.header_read_pos += n;
            Ok(n)
        } else {
            let n = self.inner.read(buf)?;
            self.pos += n as u64;
            Ok(n)
        }
    }
}

impl<R: Read> Seek for StreamingReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let current_total_pos = if self.header_read_pos < self.header_buffer.len() {
            self.header_read_pos as u64
        } else {
            self.pos
        };

        match pos {
            SeekFrom::Start(n) => {
                if n < self.header_buffer.len() as u64 {
                    self.header_read_pos = n as usize;
                    Ok(n)
                } else if n == current_total_pos {
                    Ok(current_total_pos)
                } else if n > self.pos {
                    let to_skip = n - self.pos;
                    std::io::copy(&mut self.inner.by_ref().take(to_skip), &mut std::io::sink())?;
                    self.pos = n;
                    self.header_read_pos = self.header_buffer.len();
                    Ok(self.pos)
                } else {
                    // Can't seek back beyond the header buffer
                    Ok(current_total_pos)
                }
            }
            SeekFrom::Current(n) => {
                if n == 0 {
                    return Ok(current_total_pos);
                }

                if n < 0 {
                    let back = (-n) as u64;
                    if back <= current_total_pos
                        && current_total_pos - back < self.header_buffer.len() as u64
                    {
                        self.header_read_pos = (current_total_pos - back) as usize;
                        Ok(current_total_pos - back)
                    } else {
                        Ok(current_total_pos)
                    }
                } else {
                    let n_u64 = n as u64;
                    if self.header_read_pos + n_u64 as usize <= self.header_buffer.len() {
                        self.header_read_pos += n_u64 as usize;
                        Ok(self.header_read_pos as u64)
                    } else {
                        let remaining_header =
                            (self.header_buffer.len() - self.header_read_pos) as u64;
                        let to_skip_from_inner = n_u64 - remaining_header;
                        std::io::copy(
                            &mut self.inner.by_ref().take(to_skip_from_inner),
                            &mut std::io::sink(),
                        )?;
                        self.pos += to_skip_from_inner;
                        self.header_read_pos = self.header_buffer.len();
                        Ok(self.pos)
                    }
                }
            }
            SeekFrom::End(_) => Ok(current_total_pos),
        }
    }
}

use crate::core::dsp::{AudioAnalyzer, DspState};
use std::sync::mpsc as channel;

fn suppress_alsa_errors() {
    // Already handled by gag-based redirection in logger.rs
}

#[derive(Clone, Debug)]
pub struct LyricLine {
    pub time: Duration,
    pub text: String,
}

struct VisualizerTracker<S>
where
    S: rodio::Source<Item = f32>,
{
    inner: S,
    sample_tx: channel::SyncSender<f32>,
    amplitude: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl<S> rodio::Source for VisualizerTracker<S>
where
    S: rodio::Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

impl<S> Iterator for VisualizerTracker<S>
where
    S: rodio::Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;
        let val = sample.abs();

        // Send sample to analyzer
        let _ = self.sample_tx.try_send(sample);

        // Simple exponential moving average for smoothing (legacy support)
        let current_bits = self.amplitude.load(std::sync::atomic::Ordering::Relaxed);
        let current = f32::from_bits(current_bits);
        let new_val = current * 0.85 + val * 0.15;
        self.amplitude
            .store(new_val.to_bits(), std::sync::atomic::Ordering::Relaxed);

        Some(sample)
    }
}

enum AudioCmd {
    Init,
    Play(PathBuf),
    PlayStream(String, u64),
    PlayRaw(mpsc::Receiver<Vec<f32>>, u32, u16), // samples_rx, sample_rate, channels
    SetVolume(f32),
    Pause,
    Resume,
    Stop,
    RegisterRadioSink(Sink, u64),
}

struct RawSource {
    samples_rx: mpsc::Receiver<Vec<f32>>,
    current_chunk: Vec<f32>,
    read_pos: usize,
    sample_rate: u32,
    channels: u16,
}

impl Iterator for RawSource {
    type Item = f32;
    fn next(&mut self) -> Option<Self::Item> {
        if self.read_pos >= self.current_chunk.len() {
            match self.samples_rx.try_recv() {
                Ok(chunk) => {
                    self.current_chunk = chunk;
                    self.read_pos = 0;
                }
                Err(_) => return Some(0.0), // Fill with silence if buffer empty
            }
        }
        
        if self.read_pos < self.current_chunk.len() {
            let s = self.current_chunk[self.read_pos];
            self.read_pos += 1;
            Some(s)
        } else {
            Some(0.0)
        }
    }
}

impl rodio::Source for RawSource {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { self.channels }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<Duration> { None }
}

pub struct AudioPlayer {
    cmd_tx: mpsc::Sender<AudioCmd>,
    pub is_empty: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub has_error: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub is_initializing: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub mode: std::sync::Arc<std::sync::Mutex<String>>,
    pub last_error: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    pub amplitude: std::sync::Arc<std::sync::atomic::AtomicU32>,
    pub dsp_state: std::sync::Arc<std::sync::RwLock<DspState>>,
}

struct AudioBackend {
    stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    radio_sink: Option<Sink>,
    volume: f32,
    active_radio_request_id: u64,
    is_empty_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    has_error_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    is_initializing_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    last_error_shared: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    amplitude_shared: std::sync::Arc<std::sync::atomic::AtomicU32>,
    sample_tx: channel::SyncSender<f32>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        suppress_alsa_errors();
        let (tx, rx) = mpsc::channel();
        let (sample_tx, sample_rx) = channel::sync_channel(10000);

        let is_empty = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let has_error = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let is_initializing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mode = std::sync::Arc::new(std::sync::Mutex::new("PIPEWIRE".into()));
        let last_error = std::sync::Arc::new(std::sync::Mutex::new(None));
        let amplitude = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0.0f32.to_bits()));

        let mut analyzer = AudioAnalyzer::new();
        let dsp_state = analyzer.state.clone();

        let backend_empty = is_empty.clone();
        let backend_error = has_error.clone();
        let backend_init = is_initializing.clone();
        let backend_last_err = last_error.clone();
        let backend_amplitude = amplitude.clone();
        let backend_tx = tx.clone();

        // Start Analyzer thread
        std::thread::spawn(move || {
            let mut samples_buf = Vec::with_capacity(2048);
            loop {
                while let Ok(sample) = sample_rx.recv_timeout(Duration::from_millis(10)) {
                    samples_buf.push(sample);
                    if samples_buf.len() >= 2048 {
                        // Sliding window: process current buffer
                        analyzer.process_samples(&samples_buf);

                        // Shift buffer by HOP_SIZE (512)
                        let hop = 512;
                        if samples_buf.len() > hop {
                            samples_buf.drain(0..hop);
                        } else {
                            samples_buf.clear();
                        }
                    }
                }
            }
        });

        let backend_sample_tx_clone = sample_tx.clone();
        std::thread::spawn(move || {
            let mut backend = AudioBackend {
                stream: None,
                handle: None,
                sink: None,
                radio_sink: None,
                volume: 1.0,
                active_radio_request_id: 0,
                is_empty_shared: backend_empty,
                has_error_shared: backend_error,
                is_initializing_shared: backend_init,
                last_error_shared: backend_last_err,
                amplitude_shared: backend_amplitude,
                sample_tx: backend_sample_tx_clone,
            };

            let _ = backend.try_init(false);

            loop {
                let cmd = rx.recv_timeout(Duration::from_millis(50));
                match cmd {
                    Ok(AudioCmd::Init) => {
                        tracing::debug!("AudioCmd::Init received");
                        backend
                            .is_initializing_shared
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        let res = backend.try_init(true);

                        if let Err(e) = &res {
                            *backend.last_error_shared.lock().unwrap() = Some(e.clone());
                        } else {
                            *backend.last_error_shared.lock().unwrap() = None;
                        }
                        backend
                            .has_error_shared
                            .store(res.is_err(), std::sync::atomic::Ordering::Relaxed);
                        backend
                            .is_initializing_shared
                            .store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                    Ok(AudioCmd::Play(path)) => {
                        tracing::debug!(path = ?path, "AudioCmd::Play received");
                        let res = backend.play(path);
                        if let Err(e) = &res {
                            *backend.last_error_shared.lock().unwrap() = Some(e.clone());
                        } else {
                            *backend.last_error_shared.lock().unwrap() = None;
                        }
                        backend
                            .has_error_shared
                            .store(res.is_err(), std::sync::atomic::Ordering::Relaxed);
                    }
                    Ok(AudioCmd::PlayStream(url, request_id)) => {
                        tracing::info!(url = %url, request_id, "AudioCmd::PlayStream received");
                        backend.stop_sink();
                        backend.active_radio_request_id = request_id;

                        // Ensure we have a handle before spawning radio thread
                        if backend.handle.is_none() {
                            let _ = backend.try_init(false);
                        }

                        let tx_clone = backend_tx.clone();
                        let handle_shared = backend.handle.clone();
                        let volume = backend.volume;
                        let last_error_shared = backend.last_error_shared.clone();
                        let has_error_shared = backend.has_error_shared.clone();
                        let amplitude_shared_thread = backend.amplitude_shared.clone();

                        let sample_tx_clone = backend.sample_tx.clone();

                        std::thread::spawn(move || {
                            if let Some(handle) = handle_shared {
                                let mut retry_count = 0;
                                let max_retries = 3;

                                while retry_count < max_retries {
                                    let res = (|| -> Result<Sink, String> {
                                        let response = reqwest::blocking::Client::builder()
                                            .user_agent(
                                                "Chord/1.2 (https://github.com/0xcr3at0rx/chord)",
                                            )
                                            .timeout(Duration::from_secs(15))
                                            .connect_timeout(Duration::from_secs(10))
                                            .redirect(reqwest::redirect::Policy::limited(10))
                                            .build()
                                            .map_err(|e| format!("Client: {}", e))?
                                            .get(&url)
                                            .header("Icy-MetaData", "0")
                                            .header("Connection", "keep-alive")
                                            .send()
                                            .map_err(|e| format!("Request: {}", e))?;

                                        if !response.status().is_success() {
                                            return Err(format!("HTTP {}", response.status()));
                                        }

                                        let reader = StreamingReader::new(response);
                                        let source = Decoder::new(reader)
                                            .map_err(|e| format!("Decoder: {}", e))?;
                                        let source = rodio::Source::convert_samples::<f32>(source);
                                        let source = VisualizerTracker {
                                            inner: source,
                                            amplitude: amplitude_shared_thread.clone(),
                                            sample_tx: sample_tx_clone.clone(),
                                        };
                                        let sink = Sink::try_new(&handle)
                                            .map_err(|e| format!("Sink: {}", e))?;
                                        sink.set_volume(volume);
                                        sink.append(source);
                                        sink.play();
                                        Ok(sink)
                                    })();

                                    match res {
                                        Ok(sink) => {
                                            let _ = tx_clone.send(AudioCmd::RegisterRadioSink(
                                                sink, request_id,
                                            ));
                                            return;
                                        }
                                        Err(e) => {
                                            retry_count += 1;
                                            if retry_count >= max_retries {
                                                *last_error_shared.lock().unwrap() = Some(format!(
                                                    "Connection failed after {} attempts: {}",
                                                    max_retries, e
                                                ));
                                                has_error_shared.store(
                                                    true,
                                                    std::sync::atomic::Ordering::Relaxed,
                                                );
                                            } else {
                                                std::thread::sleep(Duration::from_millis(
                                                    500 * retry_count,
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }
                    Ok(AudioCmd::PlayRaw(rx, sample_rate, channels)) => {
                        tracing::info!(sample_rate, channels, "AudioCmd::PlayRaw received (streaming)");
                        backend.stop_sink();
                        if let Some(handle) = &backend.handle {
                            let source = RawSource {
                                samples_rx: rx,
                                current_chunk: Vec::new(),
                                read_pos: 0,
                                sample_rate,
                                channels,
                            };
                            let source = VisualizerTracker {
                                inner: source,
                                amplitude: backend.amplitude_shared.clone(),
                                sample_tx: backend.sample_tx.clone(),
                            };
                            if let Ok(sink) = Sink::try_new(handle) {
                                sink.set_volume(backend.volume);
                                sink.append(source);
                                sink.play();
                                backend.sink = Some(sink);
                                backend.is_empty_shared.store(false, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    }
                    Ok(AudioCmd::RegisterRadioSink(sink, request_id)) => {
                        tracing::debug!(request_id, "AudioCmd::RegisterRadioSink received");
                        if request_id == backend.active_radio_request_id {
                            backend.radio_sink = Some(sink);
                            backend
                                .is_empty_shared
                                .store(false, std::sync::atomic::Ordering::Relaxed);
                        } else {
                            sink.stop();
                        }
                    }
                    Ok(AudioCmd::SetVolume(v)) => {
                        tracing::debug!(volume = v, "AudioCmd::SetVolume received");
                        backend.volume = v;
                        if let Some(s) = &backend.sink {
                            s.set_volume(v);
                        }
                        if let Some(s) = &backend.radio_sink {
                            s.set_volume(v);
                        }
                    }
                    Ok(AudioCmd::Pause) => {
                        tracing::info!("AudioCmd::Pause received");
                        if let Some(s) = &backend.sink {
                            s.pause();
                        }
                        if let Some(s) = &backend.radio_sink {
                            s.pause();
                        }
                    }
                    Ok(AudioCmd::Resume) => {
                        tracing::info!("AudioCmd::Resume received");
                        if let Some(s) = &backend.sink {
                            s.play();
                        }
                        if let Some(s) = &backend.radio_sink {
                            s.play();
                        }
                    }
                    Ok(AudioCmd::Stop) => {
                        tracing::info!("AudioCmd::Stop received");
                        backend.stop_sink();
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }

                backend.is_empty_shared.store(
                    backend.sink.as_ref().map(|s| s.empty()).unwrap_or(true)
                        && backend
                            .radio_sink
                            .as_ref()
                            .map(|s| s.empty())
                            .unwrap_or(true),
                    std::sync::atomic::Ordering::Relaxed,
                );
            }
        });

        Self {
            cmd_tx: tx,
            is_empty,
            has_error,
            is_initializing,
            mode,
            last_error,
            amplitude,
            dsp_state,
        }
    }

    pub fn try_init(&self) {
        let _ = self.cmd_tx.send(AudioCmd::Init);
    }

    pub fn play(&self, path: PathBuf) {
        self.is_empty
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.has_error
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = self.cmd_tx.send(AudioCmd::Play(path));
    }

    pub fn play_stream(&self, url: String) {
        self.is_empty
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.has_error
            .store(false, std::sync::atomic::Ordering::Relaxed);

        let request_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let _ = self.cmd_tx.send(AudioCmd::PlayStream(url, request_id));
    }

    pub fn play_raw(&self, rx: mpsc::Receiver<Vec<f32>>, sample_rate: u32, channels: u16) {
        self.is_empty.store(false, std::sync::atomic::Ordering::Relaxed);
        self.has_error.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = self.cmd_tx.send(AudioCmd::PlayRaw(rx, sample_rate, channels));
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self.cmd_tx.send(AudioCmd::SetVolume(volume));
    }

    pub fn pause(&self) {
        let _ = self.cmd_tx.send(AudioCmd::Pause);
    }

    pub fn resume(&self) {
        let _ = self.cmd_tx.send(AudioCmd::Resume);
    }

    pub fn stop(&self) {
        let _ = self.cmd_tx.send(AudioCmd::Stop);
    }

    pub fn is_empty(&self) -> bool {
        self.is_empty.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn has_error(&self) -> bool {
        self.has_error.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_amplitude(&self) -> f32 {
        f32::from_bits(self.amplitude.load(std::sync::atomic::Ordering::Relaxed))
    }

    pub fn is_initializing(&self) -> bool {
        self.is_initializing
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_mode(&self, mode: &str) {
        let mut m = self.mode.lock().unwrap();
        *m = mode.to_string();
    }
}

impl AudioBackend {
    fn stop_sink(&mut self) {
        tracing::debug!("Stopping audio sinks");
        if let Some(sink) = &self.sink {
            sink.stop();
        }
        if let Some(sink) = &self.radio_sink {
            sink.stop();
        }
        self.sink = None;
        self.radio_sink = None;
    }

    fn stop_all(&mut self) {
        tracing::debug!("Stopping all audio components and streams");
        self.stop_sink();
        self.handle = None;
        self.stream = None;
    }

    #[tracing::instrument(skip(self))]
    fn try_init(&mut self, _force: bool) -> Result<(), String> {
        tracing::info!("Initializing audio output device");
        self.stop_all();
        let host = rodio::cpal::default_host();
        tracing::debug!(host = host.id().name(), "Using CPAL host");

        if let Ok((stream, handle)) = OutputStream::try_default() {
            tracing::info!("Successfully initialized default output device");
            self.stream = Some(stream);
            self.handle = Some(handle);
            return Ok(());
        }

        tracing::warn!("Default output device failed, scanning for alternatives");
        if let Ok(devices) = host.output_devices() {
            for (idx, device) in devices.enumerate() {
                let name = device.name().unwrap_or_else(|_| "Unknown".into());
                tracing::debug!(idx, name = %name, "Trying alternative device");
                if let Ok((stream, handle)) = OutputStream::try_from_device(&device) {
                    tracing::info!(name = %name, "Successfully initialized alternative device");
                    self.stream = Some(stream);
                    self.handle = Some(handle);
                    return Ok(());
                }
            }
        }

        tracing::error!("No audio output devices found");
        Err("No audio output devices found.".into())
    }

    #[tracing::instrument(skip(self))]
    fn play(&mut self, path: PathBuf) -> Result<(), String> {
        tracing::info!(path = ?path, "Starting local file playback");
        self.stop_sink();

        // Only initialize if we don't have a valid handle
        if self.handle.is_none() {
            tracing::debug!("No active audio handle, attempting initialization");
            for attempt in 0..3 {
                if let Err(e) = self.try_init(false) {
                    tracing::warn!(attempt = attempt + 1, error = %e, "Initialization attempt failed");
                    if attempt == 2 {
                        return Err(e);
                    }
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                break;
            }
        }

        if let Some(handle) = &self.handle {
            tracing::trace!(path = ?path, "Opening file and creating decoder");
            let file = File::open(&path).map_err(|e| {
                tracing::error!(path = ?path, error = %e, "Failed to open audio file");
                e.to_string()
            })?;
            let source = Decoder::new(BufReader::new(file)).map_err(|e| {
                tracing::error!(path = ?path, error = %e, "Failed to decode audio file");
                e.to_string()
            })?;
            let source = rodio::Source::convert_samples::<f32>(source);
            let source = VisualizerTracker {
                inner: source,
                amplitude: self.amplitude_shared.clone(),
                sample_tx: self.sample_tx.clone(),
            };

            tracing::debug!("Creating sink and appending source");
            match Sink::try_new(handle) {
                Ok(sink) => {
                    sink.set_volume(self.volume);
                    sink.append(source);
                    sink.play();
                    self.sink = Some(sink);
                    self.is_empty_shared
                        .store(false, std::sync::atomic::Ordering::Release);
                    tracing::info!(path = ?path, "Playback started successfully");
                    return Ok(());
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create rodio sink");
                    self.stop_all();
                    return Err(format!("Sink error: {}", e));
                }
            }
        }

        tracing::error!("No audio handle available after initialization attempts");
        Err("Playback failed".into())
    }
}
