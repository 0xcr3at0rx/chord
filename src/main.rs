mod config;
mod core;
mod player;
mod storage;

use anyhow::Result;
use core::config::Settings;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use storage::index::LibraryIndex;

fn init_logging(settings: &Settings) {
    let is_debug = cfg!(debug_assertions) || std::env::var("CHORD_DEBUG").is_ok();
    
    if !is_debug {
        let log_path = settings.config_dir.join("chord.log");
        let log_file = File::create(log_path).expect("Failed to create log file");

        env_logger::Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{} [{}] - {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    record.level(),
                    record.args()
                )
            })
            .target(env_logger::Target::Pipe(Box::new(log_file)))
            .filter(None, log::LevelFilter::Error)
            .filter_module("chord", log::LevelFilter::Info)
            .init();
    } else {
        // No logging in dev mode by default to keep console clean
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings_raw = Settings::new()?;
    init_logging(&settings_raw);

    let settings = Arc::new(settings_raw);
    let index = Arc::new(LibraryIndex::new(&settings.config_dir));

    player::run_player(settings, index).await?;

    Ok(())
}
