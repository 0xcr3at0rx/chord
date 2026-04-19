use anyhow::Result;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
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
        log::info!(
            "StreamingReader: buffered {} bytes of header",
            header_buffer.len()
        );
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

fn suppress_alsa_errors() {
    // Disabled C-variadic FFI as it is unstable.
    // For most users, env_logger or other filtering is enough.
}

#[derive(Clone, Debug)]
pub struct LyricLine {
    pub time: Duration,
    pub text: String,
}

struct AmplitudeTracker<S>
where
    S: rodio::Source<Item = f32>,
{
    inner: S,
    amplitude: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl<S> rodio::Source for AmplitudeTracker<S>
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

impl<S> Iterator for AmplitudeTracker<S>
where
    S: rodio::Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;
        let val = sample.abs();

        // Simple exponential moving average for smoothing
        let current_bits = self.amplitude.load(std::sync::atomic::Ordering::Relaxed);
        let current = f32::from_bits(current_bits);
        let new_val = current * 0.98 + val * 0.02; // Slower decay for smoother visuals
        self.amplitude
            .store(new_val.to_bits(), std::sync::atomic::Ordering::Relaxed);

        Some(sample)
    }
}

enum AudioCmd {
    Init(Option<String>),
    Play(PathBuf),
    PlayStream(String, u64),
    SetVolume(f32),
    Pause,
    Resume,
    RegisterRadioSink(Sink, u64),
    UpdateConfig {
        sample_rate: u32,
        buffer_ms: u32,
        resample_quality: u32,
    },
}

pub struct AudioPlayer {
    cmd_tx: mpsc::Sender<AudioCmd>,
    pub device_name: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    pub is_empty: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub has_error: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub is_initializing: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub mode: std::sync::Arc<std::sync::Mutex<String>>,
    pub last_error: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    pub amplitude: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

struct AudioBackend {
    stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    radio_sink: Option<Sink>,
    volume: f32,
    sample_rate: u32,
    buffer_ms: u32,
    resample_quality: u32,
    active_radio_request_id: u64,
    device_name_shared: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    is_empty_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    has_error_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    is_initializing_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    last_error_shared: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    amplitude_shared: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        suppress_alsa_errors();
        let (tx, rx) = mpsc::channel();
        let device_name = std::sync::Arc::new(std::sync::Mutex::new(None));
        let is_empty = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let has_error = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let is_initializing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mode = std::sync::Arc::new(std::sync::Mutex::new("PIPEWIRE".into()));
        let last_error = std::sync::Arc::new(std::sync::Mutex::new(None));
        let amplitude = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0.0f32.to_bits()));

        let backend_name = device_name.clone();
        let backend_empty = is_empty.clone();
        let backend_error = has_error.clone();
        let backend_init = is_initializing.clone();
        let backend_last_err = last_error.clone();
        let backend_amplitude = amplitude.clone();
        let backend_tx = tx.clone();

        std::thread::spawn(move || {
            let mut backend = AudioBackend {
                stream: None,
                handle: None,
                sink: None,
                radio_sink: None,
                volume: 1.0,
                sample_rate: 48000,
                buffer_ms: 100,
                resample_quality: 4,
                active_radio_request_id: 0,
                device_name_shared: backend_name,
                is_empty_shared: backend_empty,
                has_error_shared: backend_error,
                is_initializing_shared: backend_init,
                last_error_shared: backend_last_err,
                amplitude_shared: backend_amplitude,
            };

            let _ = backend.try_init(false);

            loop {
                let cmd = rx.recv_timeout(Duration::from_millis(50));
                match cmd {
                    Ok(AudioCmd::Init(name)) => {
                        backend
                            .is_initializing_shared
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        let res = if let Some(n) = name {
                            backend.try_init_with_name(&n)
                        } else {
                            backend.try_init(true)
                        };

                        if let Err(e) = &res {
                            log::error!("Audio initialization failed: {}", e);
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
                        let res = backend.play(path);
                        if let Err(e) = &res {
                            log::error!("Playback failed: {}", e);
                            *backend.last_error_shared.lock().unwrap() = Some(e.clone());
                        } else {
                            *backend.last_error_shared.lock().unwrap() = None;
                        }
                        backend
                            .has_error_shared
                            .store(res.is_err(), std::sync::atomic::Ordering::Relaxed);
                    }
                    Ok(AudioCmd::PlayStream(url, request_id)) => {
                        backend.stop_sink();
                        backend.active_radio_request_id = request_id;

                        let tx_clone = backend_tx.clone();
                        let handle_shared = backend.handle.clone();
                        let volume = backend.volume;
                        let last_error_shared = backend.last_error_shared.clone();
                        let has_error_shared = backend.has_error_shared.clone();
                        let amplitude_shared_thread = backend.amplitude_shared.clone();

                        std::thread::spawn(move || {
                            if let Some(handle) = handle_shared {
                                let res = (|| -> Result<Sink, String> {
                                    let response = reqwest::blocking::Client::builder()
                                        .user_agent(
                                            "Chord/1.1 (https://github.com/0xcr3at0rx/chord)",
                                        )
                                        .timeout(Duration::from_secs(20))
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
                                    let source = AmplitudeTracker {
                                        inner: source,
                                        amplitude: amplitude_shared_thread,
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
                                        let _ = tx_clone
                                            .send(AudioCmd::RegisterRadioSink(sink, request_id));
                                    }
                                    Err(e) => {
                                        log::error!("Stream error: {}", e);
                                        *last_error_shared.lock().unwrap() = Some(e);
                                        has_error_shared
                                            .store(true, std::sync::atomic::Ordering::Relaxed);
                                    }
                                }
                            }
                        });
                    }
                    Ok(AudioCmd::RegisterRadioSink(sink, request_id)) => {
                        if request_id == backend.active_radio_request_id {
                            backend.radio_sink = Some(sink);
                            backend
                                .is_empty_shared
                                .store(false, std::sync::atomic::Ordering::Relaxed);
                        } else {
                            sink.stop();
                        }
                    }
                    Ok(AudioCmd::UpdateConfig {
                        sample_rate,
                        buffer_ms,
                        resample_quality,
                    }) => {
                        backend
                            .is_initializing_shared
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        backend.sample_rate = sample_rate;
                        backend.buffer_ms = buffer_ms;
                        backend.resample_quality = resample_quality;
                        let _ = backend.try_init(true);
                        backend
                            .is_initializing_shared
                            .store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                    Ok(AudioCmd::SetVolume(v)) => {
                        backend.volume = v;
                        if let Some(s) = &backend.sink {
                            s.set_volume(v);
                        }
                        if let Some(s) = &backend.radio_sink {
                            s.set_volume(v);
                        }
                    }
                    Ok(AudioCmd::Pause) => {
                        if let Some(s) = &backend.sink {
                            s.pause();
                        }
                        if let Some(s) = &backend.radio_sink {
                            s.pause();
                        }
                    }
                    Ok(AudioCmd::Resume) => {
                        if let Some(s) = &backend.sink {
                            s.play();
                        }
                        if let Some(s) = &backend.radio_sink {
                            s.play();
                        }
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
            device_name,
            is_empty,
            has_error,
            is_initializing,
            mode,
            last_error,
            amplitude,
        }
    }

    pub fn try_init(&self) {
        let _ = self.cmd_tx.send(AudioCmd::Init(None));
    }

    #[allow(dead_code)]
    pub fn try_init_with_name(&self, name: &str) {
        let _ = self.cmd_tx.send(AudioCmd::Init(Some(name.to_string())));
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

    pub fn update_audio_config(&self, sample_rate: u32, buffer_ms: u32, resample_quality: u32) {
        let _ = self.cmd_tx.send(AudioCmd::UpdateConfig {
            sample_rate,
            buffer_ms,
            resample_quality,
        });
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
        self.stop_sink();
        self.handle = None;
        self.stream = None;
    }

    fn try_init(&mut self, _force: bool) -> Result<(), String> {
        self.stop_all();
        let host = rodio::cpal::default_host();

        if let Ok((stream, handle)) = OutputStream::try_default() {
            self.stream = Some(stream);
            self.handle = Some(handle);
            let name = host
                .default_output_device()
                .and_then(|d| d.name().ok())
                .unwrap_or_else(|| "System Default".into());
            *self.device_name_shared.lock().unwrap() = Some(name);
            return Ok(());
        }

        if let Ok(devices) = host.output_devices() {
            for device in devices {
                if let Ok((stream, handle)) = OutputStream::try_from_device(&device) {
                    self.stream = Some(stream);
                    self.handle = Some(handle);
                    let name = device.name().unwrap_or_else(|_| "Unknown Device".into());
                    *self.device_name_shared.lock().unwrap() = Some(name.clone());
                    return Ok(());
                }
            }
        }

        Err("No audio output devices found.".into())
    }

    fn try_init_with_name(&mut self, name: &str) -> Result<(), String> {
        let host = rodio::cpal::default_host();
        if let Ok(devices) = host.output_devices() {
            for device in devices {
                if let Ok(d_name) = device.name() {
                    if d_name == name {
                        if let Ok((stream, handle)) = OutputStream::try_from_device(&device) {
                            self.stop_all();
                            self.stream = Some(stream);
                            self.handle = Some(handle);
                            *self.device_name_shared.lock().unwrap() = Some(d_name);
                            return Ok(());
                        }
                    }
                }
            }
        }
        self.try_init(true)
    }

    fn play(&mut self, path: PathBuf) -> Result<(), String> {
        self.stop_sink();

        for attempt in 0..3 {
            if let Err(e) = self.try_init(false) {
                if attempt == 2 {
                    return Err(e);
                }
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }

            if let Some(handle) = &self.handle {
                let file = File::open(&path).map_err(|e| e.to_string())?;
                let source = Decoder::new(BufReader::new(file)).map_err(|e| e.to_string())?;
                let source = rodio::Source::convert_samples::<f32>(source);
                let source = AmplitudeTracker {
                    inner: source,
                    amplitude: self.amplitude_shared.clone(),
                };

                match Sink::try_new(handle) {
                    Ok(sink) => {
                        sink.set_volume(self.volume);
                        sink.append(source);
                        sink.play();
                        self.sink = Some(sink);
                        return Ok(());
                    }
                    Err(_) => {
                        self.stop_all();
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        }

        Err("Playback failed".into())
    }
}

pub fn probe_duration(path: &Path) -> Option<Duration> {
    use std::fs::File;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::default::get_probe;

    if let Ok(file) = File::open(path) {
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        if let Ok(probed) = get_probe().format(
            &Default::default(),
            mss,
            &Default::default(),
            &Default::default(),
        ) {
            let format = probed.format;
            if let Some(track) = format.tracks().first() {
                if let Some(n_frames) = track.codec_params.n_frames {
                    if let Some(tb) = track.codec_params.time_base {
                        let time = tb.calc_time(n_frames);
                        return Some(
                            Duration::from_secs(time.seconds) + Duration::from_secs_f64(time.frac),
                        );
                    }
                }
            }
        }
    }
    None
}
