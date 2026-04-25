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
        let mut header_buffer = Vec::with_capacity(524288); // 512KB buffer for headers
        let mut temp_buf = [0u8; 16384];
        for _ in 0..32 {
            // Read up to 512KB in chunks for reliable detection
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
use crossbeam_queue::ArrayQueue;
use std::sync::Arc;

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
    sample_queue: Arc<ArrayQueue<f32>>,
    sample_counter: u32,
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

        // Send every 4th sample to analyzer to reduce overhead and prevent underruns
        self.sample_counter = (self.sample_counter + 1) % 4;
        if self.sample_counter == 0 {
            let _ = self.sample_queue.push(sample);
        }

        Some(sample)
    }
}

enum AudioCmd {
    Init,
    Play(PathBuf),
    PlayStream(String, u64),
    SetVolume(f32),
    Pause,
    Resume,
    RegisterRadioSink(Sink, u64),
}

pub struct AudioPlayer {
    cmd_tx: mpsc::Sender<AudioCmd>,
    pub is_empty: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub has_error: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub is_initializing: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub mode: std::sync::Arc<std::sync::Mutex<String>>,
    pub last_error: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    pub dsp_state: std::sync::Arc<std::sync::RwLock<DspState>>,
}

struct AudioBackend {
    stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    volume: f32,
    active_request_id: u64,
    active_request_id_shared: Arc<std::sync::atomic::AtomicU64>,
    is_empty_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    has_error_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    is_initializing_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    last_error_shared: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    sample_queue: Arc<ArrayQueue<f32>>,
    is_null: bool,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self::new_internal(false)
    }

    #[cfg(test)]
    pub fn new_null() -> Self {
        Self::new_internal(true)
    }

    fn new_internal(is_null: bool) -> Self {
        suppress_alsa_errors();
        let (tx, rx) = mpsc::channel();
        
        // Use lock-free crossbeam ArrayQueue
        let sample_queue = Arc::new(ArrayQueue::new(16384));

        let is_empty = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let has_error = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let is_initializing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mode = std::sync::Arc::new(std::sync::Mutex::new(if is_null { "NULL".into() } else { "PIPEWIRE".into() }));
        let last_error = std::sync::Arc::new(std::sync::Mutex::new(None));

        let mut analyzer = AudioAnalyzer::new();
        let dsp_state = analyzer.state.clone();

        if is_null {
            return Self {
                cmd_tx: tx,
                is_empty,
                has_error,
                is_initializing,
                mode,
                last_error,
                dsp_state,
            };
        }

        let backend_empty = is_empty.clone();
        let backend_error = has_error.clone();
        let backend_init = is_initializing.clone();
        let backend_last_err = last_error.clone();
        let backend_tx = tx.clone();
        let analyzer_queue = Arc::clone(&sample_queue);

        // Start Analyzer thread
        std::thread::spawn(move || {
            let mut samples_buf = Vec::with_capacity(2048);
            loop {
                while let Some(sample) = analyzer_queue.pop() {
                    samples_buf.push(sample);
                    if samples_buf.len() >= 2048 {
                        // Sliding window: process current buffer
                        analyzer.process_samples(&samples_buf);

                        // Increased hop size (1024) to reduce context-switching overhead
                        let hop = 1024;
                        if samples_buf.len() > hop {
                            samples_buf.drain(0..hop);
                        } else {
                            samples_buf.clear();
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });

        let backend_queue = Arc::clone(&sample_queue);
        let active_request_id_shared = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let active_request_id_backend = active_request_id_shared.clone();
        
        std::thread::spawn(move || {
            let mut backend = AudioBackend {
                stream: None,
                handle: None,
                sink: None,
                volume: 1.0,
                active_request_id: 0,
                active_request_id_shared: active_request_id_backend,
                is_empty_shared: backend_empty,
                has_error_shared: backend_error,
                is_initializing_shared: backend_init,
                last_error_shared: backend_last_err,
                sample_queue: backend_queue,
                is_null,
            };

            if !is_null {
                let _ = backend.try_init(false);
            }

            loop {
                let cmd = rx.recv_timeout(Duration::from_millis(50));
                match cmd {
                    Ok(AudioCmd::Init) => {
                        tracing::debug!("AudioCmd::Init received");
                        if backend.is_null { continue; }
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
                        if backend.is_null {
                            backend.is_empty_shared.store(false, std::sync::atomic::Ordering::Relaxed);
                            continue;
                        }
                        backend.active_request_id = 0; // Local playback doesn't need async registration
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
                        backend.active_request_id = request_id;
                        backend.active_request_id_shared.store(request_id, std::sync::atomic::Ordering::Relaxed);

                        if backend.is_null {
                            backend.is_empty_shared.store(false, std::sync::atomic::Ordering::Relaxed);
                            continue;
                        }

                        // Ensure we have a handle before spawning radio thread
                        if backend.handle.is_none() {
                            let _ = backend.try_init(false);
                        }

                        let tx_clone = backend_tx.clone();
                        let handle_shared = backend.handle.clone();
                        let volume = backend.volume;
                        let last_error_shared = backend.last_error_shared.clone();
                        let has_error_shared = backend.has_error_shared.clone();
                        let queue_clone = Arc::clone(&backend.sample_queue);
                        let active_id_shared = backend.active_request_id_shared.clone();

                        std::thread::spawn(move || {
                            if let Some(handle) = handle_shared {
                                let mut retry_count = 0;
                                let max_retries = 3;

                                while retry_count < max_retries {
                                    // Check if we're still the active request
                                    if active_id_shared.load(std::sync::atomic::Ordering::Relaxed) != request_id {
                                        tracing::debug!(request_id, "Radio thread cancelling: stale request");
                                        return;
                                    }

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

                                        // Final check before heavy decoder/sink allocation
                                        if active_id_shared.load(std::sync::atomic::Ordering::Relaxed) != request_id {
                                            return Err("Canceled".into());
                                        }

                                        let reader = StreamingReader::new(response);
                                        let source = Decoder::new(reader)
                                            .map_err(|e| format!("Decoder: {}", e))?;
                                        let source = rodio::Source::convert_samples::<f32>(source);
                                        let source = VisualizerTracker {
                                            inner: source,
                                            sample_queue: queue_clone.clone(),
                                            sample_counter: 0,
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
                                        Err(e) if e == "Canceled" => return,
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
                    Ok(AudioCmd::RegisterRadioSink(sink, request_id)) => {
                        tracing::debug!(request_id, "AudioCmd::RegisterRadioSink received");
                        if request_id == backend.active_request_id {
                            backend.sink = Some(sink);
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
                    }
                    Ok(AudioCmd::Pause) => {
                        tracing::info!("AudioCmd::Pause received");
                        if let Some(s) = &backend.sink {
                            s.pause();
                        }
                    }
                    Ok(AudioCmd::Resume) => {
                        tracing::info!("AudioCmd::Resume received");
                        if let Some(s) = &backend.sink {
                            s.play();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }

                if !backend.is_null {
                    backend.is_empty_shared.store(
                        backend.sink.as_ref().map(|s| s.empty()).unwrap_or(true),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
            }
        });

        Self {
            cmd_tx: tx,
            is_empty,
            has_error,
            is_initializing,
            mode,
            last_error,
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

    pub fn set_volume(&self, volume: f32) {
        let _ = self.cmd_tx.send(AudioCmd::SetVolume(volume));
    }

    pub fn pause(&self) {
        let _ = self.cmd_tx.send(AudioCmd::Pause);
    }

    pub fn resume(&self) {
        let _ = self.cmd_tx.send(AudioCmd::Resume);
    }

    pub fn is_empty(&self) -> bool {
        self.is_empty.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn has_error(&self) -> bool {
        self.has_error.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_amplitude(&self) -> f32 {
        self.dsp_state.read().unwrap().amplitude
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
        tracing::debug!("Stopping audio sink");
        if let Some(sink) = &self.sink {
            sink.stop();
        }
        self.sink = None;
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
            let source = Decoder::new(BufReader::with_capacity(65536, file)).map_err(|e| {
                tracing::error!(path = ?path, error = %e, "Failed to decode audio file");
                e.to_string()
            })?;
            let source = rodio::Source::convert_samples::<f32>(source);
            let source = VisualizerTracker {
                inner: source,
                sample_queue: self.sample_queue.clone(),
                sample_counter: 0,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct TestSource {
        samples: std::vec::IntoIter<f32>,
    }
    impl Iterator for TestSource {
        type Item = f32;
        fn next(&mut self) -> Option<Self::Item> { self.samples.next() }
    }
    impl rodio::Source for TestSource {
        fn current_frame_len(&self) -> Option<usize> { None }
        fn channels(&self) -> u16 { 1 }
        fn sample_rate(&self) -> u32 { 44100 }
        fn total_duration(&self) -> Option<Duration> { None }
    }

    #[test]
    fn test_streaming_reader_new() {
        let data = vec![1u8; 20000];
        let reader = StreamingReader::new(Cursor::new(data.clone()));
        assert_eq!(reader.header_buffer.len(), 20000);
        assert_eq!(reader.pos, 20000);
        assert_eq!(reader.header_read_pos, 0);
    }

    #[test]
    fn test_streaming_reader_read() {
        let data = (0..100u8).collect::<Vec<_>>();
        let mut reader = StreamingReader::new(Cursor::new(data));
        let mut buf = [0u8; 10];
        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 10);
        assert_eq!(buf, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_streaming_reader_seek_start() {
        let data = (0..100u8).collect::<Vec<_>>();
        let mut reader = StreamingReader::new(Cursor::new(data));
        let pos = reader.seek(SeekFrom::Start(10)).unwrap();
        assert_eq!(pos, 10);
        assert_eq!(reader.header_read_pos, 10);
    }

    #[test]
    fn test_streaming_reader_seek_current() {
        let data = (0..100u8).collect::<Vec<_>>();
        let mut reader = StreamingReader::new(Cursor::new(data));
        reader.seek(SeekFrom::Start(10)).unwrap();
        let pos = reader.seek(SeekFrom::Current(5)).unwrap();
        assert_eq!(pos, 15);
        let pos = reader.seek(SeekFrom::Current(-10)).unwrap();
        assert_eq!(pos, 5);
    }

    #[test]
    fn test_streaming_reader_seek_current_backward_past_header() {
        let data = (0..100u8).collect::<Vec<_>>();
        let mut reader = StreamingReader::new(Cursor::new(data));
        reader.seek(SeekFrom::Start(10)).unwrap();
        let pos = reader.seek(SeekFrom::Current(-15)).unwrap();
        assert_eq!(pos, 10);
    }

    #[test]
    fn test_streaming_reader_read_past_header() {
        let data = (0..150u8).collect::<Vec<_>>();
        let mut reader = StreamingReader::new(Cursor::new(data));
        let mut buf = vec![0u8; 150];
        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 150);
        assert_eq!(buf[149], 149);
    }

    #[test]
    fn test_streaming_reader_empty_source() {
        let data: Vec<u8> = vec![];
        let mut reader = StreamingReader::new(Cursor::new(data));
        assert_eq!(reader.header_buffer.len(), 0);
        let mut buf = [0u8; 10];
        let n = reader.read(&mut buf).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn test_visualizer_tracker_properties() {
        let samples = vec![0.1f32; 100];
        let tracker = VisualizerTracker {
            inner: TestSource { samples: samples.into_iter() },
            sample_queue: Arc::new(ArrayQueue::new(10)),
            sample_counter: 0,
        };
        use rodio::Source;
        assert_eq!(tracker.channels(), 1);
        assert_eq!(tracker.sample_rate(), 44100);
    }

    #[test]
    fn test_visualizer_tracker_sampling_overflow() {
        let samples = vec![0.1f32; 100];
        let queue = Arc::new(ArrayQueue::new(1));
        let mut tracker = VisualizerTracker {
            inner: TestSource { samples: samples.into_iter() },
            sample_queue: queue.clone(),
            sample_counter: 0,
        };
        for _ in 0..4 { tracker.next(); }
        assert_eq!(queue.len(), 1);
        for _ in 0..4 { tracker.next(); }
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_streaming_reader_seek_start_at_current() {
        let data = (0..100u8).collect::<Vec<_>>();
        let mut reader = StreamingReader::new(Cursor::new(data));
        reader.seek(SeekFrom::Start(10)).unwrap();
        let pos = reader.seek(SeekFrom::Start(10)).unwrap();
        assert_eq!(pos, 10);
    }

    #[test]
    fn test_streaming_reader_seek_current_forward_past_header() {
        let data = (0..150u8).collect::<Vec<_>>();
        let mut reader = StreamingReader::new(Cursor::new(data));
        let pos = reader.seek(SeekFrom::Current(50)).unwrap();
        assert_eq!(pos, 50);
    }

    #[test]
    fn test_streaming_reader_seek_current_zero() {
        let data = (0..100u8).collect::<Vec<_>>();
        let mut reader = StreamingReader::new(Cursor::new(data));
        reader.seek(SeekFrom::Start(10)).unwrap();
        let pos = reader.seek(SeekFrom::Current(0)).unwrap();
        assert_eq!(pos, 10);
    }

    #[test]
    fn test_streaming_reader_seek_end() {
        let data = (0..100u8).collect::<Vec<_>>();
        let mut reader = StreamingReader::new(Cursor::new(data));
        let pos = reader.seek(SeekFrom::End(0)).unwrap();
        assert_eq!(pos, 0);
    }

    #[test]
    fn test_visualizer_tracker_sampling() {
        let samples = vec![0.1f32, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let queue = Arc::new(ArrayQueue::new(10));
        let mut tracker = VisualizerTracker {
            inner: TestSource { samples: samples.into_iter() },
            sample_queue: queue.clone(),
            sample_counter: 0,
        };
        tracker.next();
        tracker.next();
        tracker.next();
        let s4 = tracker.next().unwrap();
        assert_eq!(s4, 0.4);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pop().unwrap(), 0.4);
    }

    #[test]
    fn test_streaming_reader_slow_input() {
        struct SlowReader {
            data: Vec<u8>,
            pos: usize,
        }
        impl Read for SlowReader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.pos >= self.data.len() { return Ok(0); }
                if buf.is_empty() { return Ok(0); }
                buf[0] = self.data[self.pos];
                self.pos += 1;
                Ok(1)
            }
        }
        let data = vec![1, 2, 3, 4, 5];
        let mut reader = StreamingReader::new(SlowReader { data, pos: 0 });
        let mut buf = [0u8; 10];
        let n = reader.read(&mut buf).unwrap();
        assert!(n > 0);
    }

    #[test]
    fn test_streaming_reader_consecutive_seeks() {
        let data = (0..100u8).collect::<Vec<_>>();
        let mut reader = StreamingReader::new(Cursor::new(data));
        reader.seek(SeekFrom::Start(10)).unwrap();
        reader.seek(SeekFrom::Start(20)).unwrap();
        reader.seek(SeekFrom::Current(-5)).unwrap();
        let pos = reader.seek(SeekFrom::Current(2)).unwrap();
        assert_eq!(pos, 17);
    }

    #[test]
    fn test_visualizer_tracker_empty_source() {
        let samples: Vec<f32> = vec![];
        let queue = Arc::new(ArrayQueue::new(10));
        let mut tracker = VisualizerTracker {
            inner: TestSource { samples: samples.into_iter() },
            sample_queue: queue.clone(),
            sample_counter: 0,
        };
        assert!(tracker.next().is_none());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_visualizer_tracker_exactly_4_samples() {
        let samples = vec![0.1, 0.2, 0.3, 0.4];
        let queue = Arc::new(ArrayQueue::new(10));
        let mut tracker = VisualizerTracker {
            inner: TestSource { samples: samples.into_iter() },
            sample_queue: queue.clone(),
            sample_counter: 0,
        };
        for _ in 0..4 { tracker.next(); }
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.pop().unwrap(), 0.4);
    }

    #[test]
    fn test_visualizer_tracker_10_samples() {
        let samples = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let queue = Arc::new(ArrayQueue::new(10));
        let mut tracker = VisualizerTracker {
            inner: TestSource { samples: samples.into_iter() },
            sample_queue: queue.clone(),
            sample_counter: 0,
        };
        for _ in 0..10 { tracker.next(); }
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_lyric_line_display() {
        let line = LyricLine {
            time: Duration::from_secs(10),
            text: "Hello World".to_string(),
        };
        assert_eq!(line.text, "Hello World");
        assert_eq!(line.time.as_secs(), 10);
    }
}
