use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use std::sync::Once;
use tracing_appender::non_blocking::WorkerGuard;
use directories::ProjectDirs;
use crossterm::terminal::disable_raw_mode;
use gag::Redirect;
use std::fs::OpenOptions;

static INIT: Once = Once::new();

pub struct LoggerGuards {
    pub worker: Option<WorkerGuard>,
    pub stderr_redirect: Option<Redirect<std::fs::File>>,
}

pub fn init_logger() -> LoggerGuards {
    let mut worker_guard = None;
    let mut stderr_guard = None;
    
    INIT.call_once(|| {
        let proj_dirs = ProjectDirs::from("", "", "chord");
        
        if let Some(dirs) = proj_dirs {
            let log_dir = dirs.config_dir().join("logs");
            let _ = std::fs::create_dir_all(&log_dir);
            
            // 1. Set up tracing file appender
            let file_appender = tracing_appender::rolling::daily(&log_dir, "chord.log");
            let (non_blocking, g) = tracing_appender::non_blocking(file_appender);
            worker_guard = Some(g);

            // 2. Set up C library output redirection (stderr ONLY)
            // We redirect stderr to a separate file to catch ALSA/PipeWire errors.
            // Redirecting stdout breaks the TUI as ratatui writes to it.
            let c_log_path = log_dir.join("system_audio.log");
            if let Ok(c_log_file) = OpenOptions::new().create(true).append(true).open(&c_log_path) {
                stderr_guard = Redirect::stderr(c_log_file).ok();
            }
            
            let file_layer = fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_line_number(true);

            // Select log level based on build profile
            let default_level = if cfg!(debug_assertions) {
                "chord=trace,info"
            } else {
                "chord=info"
            };

            let filter_layer = EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(default_level))
                .unwrap();

            tracing_subscriber::registry()
                .with(filter_layer)
                .with(file_layer)
                .init();
        } else {
            // Minimal fallback if no config dir
            let filter_layer = EnvFilter::new("info");
            tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt::layer())
                .init();
        };

        // Set up panic hook to log panics and reset terminal
        std::panic::set_hook(Box::new(|panic_info| {
            let _ = disable_raw_mode();
            let location = panic_info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_else(|| "unknown".to_string());
            let payload = panic_info.payload();
            let message = if let Some(s) = payload.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.as_str()
            } else {
                "Box<Any>"
            };
            
            tracing::error!(target: "chord::panic", message, location, "APPLICATION PANIC");
            eprintln!("\n\n\x1b[31;1mFATAL ERROR (PANIC)\x1b[0m");
            eprintln!("\x1b[1mLocation:\x1b[0m {}", location);
            eprintln!("\x1b[1mMessage:\x1b[0m  {}", message);
            eprintln!("\x1b[33mCheck log file for full details.\x1b[0m\n");
        }));
    });

    LoggerGuards {
        worker: worker_guard,
        stderr_redirect: stderr_guard,
    }
}

/// FFI interface for external logging
#[no_mangle]
pub extern "C" fn chord_log_info(msg: *const std::os::raw::c_char) {
    if msg.is_null() { return; }
    unsafe {
        if let Ok(s) = std::ffi::CStr::from_ptr(msg).to_str() {
            tracing::info!(target: "chord::ffi", "{}", s);
        }
    }
}

#[no_mangle]
pub extern "C" fn chord_log_error(msg: *const std::os::raw::c_char) {
    if msg.is_null() { return; }
    unsafe {
        if let Ok(s) = std::ffi::CStr::from_ptr(msg).to_str() {
            tracing::error!(target: "chord::ffi", "{}", s);
        }
    }
}

#[no_mangle]
pub extern "C" fn chord_log_debug(msg: *const std::os::raw::c_char) {
    if msg.is_null() { return; }
    unsafe {
        if let Ok(s) = std::ffi::CStr::from_ptr(msg).to_str() {
            tracing::debug!(target: "chord::ffi", "{}", s);
        }
    }
}

#[no_mangle]
pub extern "C" fn chord_log_trace(msg: *const std::os::raw::c_char) {
    if msg.is_null() { return; }
    unsafe {
        if let Ok(s) = std::ffi::CStr::from_ptr(msg).to_str() {
            tracing::trace!(target: "chord::ffi", "{}", s);
        }
    }
}
