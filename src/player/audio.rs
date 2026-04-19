use anyhow::Result;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

// --- Manual ALSA FFI Implementation ---
#[cfg(target_os = "linux")]
mod alsa_ffi {
    use std::os::raw::c_int;

    #[link(name = "asound")]
    extern "C" {
        pub fn snd_lib_error_set_handler(handler: *const std::ffi::c_void) -> c_int;
    }
}

#[cfg(target_os = "linux")]
unsafe extern "C" fn alsa_error_handler(
    _file: *const std::os::raw::c_char,
    _line: std::os::raw::c_int,
    _func: *const std::os::raw::c_char,
    _err: std::os::raw::c_int,
    _fmt: *const std::os::raw::c_char,
) {
}

pub fn suppress_alsa_errors() {
    #[cfg(target_os = "linux")]
    // SAFETY: This is a safe FFI call to set an error handler for the ALSA library.
    // The handler is a static function that does nothing, and the call is made
    // only on Linux systems where the asound library is expected to be present.
    unsafe {
        alsa_ffi::snd_lib_error_set_handler(alsa_error_handler as *const std::ffi::c_void);
    }
}

#[derive(Clone, Debug)]
pub struct LyricLine {
    pub time: Duration,
    pub text: String,
}

enum AudioCmd {
    Init(Option<String>),
    Play(PathBuf),
    PlayStream(String),
    SetVolume(f32),
    Pause,
    Resume,
    NextDevice,
}

pub struct AudioPlayer {
    cmd_tx: mpsc::Sender<AudioCmd>,
    pub device_name: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    pub is_empty: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub has_error: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub is_initializing: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub mode: std::sync::Arc<std::sync::Mutex<String>>,
    pub last_error: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

struct AudioBackend {
    stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    volume: f32,
    device_name_shared: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    is_empty_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    has_error_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    is_initializing_shared: std::sync::Arc<std::sync::atomic::AtomicBool>,
    last_error_shared: std::sync::Arc<std::sync::Mutex<Option<String>>>,
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
        
        let backend_name = device_name.clone();
        let backend_empty = is_empty.clone();
        let backend_error = has_error.clone();
        let backend_init = is_initializing.clone();
        let backend_last_err = last_error.clone();
        
        std::thread::spawn(move || {
            let mut backend = AudioBackend {
                stream: None,
                handle: None,
                sink: None,
                volume: 1.0,
                device_name_shared: backend_name,
                is_empty_shared: backend_empty,
                has_error_shared: backend_error,
                is_initializing_shared: backend_init,
                last_error_shared: backend_last_err,
            };
            
            let _ = backend.try_init(false);

            loop {
                let cmd = rx.recv_timeout(Duration::from_millis(50));
                match cmd {
                    Ok(AudioCmd::Init(name)) => {
                        backend.is_initializing_shared.store(true, std::sync::atomic::Ordering::Relaxed);
                        let res = if let Some(n) = name {
                            log::info!("Initializing audio with device: {}", n);
                            backend.try_init_with_name(&n)
                        } else {
                            log::info!("Re-initializing default audio device");
                            backend.try_init(true)
                        };
                        
                        if let Err(e) = &res {
                            log::error!("Audio initialization failed: {}", e);
                            *backend.last_error_shared.lock().unwrap() = Some(e.clone());
                        } else {
                            *backend.last_error_shared.lock().unwrap() = None;
                        }
                        backend.has_error_shared.store(res.is_err(), std::sync::atomic::Ordering::Relaxed);
                        backend.is_initializing_shared.store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                    Ok(AudioCmd::Play(path)) => {
                        let res = backend.play(path);
                        if let Err(e) = &res {
                            log::error!("Playback failed: {}", e);
                            *backend.last_error_shared.lock().unwrap() = Some(e.clone());
                        } else {
                            *backend.last_error_shared.lock().unwrap() = None;
                        }
                        backend.has_error_shared.store(res.is_err(), std::sync::atomic::Ordering::Relaxed);
                    }
                    Ok(AudioCmd::PlayStream(url)) => {
                        let res = backend.play_stream(url);
                        if let Err(e) = &res {
                            log::error!("Stream playback failed: {}", e);
                            *backend.last_error_shared.lock().unwrap() = Some(e.clone());
                        } else {
                            *backend.last_error_shared.lock().unwrap() = None;
                        }
                        backend.has_error_shared.store(res.is_err(), std::sync::atomic::Ordering::Relaxed);
                    }
                    Ok(AudioCmd::SetVolume(v)) => {
                        backend.volume = v;
                        if let Some(s) = &backend.sink {
                            s.set_volume(v);
                        }
                    }
                    Ok(AudioCmd::Pause) => {
                        log::info!("Pausing playback");
                        if let Some(s) = &backend.sink {
                            s.pause();
                        }
                    }
                    Ok(AudioCmd::Resume) => {
                        log::info!("Resuming playback");
                        if let Some(s) = &backend.sink {
                            s.play();
                        }
                    }
                    Ok(AudioCmd::NextDevice) => {
                        log::info!("Cycling to next audio device");
                        backend.is_initializing_shared.store(true, std::sync::atomic::Ordering::Relaxed);
                        if let Err(e) = backend.next_device() {
                            log::error!("Failed to cycle device: {}", e);
                        }
                        backend.is_initializing_shared.store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                
                backend.is_empty_shared.store(
                    backend.sink.as_ref().map(|s| s.empty()).unwrap_or(true),
                    std::sync::atomic::Ordering::Relaxed
                );
            }
        });
        
        Self { cmd_tx: tx, device_name, is_empty, has_error, is_initializing, mode, last_error }
    }

    pub fn try_init(&self) {
        let _ = self.cmd_tx.send(AudioCmd::Init(None));
    }

    pub fn try_init_with_name(&self, name: &str) {
        let _ = self.cmd_tx.send(AudioCmd::Init(Some(name.to_string())));
    }

    pub fn next_device(&self) {
        let _ = self.cmd_tx.send(AudioCmd::NextDevice);
    }

    pub fn play(&self, path: PathBuf) {
        self.is_empty.store(false, std::sync::atomic::Ordering::Relaxed);
        self.has_error.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = self.cmd_tx.send(AudioCmd::Play(path));
    }

    pub fn play_stream(&self, url: String) {
        self.is_empty.store(false, std::sync::atomic::Ordering::Relaxed);
        self.has_error.store(false, std::sync::atomic::Ordering::Relaxed);
        let _ = self.cmd_tx.send(AudioCmd::PlayStream(url));
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

    pub fn is_initializing(&self) -> bool {
        self.is_initializing.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_mode(&self, mode: &str) {
        *self.mode.lock().unwrap() = mode.to_string();
    }
}

impl AudioBackend {
    fn stop_sink(&mut self) {
        if let Some(sink) = &self.sink {
            sink.stop();
        }
        self.sink = None;
    }

    fn stop_all(&mut self) {
        self.stop_sink();
        self.handle = None;
        self.stream = None;
    }

    fn try_init(&mut self, force: bool) -> Result<(), String> {
        if !force && self.stream.is_some() && self.handle.is_some() {
            return Ok(());
        }

        self.stop_all();
        let host = rodio::cpal::default_host();

        for attempt in 0..3 {
            // Strategy 1: Try default output
            if let Ok((stream, handle)) = OutputStream::try_default() {
                self.stream = Some(stream);
                self.handle = Some(handle);
                let name = host.default_output_device().and_then(|d| d.name().ok()).unwrap_or_else(|| "Default".into());
                *self.device_name_shared.lock().unwrap() = Some(name);
                return Ok(());
            }

            // Strategy 2: Iterate all devices
            if let Ok(devices) = host.output_devices() {
                for device in devices {
                    if let Ok((stream, handle)) = OutputStream::try_from_device(&device) {
                        self.stream = Some(stream);
                        self.handle = Some(handle);
                        let name = device.name().unwrap_or_else(|_| "Unknown".into());
                        *self.device_name_shared.lock().unwrap() = Some(name);
                        return Ok(());
                    }
                }
            }
            
            std::thread::sleep(Duration::from_millis(100 * (attempt + 1)));
        }

        Err("No audio device available".into())
    }

    fn try_init_with_name(&mut self, name: &str) -> Result<(), String> {
        let host = rodio::cpal::default_host();
        if let Ok(devices) = host.output_devices() {
            for d in devices {
                if d.name().ok().as_deref() == Some(name) {
                    self.stop_all();
                    if let Ok((stream, handle)) = OutputStream::try_from_device(&d) {
                        self.stream = Some(stream);
                        self.handle = Some(handle);
                        *self.device_name_shared.lock().unwrap() = Some(name.to_string());
                        return Ok(());
                    }
                }
            }
        }
        self.try_init(true)
    }

    fn next_device(&mut self) -> Result<(), String> {
        let host = rodio::cpal::default_host();
        let devices: Vec<_> = host.output_devices().map(|d| d.collect()).unwrap_or_default();
        if devices.is_empty() { return Err("No devices".into()); }

        let current_name = self.device_name_shared.lock().unwrap().clone();
        let mut next_idx = 0;

        if let Some(cur) = current_name {
            if let Some(idx) = devices.iter().position(|d| d.name().ok().as_deref() == Some(&cur)) {
                next_idx = (idx + 1) % devices.len();
            }
        }

        if let Some(device) = devices.get(next_idx) {
            if let Ok(name) = device.name() {
                return self.try_init_with_name(&name);
            }
        }
        self.try_init(true)
    }

    fn play(&mut self, path: PathBuf) -> Result<(), String> {
        self.stop_sink();
        
        for attempt in 0..3 {
            if let Err(e) = self.try_init(false) {
                if attempt == 2 { return Err(e); }
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }

            if let Some(handle) = &self.handle {
                let file = File::open(&path).map_err(|e| e.to_string())?;
                let source = Decoder::new(BufReader::new(file)).map_err(|e| e.to_string())?;
                
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

    fn play_stream(&mut self, url: String) -> Result<(), String> {
        self.stop_sink();
        
        for attempt in 0..3 {
            if let Err(e) = self.try_init(false) {
                if attempt == 2 { return Err(e); }
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }

            if let Some(handle) = &self.handle {
                let mut response = reqwest::blocking::get(&url).map_err(|e| e.to_string())?;
                let mut buffer = Vec::new();
                std::io::copy(&mut response, &mut buffer).map_err(|e| e.to_string())?;
                let source = Decoder::new(std::io::Cursor::new(buffer)).map_err(|e| e.to_string())?;
                
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
        
        Err("Stream playback failed".into())
    }
}


pub fn probe_duration(path: &Path) -> Option<Duration> {
    use symphonia::default::get_probe;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::probe::Hint;

    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = get_probe().format(&hint, mss, &Default::default(), &Default::default()).ok()?;
    let format = probed.format;
    
    // Find the first track with a known duration
    for track in format.tracks() {
        if let Some(params) = &track.codec_params.time_base {
            if let Some(n_frames) = track.codec_params.n_frames {
                let time = params.calc_time(n_frames);
                return Some(Duration::from_secs(time.seconds) + Duration::from_secs_f64(time.frac));
            }
        }
    }
    None
}
