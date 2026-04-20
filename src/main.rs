mod core;
mod player;
mod storage;

use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings_raw = core::config::Settings::new()?;
    let settings = Arc::new(settings_raw);
    let index = Arc::new(storage::index::LibraryIndex::new(&settings.config_dir));

    if let Err(e) = player::run_player(settings, index).await {
        eprintln!("FATAL ERROR: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
