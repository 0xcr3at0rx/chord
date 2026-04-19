mod core;
mod player;
mod storage;

use anyhow::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let settings_raw = core::config::Settings::new()?;
    let settings = Arc::new(settings_raw);
    let index = Arc::new(storage::index::LibraryIndex::new(&settings.config_dir));

    player::run_player(settings, index).await?;

    Ok(())
}
