use crate::core::config::Settings;
use crate::core::constants::*;
use crate::core::error::{ChordError, ChordResult};
use crate::player::app::{App, InputMode};
use crate::player::ui::ui;
use crate::storage::index::LibraryIndex;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub mod app;
pub mod audio;
pub mod ui;

use crate::core::remote::{self, Command, RemoteManager, RemoteEvent, pb};
use crate::core::remote::pb::remote_event::Event as PbEvent;
use tokio::sync::mpsc::UnboundedReceiver;
use prost::Message;
use tokio::io::AsyncReadExt;

#[tracing::instrument(skip(settings, index, remote_manager, remote_cmd_rx))]
pub async fn run_player(
    settings: Arc<Settings>,
    index: Arc<LibraryIndex>,
    remote_manager: Arc<RemoteManager>,
    remote_cmd_rx: UnboundedReceiver<Command>,
) -> ChordResult<()> {
    tracing::info!("Initializing TUI and starting player");
    enable_raw_mode().map_err(|e| {
        tracing::error!(error = %e, "Failed to enable raw mode");
        ChordError::Internal(e.to_string())
    })?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to enter alternate screen");
            ChordError::Internal(e.to_string())
        })?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create terminal");
            ChordError::Internal(e.to_string())
        })?;

    // Pass references to App
    let mut app = App::new(&settings, &index, remote_manager).await?;
    app.needs_redraw = true;

    tracing::info!("Starting application loop");
    let res = run_app(&mut terminal, &mut app, remote_cmd_rx).await;

    tracing::info!("Shutting down TUI");
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();

    if let Err(err) = res {
        tracing::error!(error = ?err, "Application error during run");
        eprintln!("\n\x1b[31;1mRUNTIME ERROR\x1b[0m");
        eprintln!("Details: {:?}\n", err);
    }
    Ok(())
}

#[tracing::instrument(skip(terminal, app, remote_cmd_rx))]
async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App<'_>,
    mut remote_cmd_rx: UnboundedReceiver<Command>,
) -> ChordResult<()> {
    let tick_rate = Duration::from_millis(DEFAULT_TICK_RATE_MS);
    let mut last_tick = Instant::now();

    let mut active_remote_id: Option<String> = None;
    let mut active_remote_stream: Option<tokio::net::TcpStream> = None;
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<RemoteEvent>();

    let mut stream_tx: Option<std::sync::mpsc::Sender<Vec<f32>>> = None;

    loop {
        // Handle Remote Commands (Incoming)
        while let Ok(cmd) = remote_cmd_rx.try_recv() {
            tracing::info!(command = ?cmd, "Processing incoming remote command");
            match cmd {
                Command::PlayTrackId(id) => {
                    if let Some(idx) = app.all_tracks.iter().position(|t| t.track_id == id) {
                        let _ = app.play_track(idx);
                    }
                }
                Command::PlayRadioUrl(url) => {
                    if let Some(idx) = app.radio_stations.iter().position(|s| s.url == url) {
                        app.play_radio(idx);
                    }
                }
                Command::TogglePlayback(_) => app.toggle_playback().await,
                Command::NextTrack(_) => {
                    app.next();
                    if let Some(i) = app.list_state.selected() {
                        let _ = app.play_track(i);
                    }
                }
                Command::PrevTrack(_) => {
                    app.previous();
                    if let Some(i) = app.list_state.selected() {
                        let _ = app.play_track(i);
                    }
                }
                Command::SetVolume(v) => {
                    app.volume = v;
                    app.audio.set_volume(v);
                }
                Command::Stop(_) => {
                    app.audio.stop();
                    app.is_playing = false;
                }
                Command::TransferPlaybackTo(_target_id) => {
                    // Logic to transfer playback - for now just stop here
                    app.audio.stop();
                    app.is_playing = false;
                    // The other device should start playing if we implemented full transfer
                }
                Command::StreamSetup(setup) => {
                    tracing::info!("Received StreamSetup: {:?}", setup);
                    let (tx, rx) = std::sync::mpsc::channel();
                    app.audio.play_raw(rx, setup.sample_rate, setup.channels as u16);
                    stream_tx = Some(tx);
                    app.is_playing = true;
                }
                Command::StreamPacket(packet) => {
                    if let Some(tx) = &stream_tx {
                        // Convert bytes back to f32
                        let samples: Vec<f32> = packet.data.chunks_exact(4)
                            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                            .collect();
                        let _ = tx.send(samples);
                    }
                }                Command::BrowseRequest(_) => {
                    // TODO: Implement remote library browsing
                }
                Command::QueueRequest(_) => {
                    // TODO: Implement remote queue management
                }
                Command::Capabilities(_) => {
                    // TODO: Implement capabilities negotiation
                }
                Command::SetMute(mute) => {
                    // TODO: Implement set mute
                    tracing::info!("Received SetMute command: {}", mute);
                }
                Command::SetPlaybackMode(mode) => {
                    // TODO: Implement set playback mode
                    tracing::info!("Received SetPlaybackMode command: {}", mode);
                }
                Command::SeekToMs(ms) => {
                    // TODO: Implement seek to ms
                    tracing::info!("Received SeekToMs command: {}ms", ms);
                }
                Command::SyncRequest(_) => {
                    // TODO: Implement sync request
                    tracing::info!("Received SyncRequest command");
                }
            }
        }

        // Handle Events (from the device we are controlling)
        while let Ok(event) = event_rx.try_recv() {
            if active_remote_id.is_some() {
                if let Some(ev) = event.event {
                    match ev {
                        PbEvent::Status(status) => {
                            app.is_playing = status.is_playing;
                            app.progress = status.progress;
                            app.volume = status.volume;
                            if let Some(id) = status.current_track_id {
                                let current_id = app.current_track.as_ref().map(|t| t.track_id.clone());
                                if current_id != Some(id.clone()) {
                                    if let Some(track) = app.all_tracks.iter().find(|t| t.track_id == id) {
                                        app.current_track = Some(track.clone());
                                    }
                                }
                            }
                            app.needs_redraw = true;
                        }
                        PbEvent::BrowseResponse(res) => {
                            tracing::info!("Received BrowseResponse with {} tracks", res.tracks.len());
                            // TODO: Display remote library results in UI
                        }
                        PbEvent::SyncResponse(sync) => {
                            tracing::debug!("Received SyncResponse: {:?}", sync);
                        }
                        PbEvent::ErrorMessage(msg) => {
                            tracing::error!("Remote Error: {}", msg);
                            app.last_error = Some(format!("Remote Error: {}", msg));
                        }
                    }
                }
            }
        }

        // Update our own status for others to see
        {
            let mut status = app.remote_manager.status.write().await;
            status.current_track_id = app.current_track.as_ref().map(|t| t.track_id.clone());
            status.is_playing = app.is_playing;
            status.progress = app.progress;
            status.volume = app.volume;
            status.title = app.current_track.as_ref().map(|t| t.title.clone()).unwrap_or_default();
            status.artist = app.current_track.as_ref().map(|t| t.artist.clone()).unwrap_or_default();
            status.device_name = app.remote_manager.device_name.clone();
            status.device_id = app.remote_manager.device_id.clone();
        }

        if last_tick.elapsed() >= tick_rate {
            if active_remote_id.is_none() {
                app.update().await;
            }
            last_tick = Instant::now();
        }

        if app.needs_redraw {
            terminal
                .draw(|f| ui(f, app))
                .map_err(|e| ChordError::Internal(e.to_string()))?;
            app.needs_redraw = false;
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(1));

        if event::poll(timeout).map_err(|e| ChordError::Internal(e.to_string()))? {
            if let Event::Key(key) = event::read().map_err(|e| ChordError::Internal(e.to_string()))? {
                if key.kind != event::KeyEventKind::Press {
                    continue;
                }

                let now = std::time::Instant::now();
                if let Some((last_code, last_time)) = app.last_key_event {
                    if last_code == key.code
                        && now.duration_since(last_time).as_millis() < KEY_DEBOUNCE_MS
                    {
                        continue;
                    }
                }
                app.last_key_event = Some((key.code, now));
                app.needs_redraw = true;

                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KEY_RADIO_MODE {
                    if app.input_mode == InputMode::Online {
                        app.input_mode = InputMode::Offline;
                    } else {
                        app.previous_mode = app.input_mode;
                        app.input_mode = InputMode::Online;
                        app.load_radio_stations();
                    }
                    continue;
                }

                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KEY_DEVICES_MODE {
                    if app.input_mode == InputMode::Devices {
                        app.input_mode = app.previous_mode;
                    } else {
                        app.previous_mode = app.input_mode;
                        app.input_mode = InputMode::Devices;
                    }
                    continue;
                }

                match app.input_mode {
                    InputMode::Offline => match key.code {
                        KEY_QUIT => return Ok(()),
                        KEY_TOGGLE_PLAYBACK_1 => {
                            if let Some(ref mut stream) = active_remote_stream {
                                let _ = RemoteManager::send_command(stream, Command::TogglePlayback(true)).await;
                            } else {
                                app.toggle_playback().await;
                            }
                        }
                        KEY_VOL_UP => {
                            app.volume = (app.volume + 0.05).min(1.0);
                            if let Some(ref mut stream) = active_remote_stream {
                                let _ = RemoteManager::send_command(stream, Command::SetVolume(app.volume)).await;
                            } else {
                                app.audio.set_volume(app.volume);
                                let _ = app.save_config();
                            }
                        }
                        KEY_VOL_DOWN => {
                            app.volume = (app.volume - 0.05).max(0.0);
                            if let Some(ref mut stream) = active_remote_stream {
                                let _ = RemoteManager::send_command(stream, Command::SetVolume(app.volume)).await;
                            } else {
                                app.audio.set_volume(app.volume);
                                let _ = app.save_config();
                            }
                        }
                        KEY_LIST_DOWN | KEY_LIST_DOWN_VIM => app.next(),
                        KEY_LIST_UP | KEY_LIST_UP_VIM => app.previous(),
                        KEY_CONFIRM => {
                            if !app.filtered_tracks.is_empty() {
                                if let Some(i) = app.list_state.selected() {
                                    if let Some(track) = app.filtered_tracks.get(i) {
                                        if let Some(remote_id) = &active_remote_id {
                                            let info = app.discovered_devices.iter().find(|d| &d.id == remote_id).cloned();
                                            if let (Some(info), Some(path_str)) = (info, &track.file_path) {
                                                let path = std::path::Path::new(path_str).to_path_buf();
                                                let rm = app.remote_manager.clone();
                                                let mut streamer = crate::core::streamer::AudioStreamer::new();
                                                tokio::spawn(async move {
                                                    if let Ok(stream) = rm.connect_to_device(&info).await {
                                                        let _ = streamer.stream_file(&path, stream).await;
                                                    }
                                                });
                                                tracing::info!(title = %track.title, "Casting track to remote device");
                                            }
                                        } else {
                                            let _ = app.play_track(i);
                                        }
                                    }
                                }
                            }
                        }
                        KEY_NEXT_TRACK_1 | KEY_NEXT_TRACK_2 => {
                            if let Some(ref mut stream) = active_remote_stream {
                                let _ = RemoteManager::send_command(stream, Command::NextTrack(true)).await;
                            } else {
                                app.next();
                                if let Some(i) = app.list_state.selected() {
                                    let _ = app.play_track(i);
                                }
                            }
                        }
                        KEY_PREV_TRACK_1 | KEY_PREV_TRACK_2 => {
                            if let Some(ref mut stream) = active_remote_stream {
                                let _ = RemoteManager::send_command(stream, Command::PrevTrack(true)).await;
                            } else {
                                app.previous();
                                if let Some(i) = app.list_state.selected() {
                                    let _ = app.play_track(i);
                                }
                            }
                        }
                        KEY_SEARCH_MODE => {
                            app.previous_mode = app.input_mode;
                            app.input_mode = InputMode::Search;
                            app.search_query.clear();
                            app.filter_tracks();
                        }
                        KEY_PLAYLIST_MODE => {
                            app.previous_mode = app.input_mode;
                            app.input_mode = InputMode::PlaylistSelect;
                        }
                        KEY_REFRESH => {
                            let _ = app.refresh_library().await;
                        }
                        _ => {}
                    },

                    InputMode::Search => match key.code {
                        KEY_CONFIRM | KEY_SEARCH_MODE | KeyCode::Esc => {
                            app.input_mode = app.previous_mode;
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                            if app.previous_mode == InputMode::Online {
                                app.filter_radio();
                            } else {
                                app.filter_tracks();
                            }
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                            if app.previous_mode == InputMode::Online {
                                app.filter_radio();
                            } else {
                                app.filter_tracks();
                            }
                        }
                        _ => {}
                    },

                    InputMode::Online => match key.code {
                        KEY_QUIT => return Ok(()),
                        KEY_TOGGLE_PLAYBACK_1 => {
                            if let Some(ref mut stream) = active_remote_stream {
                                let _ = RemoteManager::send_command(stream, Command::TogglePlayback(true)).await;
                            } else {
                                app.toggle_playback().await;
                            }
                        }
                        KEY_VOL_UP => {
                            app.volume = (app.volume + 0.05).min(1.0);
                            if let Some(ref mut stream) = active_remote_stream {
                                let _ = RemoteManager::send_command(stream, Command::SetVolume(app.volume)).await;
                            } else {
                                app.audio.set_volume(app.volume);
                            }
                        }
                        KEY_VOL_DOWN => {
                            app.volume = (app.volume - 0.05).max(0.0);
                            if let Some(ref mut stream) = active_remote_stream {
                                let _ = RemoteManager::send_command(stream, Command::SetVolume(app.volume)).await;
                            } else {
                                app.audio.set_volume(app.volume);
                            }
                        }
                        KEY_LIST_DOWN | KEY_LIST_DOWN_VIM => {
                            let len = app.filtered_stations.len();
                            if len > 0 {
                                let i = (app.radio_list_state.selected().unwrap_or(0) + 1) % len;
                                app.radio_list_state.select(Some(i));
                            }
                        }
                        KEY_LIST_UP | KEY_LIST_UP_VIM => {
                            let len = app.filtered_stations.len();
                            if len > 0 {
                                let i = (app.radio_list_state.selected().unwrap_or(0) + len - 1) % len;
                                app.radio_list_state.select(Some(i));
                            }
                        }
                        KEY_CONFIRM => {
                            if let Some(i) = app.radio_list_state.selected() {
                                if let Some(station) = app.filtered_stations.get(i) {
                                    if let Some(ref mut stream) = active_remote_stream {
                                        let _ = RemoteManager::send_command(stream, Command::PlayRadioUrl(station.url.clone())).await;
                                    } else {
                                        app.play_radio(i);
                                    }
                                }
                            }
                        }
                        KEY_SEARCH_MODE => {
                            app.previous_mode = app.input_mode;
                            app.input_mode = InputMode::Search;
                            app.search_query.clear();
                            app.filter_radio();
                        }
                        _ => {}
                    },

                    InputMode::PlaylistSelect => match key.code {
                        KEY_QUIT => return Ok(()),
                        KEY_LIST_DOWN | KEY_LIST_DOWN_VIM => app.next_playlist(),
                        KEY_LIST_UP | KEY_LIST_UP_VIM => app.previous_playlist(),
                        KEY_CONFIRM => {
                            if let Some(idx) = app.playlist_list_state.selected() {
                                if let Some(p) = app.playlists.get(idx) {
                                    let p_clone = p.clone();
                                    app.select_playlist(Some(p_clone)).await;
                                }
                            }
                            app.input_mode = InputMode::Offline;
                        }
                        KEY_PLAYLIST_MODE => app.input_mode = InputMode::Offline,
                        _ => {}
                    },

                    InputMode::Devices => match key.code {
                        KeyCode::Esc => app.input_mode = app.previous_mode,
                        KEY_LIST_DOWN | KEY_LIST_DOWN_VIM => {
                            let len = app.remote_manager.discovered_devices.read().await.len() + 1; // +1 for Local
                            let i = (app.list_state.selected().unwrap_or(0) + 1) % len;
                            app.list_state.select(Some(i));
                        }
                        KEY_LIST_UP | KEY_LIST_UP_VIM => {
                            let len = app.remote_manager.discovered_devices.read().await.len() + 1;
                            let i = (app.list_state.selected().unwrap_or(0) + len - 1) % len;
                            app.list_state.select(Some(i));
                        }
                        KEY_CONFIRM => {
                            let idx = app.list_state.selected().unwrap_or(0);
                            if idx == 0 {
                                tracing::info!("Switching to LOCAL playback");
                                active_remote_id = None;
                                active_remote_stream = None;
                                app.is_slave = false;
                                app.input_mode = InputMode::Offline;
                            } else {
                                let devices = app.remote_manager.discovered_devices.read().await;
                                if let Some((id, info)) = devices.iter().nth(idx - 1) {
                                    tracing::info!(id = %id, name = %info.name, "Attempting to switch to REMOTE device");
                                    if let Ok(stream) = app.remote_manager.connect_to_device(info).await {
                                        tracing::info!(id = %id, "Successfully connected to remote device");
                                        active_remote_id = Some(id.clone());
                                        active_remote_stream = Some(stream);
                                        app.is_slave = true;
                                        app.input_mode = InputMode::Offline;

                                        // Start event listener for this stream
                                        let mut stream_clone = app.remote_manager.connect_to_device(info).await.unwrap();
                                        let event_tx_clone = event_tx.clone();
                                        let remote_name = info.name.clone();
                                        tokio::spawn(async move {
                                            tracing::debug!(device = %remote_name, "Starting remote event listener task");
                                            loop {
                                                let mut len_buf = [0u8; 4];
                                                if stream_clone.read_exact(&mut len_buf).await.is_err() { break; }
                                                let len = u32::from_be_bytes(len_buf) as usize;
                                                let mut msg_buf = vec![0u8; len];
                                                if stream_clone.read_exact(&mut msg_buf).await.is_err() { break; }
                                                if let Ok(ev) = RemoteEvent::decode(&msg_buf[..]) {
                                                    let _ = event_tx_clone.send(ev);
                                                }
                                            }
                                            tracing::debug!(device = %remote_name, "Remote event listener task finished");
                                        });
                                    } else {
                                        tracing::error!(id = %id, "Failed to connect to remote device");
                                    }
                                }
                            }
                        }
                        _ => {}
                    },
                }
                app.needs_redraw = true;
            }
        }
    }
}
