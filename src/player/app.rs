use crate::core::config::Settings;
use crate::core::models::TrackMetadata;
use crate::storage::index::LibraryIndex;
use crate::player::audio::AudioPlayer;
use anyhow::Result;
use ratatui::widgets::ListState;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(PartialEq, Clone, Copy)]
pub enum InputMode {
    Normal,
    Search,
    PlaylistSelect,
    Config,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ConfigField {
    MusicDir,
    AudioDevice,
    AudioMode,
    ScanAtStartup,
    ThemeBg,
    ThemeAccent,
}

#[derive(Clone, Debug)]
pub struct Playlist {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct TrackMetadataUpdate {
    pub idx: usize,
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
    pub bit_depth: u8,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub cover_art: Option<Vec<u8>>,
    pub duration: Duration,
}

/// Application state and business logic for the TUI player.
pub struct App<'a> {
    /// All tracks in the library, sorted by artist and album.
    pub all_tracks: Vec<Arc<TrackMetadata>>,
    /// Tracks currently displayed in the sidebar (either all, from a playlist, or filtered by search).
    pub filtered_tracks: Vec<Arc<TrackMetadata>>,
    /// List of available playlists from the database.
    pub playlists: Vec<Playlist>,
    /// The currently active playlist, if any.
    pub current_playlist: Option<Playlist>,
    /// Full tracklist of the current playlist, used as the source for search filtering.
    pub current_playlist_tracks: Option<Vec<Arc<TrackMetadata>>>,
    /// Selection state for the main track list.
    pub list_state: ListState,
    /// Selection state for the playlist selection sidebar.
    pub playlist_list_state: ListState,
    /// Selection state for the configuration menu.
    pub config_list_state: ListState,
    /// List of fields available in config mode.
    pub config_fields: Vec<ConfigField>,
    /// Current input mode (Normal, Search, PlaylistSelect, etc.).
    pub input_mode: InputMode,
    /// Current search query string.
    pub search_query: String,
    /// Metadata of the track currently being played.
    pub current_track: Option<Arc<TrackMetadata>>,
    /// The track list context (playlist/filter) from which playback was started.
    pub playback_track_list: Vec<Arc<TrackMetadata>>,
    /// Index of the track currently being played in 'filtered_tracks'.
    pub playing_idx: Option<usize>,
    pub is_playing: bool,
    pub is_starting: bool,
    pub volume: f32,
    pub progress: f32,
    pub current_track_duration: Duration,
    pub current_pos: Duration,
    pub accumulated_pos: Duration,
    pub playback_start: Option<Instant>,
    pub lyrics: Vec<crate::player::audio::LyricLine>,
    pub current_lyric_idx: usize,
    pub lyrics_scroll: u16,
    pub auto_scroll: bool,
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
    pub bit_depth: u8,
    pub current_genre: Option<String>,
    pub current_label: Option<String>,
    pub current_description: Option<String>,
    pub current_cover_art: Option<Vec<u8>>,
    pub cached_image: Option<image::DynamicImage>,
    pub image_state: Option<ratatui_image::protocol::StatefulProtocol>,
    pub image_picker: Option<ratatui_image::picker::Picker>,
    pub last_key_event: Option<(crossterm::event::KeyCode, std::time::Instant)>,
    pub last_error: Option<String>,
    pub audio: AudioPlayer,
    pub settings: Arc<Settings>,
    pub theme: crate::config::Theme,
    pub index: Arc<LibraryIndex>,
    pub metadata_rx: mpsc::UnboundedReceiver<TrackMetadataUpdate>,
    pub metadata_tx: mpsc::UnboundedSender<TrackMetadataUpdate>,
    pub refresh_rx: mpsc::UnboundedReceiver<()>,
    pub refresh_tx: mpsc::UnboundedSender<()>,
    /// Flag to indicate that the UI needs to be redrawn in the next frame.
    pub needs_redraw: bool,
    pub _title: &'a str,
}

impl<'a> App<'a> {
    pub async fn new(
        settings: Arc<Settings>,
        index: Arc<LibraryIndex>,
    ) -> Result<App<'a>> {
        let (metadata_tx, metadata_rx) = mpsc::unbounded_channel();
        let (refresh_tx, refresh_rx) = mpsc::unbounded_channel();

        {
            let config = settings.config.read().unwrap();
            if config.library.scan_at_startup {
                let index_clone = index.clone();
                let music_dir = config.library.music_dir.clone();
                let refresh_tx_clone = refresh_tx.clone();
                tokio::spawn(async move {
                    let _ = index_clone.update_index(&music_dir).await;
                    let _ = refresh_tx_clone.send(());
                });
            }
        }

        let mut tracks = index.get_all_tracks().await;
        tracks.sort_by(|a, b| a.artist.cmp(&b.artist).then(a.album.as_deref().unwrap_or("").cmp(b.album.as_deref().unwrap_or(""))));

        let mut list_state = ListState::default();
        if !tracks.is_empty() {
            list_state.select(Some(0));
        }

        let p_rows = index.get_playlists().await;
        let playlists = p_rows.into_iter().map(|(id, name)| Playlist { id, name }).collect::<Vec<_>>();
        let mut playlist_list_state = ListState::default();
        if !playlists.is_empty() {
            playlist_list_state.select(Some(0));
        }

        let config_fields = vec![
            ConfigField::MusicDir,
            ConfigField::AudioDevice,
            ConfigField::AudioMode,
            ConfigField::ScanAtStartup,
            ConfigField::ThemeBg,
            ConfigField::ThemeAccent,
        ];
        let mut config_list_state = ListState::default();
        config_list_state.select(Some(0));

        let audio = AudioPlayer::new();
        {
            let config = settings.config.read().unwrap();
            audio.set_volume(config.audio.volume);
            audio.set_mode(&config.audio.mode);

            if let Some(preferred_name) = &config.audio.device_name {
                audio.try_init_with_name(preferred_name);
            } else {
                audio.try_init();
            }
        }

        let theme = settings.config.read().unwrap().theme.to_theme();

        Ok(App {
            all_tracks: tracks.clone(),
            filtered_tracks: tracks,
            playlists,
            current_playlist: None,
            current_playlist_tracks: None,
            list_state,
            playlist_list_state,
            config_list_state,
            config_fields,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            current_track: None,
            playback_track_list: Vec::new(),
            playing_idx: None,
            is_playing: false,
            is_starting: false,
            volume: settings.config.read().unwrap().audio.volume,
            progress: 0.0,
            current_track_duration: Duration::from_secs(0),
            current_pos: Duration::from_secs(0),
            accumulated_pos: Duration::from_secs(0),
            playback_start: None,
            lyrics: Vec::new(),
            current_lyric_idx: 0,
            lyrics_scroll: 0,
            auto_scroll: true,
            sample_rate: 0,
            channels: 0,
            bitrate: 0,
            bit_depth: 0,
            current_genre: None,
            current_label: None,
            current_description: None,
            current_cover_art: None,
            cached_image: None,
            image_state: None,
            image_picker: ratatui_image::picker::Picker::from_query_stdio().ok(),
            last_key_event: None,
            last_error: None,
            audio,
            settings: settings.clone(),
            theme,
            index: index.clone(),
            metadata_rx,
            metadata_tx,
            refresh_rx,
            refresh_tx,
            needs_redraw: true,
            _title: crate::config::APP_NAME,
        })
    }


    pub async fn select_playlist(&mut self, playlist: Option<Playlist>) {
        self.current_playlist = playlist.clone();
        if let Some(p) = playlist {
            let music_dir = self.settings.config.read().unwrap().library.music_dir.clone();
            let mut tracks = self.index.get_playlist_tracks(&p.id, &music_dir).await;
            tracks.sort_by(|a, b| a.artist.cmp(&b.artist).then(a.album.as_deref().unwrap_or("").cmp(b.album.as_deref().unwrap_or(""))));
            self.current_playlist_tracks = Some(tracks.clone());
            self.filtered_tracks = tracks;
        } else {
            self.current_playlist_tracks = None;
            self.filtered_tracks = self.all_tracks.clone();
        }
        self.filter_tracks();
    }

    pub async fn refresh_library(&mut self) {
        let mut tracks = self.index.get_all_tracks().await;
        tracks.sort_by(|a, b| a.artist.cmp(&b.artist).then(a.album.as_deref().unwrap_or("").cmp(b.album.as_deref().unwrap_or(""))));
        self.all_tracks = tracks.clone();
        
        let p_rows = self.index.get_playlists().await;
        self.playlists = p_rows.into_iter().map(|(id, name)| Playlist { id, name }).collect();

        if self.current_playlist.is_none() {
            self.filtered_tracks = tracks;
        } else {
            // Re-select current playlist to update its tracks
            let p = self.current_playlist.clone();
            self.select_playlist(p).await;
        }
        self.filter_tracks();
        self.needs_redraw = true;
    }

    pub fn next_playlist(&mut self) {
        let len = self.playlists.len() + 1; // +1 for "All ( Library )"
        let i = match self.playlist_list_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.playlist_list_state.select(Some(i));
    }

    pub fn previous_playlist(&mut self) {
        let len = self.playlists.len() + 1; // +1 for "All ( Library )"
        let i = match self.playlist_list_state.selected() {
            Some(i) => (i + len - 1) % len,
            None => 0,
        };
        self.playlist_list_state.select(Some(i));
    }

    pub fn filter_tracks(&mut self) {
        let query = self.search_query.to_lowercase();
        
        let source = if let Some(playlist_tracks) = &self.current_playlist_tracks {
            playlist_tracks
        } else {
            &self.all_tracks
        };

        if query.is_empty() {
            self.filtered_tracks = source.clone();
        } else {
            self.filtered_tracks = source
                .iter()
                .filter(|t| {
                    t.title.to_lowercase().contains(&query) || t.artist.to_lowercase().contains(&query)
                })
                .cloned()
                .collect();
        }
        
        // No need to sort here; source is already sorted and filter preserves order.
        self.list_state.select(if self.filtered_tracks.is_empty() {
            None
        } else {
            Some(0)
        });
    }

    pub fn next(&mut self) {
        let len = self.filtered_tracks.len();
        if len == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= len - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let len = self.filtered_tracks.len();
        if len == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub async fn toggle_playback(&mut self) {
        if self.audio.is_empty() {
            if let Some(idx) = self.list_state.selected() {
                self.is_starting = true;
                self.play_track(idx).await;
            }
        } else {
            if self.is_playing {
                self.audio.pause();
                self.is_playing = false;
                if let Some(start) = self.playback_start {
                    self.accumulated_pos += start.elapsed();
                }
                self.playback_start = None;
            } else {
                self.audio.resume();
                self.is_playing = true;
                self.playback_start = Some(Instant::now());
            }
        }
    }

    pub async fn play_track(&mut self, idx: usize) {
        self.is_playing = false;
        self.is_starting = true;
        let track = if let Some(t) = self.filtered_tracks.get(idx) {
            t.clone()
        } else {
            return;
        };

        self.current_genre = None;
        self.current_label = None;
        self.current_description = None;
        self.current_cover_art = None;
        self.sample_rate = 0;
        self.channels = 0;
        self.bitrate = 0;
        self.bit_depth = 0;

        if let Some(path_str) = &track.file_path {
            let path = PathBuf::from(path_str);
            if path.exists() {
                // Start playback immediately for responsiveness
                self.audio.play(path.clone());
                self.is_playing = true;
                self.playing_idx = Some(idx);
                self.current_track = Some(track.clone());
                self.playback_track_list = self.filtered_tracks.clone();
                self.accumulated_pos = Duration::from_secs(0);
                self.current_pos = Duration::from_secs(0);
                self.playback_start = Some(Instant::now());
                self.load_lyrics(&path);
                self.last_error = None;

                // Offload metadata and cover art loading to background
                let path_clone = path.clone();
                let tx = self.metadata_tx.clone();
                let idx_clone = idx;

                tokio::task::spawn_blocking(move || {
                    use lofty::file::AudioFile;
                    use lofty::prelude::*;
                    
                    let duration = crate::player::audio::probe_duration(&path_clone);

                    if let Ok(probed) = lofty::read_from_path(&path_clone) {
                        let props = probed.properties();
                        let sr = props.sample_rate().unwrap_or(0);
                        let ch = props.channels().unwrap_or(0);
                        let mut br = props.audio_bitrate().unwrap_or(0);
                        let bd = props.bit_depth().unwrap_or(0);

                        if br < 32 {
                            if let Ok(fs_metadata) = std::fs::metadata(&path_clone) {
                                let file_size = fs_metadata.len();
                                let dur = duration.unwrap_or_else(|| props.duration());
                                let dur_secs = dur.as_secs_f64();
                                if dur_secs > 0.1 {
                                    br = ((file_size as f64 * 8.0) / (dur_secs * 1000.0)) as u32;
                                }
                            }
                        }
                        if br > 10000 { br /= 1000; }

                        let final_duration = duration.unwrap_or_else(|| props.duration());
                        let mut genre = None;
                        let mut label = None;
                        let mut desc = None;
                        let mut art = None;

                        if let Some(tag) = probed.primary_tag() {
                            genre = tag.genre().map(|s| s.to_string());
                            label = tag.get_string(&lofty::tag::ItemKey::Label)
                                .map(|s| s.to_string())
                                .or_else(|| tag.get_string(&lofty::tag::ItemKey::Publisher).map(|s| s.to_string()));
                            desc = tag.get_string(&lofty::tag::ItemKey::Comment).map(|s| s.to_string());

                            if let Some(picture) = tag.pictures().first() {
                                art = Some(picture.data().to_vec());
                            }
                        }

                        let _ = tx.send(TrackMetadataUpdate {
                            idx: idx_clone,
                            sample_rate: sr,
                            channels: ch,
                            bitrate: br,
                            bit_depth: bd,
                            genre,
                            label,
                            description: desc,
                            cover_art: art,
                            duration: final_duration,
                        });
                    }
                });
            } else {
                self.last_error = Some(format!("FILE NOT FOUND: {}", path_str));
            }
        }
    }

    fn load_lyrics(&mut self, audio_path: &Path) {
        self.lyrics.clear();
        for ext in ["lrc", "txt"] {
            if let Ok(content) = std::fs::read_to_string(audio_path.with_extension(ext)) {
                if ext == "lrc" {
                    self.parse_lrc(&content);
                } else {
                    self.lyrics = content
                        .lines()
                        .map(|l| crate::player::audio::LyricLine {
                            time: Duration::from_secs(0),
                            text: l.to_string(),
                        })
                        .collect();
                }
                break;
            }
        }
        if self.lyrics.is_empty() {
            self.lyrics.push(crate::player::audio::LyricLine {
                time: Duration::from_secs(0),
                text: "NO LYRICS".into(),
            });
        }
    }

    fn parse_lrc(&mut self, content: &str) {
        let re = Regex::new(r"\[(\d+):(\d+(?:\.\d+)?)\](.*)").unwrap();
        for line in content.lines() {
            if let Some(caps) = re.captures(line) {
                let mins = caps[1].parse::<u64>().unwrap_or(0);
                let secs = caps[2].parse::<f64>().unwrap_or(0.0);
                self.lyrics.push(crate::player::audio::LyricLine {
                    time: Duration::from_secs(mins * 60) + Duration::from_secs_f64(secs),
                    text: caps[3].trim().to_string(),
                });
            }
        }
        self.lyrics.sort_by_key(|l| l.time);
    }

    pub async fn handle_config_toggle(&mut self, field: ConfigField) {
        match field {
            ConfigField::AudioMode => {
                let current = self.audio.mode.lock().unwrap().clone();
                let next = if current == "PIPEWIRE" { "ALSA" } else { "PIPEWIRE" };
                self.audio.set_mode(next);
                self.save_config().await;
            }
            ConfigField::ScanAtStartup => {
                {
                    let mut config = self.settings.config.write().unwrap();
                    config.library.scan_at_startup = !config.library.scan_at_startup;
                }
                self.save_config().await;
            }
            ConfigField::AudioDevice => {
                self.audio.next_device();
                self.save_config().await;
            }
            _ => {
                // Other fields might need a text input or color picker, 
                // which is more complex. For now, we'll just log or ignore.
            }
        }
        self.needs_redraw = true;
    }

    pub async fn save_config(&self) {
        {
            let mut config = self.settings.config.write().unwrap();
            config.audio.device_name = self.audio.device_name.lock().unwrap().clone();
            config.audio.volume = self.volume;
            config.audio.mode = self.audio.mode.lock().unwrap().clone();
        }

        let config_file = self.settings.config_dir.join("config.toml");
        let _ = self.settings.save_config(&config_file);
    }

    pub async fn update(&mut self) {
        // Drain refresh signals
        while let Ok(_) = self.refresh_rx.try_recv() {
            self.refresh_library().await;
        }

        // Drain metadata updates
        while let Ok(update) = self.metadata_rx.try_recv() {
            if self.playing_idx == Some(update.idx) {
                self.sample_rate = update.sample_rate;
                self.channels = update.channels;
                self.bitrate = update.bitrate;
                self.bit_depth = update.bit_depth;
                self.current_genre = update.genre;
                self.current_label = update.label;
                self.current_description = update.description;
                self.current_cover_art = update.cover_art;
                self.current_track_duration = update.duration;

                // Update cached image and reset UI protocol
                if let Some(art) = &self.current_cover_art {
                    if let Ok(img) = image::load_from_memory(art) {
                        self.cached_image = Some(img);
                        self.image_state = None;
                    }
                }
                
                self.needs_redraw = true;
            }
        }

        let is_empty = self.audio.is_empty();
        let has_error = self.audio.has_error();
        let is_init = self.audio.is_initializing();

        if !is_empty {
            self.is_starting = false;
        }

        if self.is_playing && !is_empty {
            if let Some(start) = self.playback_start {
                self.current_pos = self.accumulated_pos + start.elapsed();
            }
            if self.current_track_duration.as_secs_f64() > 0.0 {
                self.progress = (self.current_pos.as_secs_f64()
                    / self.current_track_duration.as_secs_f64())
                    as f32;
                self.progress = self.progress.clamp(0.0, 1.0);
            }
            if !self.lyrics.is_empty()
                && self.lyrics.iter().any(|l| l.time > Duration::from_secs(0))
            {
                let mut idx = 0;
                for (i, l) in self.lyrics.iter().enumerate() {
                    if l.time <= self.current_pos {
                        idx = i;
                    } else {
                        break;
                    }
                }
                self.current_lyric_idx = idx;
                if self.auto_scroll {
                    self.lyrics_scroll = self.current_lyric_idx.saturating_sub(5) as u16;
                }
            }

            if self.progress >= 0.999 && !self.playback_track_list.is_empty() {
                let next = self
                    .playing_idx
                    .map(|i| (i + 1) % self.playback_track_list.len())
                    .unwrap_or(0);
                self.play_track(next).await;
            }
        } else if self.is_playing && is_empty && !is_init && !self.is_starting && !self.playback_track_list.is_empty() {
            // Auto-advance on end of track OR if the file failed to play (broken track)
            let next = self
                .playing_idx
                .map(|i| (i + 1) % self.playback_track_list.len())
                .unwrap_or(0);
            
            if has_error {
                let detail = self.audio.last_error.lock().unwrap().clone();
                let error_msg = format!("SKIPPING BROKEN TRACK: {}", detail.unwrap_or_default());
                
                // Only update last_error if it's different to avoid redundant UI triggers
                if self.last_error.as_ref() != Some(&error_msg) {
                    self.last_error = Some(error_msg);
                }
            }

            self.play_track(next).await;
        }

        // Always redraw to keep the visualizer animated
        self.needs_redraw = true;
    }
}
