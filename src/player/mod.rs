use crate::core::config::Settings;
use crate::storage::index::LibraryIndex;
use crate::player::app::{App, InputMode};
use crate::player::ui::ui;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
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

pub mod audio;
pub mod app;
pub mod ui;

use crate::config::*;

pub async fn run_player(settings: Arc<Settings>, index: Arc<LibraryIndex>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let mut app = App::new(settings, index).await?;
    app.needs_redraw = true; // Ensure first draw happens
    let res = run_app(&mut terminal, &mut app).await;
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    if let Err(err) = res {
        println!("ERROR: {:?}", err);
    }
    Ok(())
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App<'_>,
) -> io::Result<()> {
    let tick_rate = Duration::from_millis(DEFAULT_TICK_RATE_MS);
    let mut last_tick = Instant::now();

    loop {
        if last_tick.elapsed() >= tick_rate {
            app.update().await;
            last_tick = Instant::now();
        }

        if app.needs_redraw {
            terminal.draw(|f| ui(f, app))?;
            app.needs_redraw = false;
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(1));
        
        if event::poll(timeout)? {
            while event::poll(Duration::from_secs(0))? {
                if let Event::Key(key) = event::read()? {
                    // Only process Press events, ignore Repeat (long-press) and Release
                    if key.kind != event::KeyEventKind::Press {
                        continue;
                    }

                    // Software debouncer to prevent rapid repeats on some terminals
                    let now = std::time::Instant::now();
                    if let Some((last_code, last_time)) = app.last_key_event {
                        if last_code == key.code && now.duration_since(last_time).as_millis() < KEY_DEBOUNCE_MS {
                            continue;
                        }
                    }
                    app.last_key_event = Some((key.code, now));

                    app.needs_redraw = true;

                    // Global keys (Playback, Volume, etc.)
                    if key.code == KEY_QUIT {
                        return Ok(());
                    } else if key.code == KEY_TOGGLE_PLAYBACK_1 || key.code == KEY_TOGGLE_PLAYBACK_2 {
                        app.toggle_playback().await;
                    } else if key.code == KEY_NEXT_TRACK_1 || key.code == KEY_NEXT_TRACK_2 {
                        app.next();
                        if let Some(i) = app.list_state.selected() {
                            app.play_track(i).await;
                        }
                    } else if key.code == KEY_PREV_TRACK_1 || key.code == KEY_PREV_TRACK_2 {
                        app.previous();
                        if let Some(i) = app.list_state.selected() {
                            app.play_track(i).await;
                        }
                    } else if key.code == KEY_VOL_UP_1 || key.code == KEY_VOL_UP_2 {
                        app.volume = (app.volume + 0.05).min(1.0);
                        app.audio.set_volume(app.volume);
                        app.save_config().await;
                    } else if key.code == KEY_VOL_DOWN {
                        app.volume = (app.volume - 0.05).max(0.0);
                        app.audio.set_volume(app.volume);
                        app.save_config().await;
                    } else if key.code == KEY_CYCLE_DEVICE {
                        app.audio.next_device();
                        app.save_config().await;
                    } else if key.code == KEY_REFRESH {
                        let index = app.index.clone();
                        let music_dir = app.settings.config.library.music_dir.clone();
                        let refresh_tx = app.refresh_tx.clone();
                        tokio::spawn(async move {
                            let _ = index.update_index(&music_dir).await;
                            let _ = refresh_tx.send(());
                        });
                    }

                    match app.input_mode {                        InputMode::Search => {
                            if key.code == KEY_CONFIRM || key.code == KEY_BACK {
                                app.input_mode = InputMode::Normal;
                            } else if let KeyCode::Char(c) = key.code {
                                app.search_query.push(c);
                                app.filter_tracks();
                            } else if key.code == KeyCode::Backspace {
                                app.search_query.pop();
                                app.filter_tracks();
                            }
                        }
                        InputMode::PlaylistSelect => {
                            if key.code == KEY_BACK || key.code == KEY_PLAYLIST_MODE {
                                app.input_mode = InputMode::Normal;
                            } else if key.code == KEY_LIST_DOWN_VIM || key.code == KEY_LIST_DOWN {
                                app.next_playlist();
                            } else if key.code == KEY_LIST_UP_VIM || key.code == KEY_LIST_UP {
                                app.previous_playlist();
                            } else if key.code == KEY_CONFIRM {
                                let idx = app.playlist_list_state.selected().unwrap_or(0);
                                if idx == 0 {
                                    app.select_playlist(None).await;
                                } else if let Some(p) = app.playlists.get(idx - 1) {
                                    let p_clone = p.clone();
                                    app.select_playlist(Some(p_clone)).await;
                                }
                                app.input_mode = InputMode::Normal;
                            }
                        }
                        InputMode::Normal => {
                            if key.code == KEY_PLAYLIST_MODE {
                                app.input_mode = InputMode::PlaylistSelect;
                            } else if key.code == KEY_SEARCH_MODE {
                                app.input_mode = InputMode::Search;
                                app.search_query.clear();
                                app.filter_tracks();
                            } else if key.code == KEY_LIST_DOWN_VIM || key.code == KEY_LIST_DOWN {
                                app.next();
                            } else if key.code == KEY_LIST_UP_VIM || key.code == KEY_LIST_UP {
                                app.previous();
                            } else if key.code == KEY_CONFIRM {
                                if let Some(i) = app.list_state.selected() {
                                    app.play_track(i).await;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
