mod core;
mod player;
mod storage;

use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _log_guards = core::logger::init_logger();
    tracing::info!("Starting Chord...");

    let settings_raw = core::config::Settings::new()?;
    let settings = Arc::new(settings_raw);
    let index = Arc::new(storage::index::LibraryIndex::new(&settings.config_dir));

    let (device_id, device_name, disable_broadcast) = {
        let config = settings.config.read().unwrap();
        (
            config.remote.device_id.clone(),
            config.remote.device_name.clone(),
            config.remote.disable_broadcast,
        )
    };

    let remote_manager =
        Arc::new(core::remote::RemoteManager::new(device_id, device_name, disable_broadcast));

    // Channel for receiving remote commands
    let (remote_cmd_tx, remote_cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    
    // Start remote services (discovery and control server)
    remote_manager.start_services(remote_cmd_tx).await?;

    // Always run as a potential player and controller
    if let Err(e) = player::run_player(settings, index, remote_manager, remote_cmd_rx).await {
        eprintln!("FATAL ERROR: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
