use crate::core::config::Settings;
use crate::core::constants::*;
use crate::player::app::{App, InputMode};
use crate::player::ui::ui;
use crate::storage::index::LibraryIndex;
use crate::core::error::{ChordError, ChordResult};
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

pub async fn run_player(settings: Arc<Settings>, index: Arc<LibraryIndex>) -> ChordResult<()> {
    enable_raw_mode().map_err(|e| ChordError::Internal(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).map_err(|e| ChordError::Internal(e.to_string()))?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout)).map_err(|e| ChordError::Internal(e.to_string()))?;
    
    // Pass references to App
    let mut app = App::new(&settings, &index).await?;
    app.needs_redraw = true; 
    let res = run_app(&mut terminal, &mut app).await;
    
    disable_raw_mode().map_err(|e| ChordError::Internal(e.to_string()))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    ).map_err(|e| ChordError::Internal(e.to_string()))?;
    terminal.show_cursor().map_err(|e| ChordError::Internal(e.to_string()))?;
    
    if let Err(err) = res {
        println!("ERROR: {:?}", err);
    }
    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App<'_>) -> ChordResult<()> {
    let tick_rate = Duration::from_millis(DEFAULT_TICK_RATE_MS);
    let mut last_tick = Instant::now();

    loop {
        if last_tick.elapsed() >= tick_rate {
            app.update().await;
            last_tick = Instant::now();
        }

        if app.needs_redraw {
            terminal.draw(|f| ui(f, app)).map_err(|e| ChordError::Internal(e.to_string()))?;
            app.needs_redraw = false;
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(1));

        if event::poll(timeout).map_err(|e| ChordError::Internal(e.to_string()))? {
            while event::poll(Duration::from_secs(0)).map_err(|e| ChordError::Internal(e.to_string()))? {
                if let Event::Key(key) = event::read().map_err(|e| ChordError::Internal(e.to_string()))? {
                    if key.kind != event::KeyEventKind::Press {
                        continue;
                    }

                    let now = std::time::Instant::now();
                    if let Some((last_code, last_time)) = app.last_key_event {
                        if last_code == key.code && now.duration_since(last_time).as_millis() < KEY_DEBOUNCE_MS {
                            continue;
                        }
                    }
                    app.last_key_event = Some((key.code, now));
                    app.needs_redraw = true;

                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        if key.code == KEY_RADIO_MODE {
                            if app.input_mode == InputMode::Online {
                                app.input_mode = InputMode::Offline;
                            } else {
                                app.previous_mode = app.input_mode;
                                app.input_mode = InputMode::Online;
                                app.load_radio_stations();
                            }
                            continue;
                        }
                    }

                    match app.input_mode {
                        InputMode::Offline => {
                            match key.code {
                                KEY_QUIT => return Ok(()),
                                KEY_TOGGLE_PLAYBACK_1 | KEY_TOGGLE_PLAYBACK_2 => app.toggle_playback().await,
                                KEY_VOL_UP_1 | KEY_VOL_UP_2 => {
                                    app.volume = (app.volume + 0.05).min(1.0);
                                    app.audio.set_volume(app.volume);
                                    let _ = app.save_config().await;
                                }
                                KEY_VOL_DOWN => {
                                    app.volume = (app.volume - 0.05).max(0.0);
                                    app.audio.set_volume(app.volume);
                                    let _ = app.save_config().await;
                                }
                                KEY_LIST_DOWN | KEY_LIST_DOWN_VIM => app.next(),
                                KEY_LIST_UP | KEY_LIST_UP_VIM => app.previous(),
                                KEY_CONFIRM => {
                                    if !app.filtered_tracks.is_empty() {
                                        if let Some(i) = app.list_state.selected() { let _ = app.play_track(i).await; }
                                    }
                                }
                                KEY_NEXT_TRACK_1 | KEY_NEXT_TRACK_2 => {
                                    if !app.filtered_tracks.is_empty() {
                                        app.next();
                                        if let Some(i) = app.list_state.selected() { let _ = app.play_track(i).await; }
                                    }
                                }
                                KEY_PREV_TRACK_1 | KEY_PREV_TRACK_2 => {
                                    if !app.filtered_tracks.is_empty() {
                                        app.previous();
                                        if let Some(i) = app.list_state.selected() { let _ = app.play_track(i).await; }
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
                                _ => {}
                            }
                        }

                        InputMode::Search => {
                            if key.code == KEY_CONFIRM {
                                app.input_mode = app.previous_mode;
                            } else if let KeyCode::Char(c) = key.code {
                                app.search_query.push(c);
                                if app.previous_mode == InputMode::Online { app.filter_radio(); } else { app.filter_tracks(); }
                            } else if key.code == KeyCode::Backspace {
                                app.search_query.pop();
                                if app.previous_mode == InputMode::Online { app.filter_radio(); } else { app.filter_tracks(); }
                            }
                        }

                        InputMode::Online => {
                            match key.code {
                                KEY_QUIT => return Ok(()),
                                KEY_TOGGLE_PLAYBACK_1 | KEY_TOGGLE_PLAYBACK_2 => app.toggle_playback().await,
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
                                    if let Some(i) = app.radio_list_state.selected() { let _ = app.play_radio(i).await; }
                                }
                                KEY_SEARCH_MODE => {
                                    app.previous_mode = app.input_mode;
                                    app.input_mode = InputMode::Search;
                                    app.search_query.clear();
                                    app.filter_radio();
                                }
                                _ => {}
                            }
                        }

                        InputMode::PlaylistSelect => {
                            match key.code {
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
                            }
                        }
                    }
                }
            }
        }
    }
}
