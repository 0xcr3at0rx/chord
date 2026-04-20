use crate::core::config::Settings;
use crate::core::models::TrackMetadata;
use crate::player::audio::AudioPlayer;
use crate::storage::index::LibraryIndex;
use crate::core::error::{ChordError, ChordResult};
use ratatui::widgets::ListState;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum InputMode {
    Offline,
    Search,
    PlaylistSelect,
    Online,
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

pub struct App<'a> {
    pub all_tracks: Vec<Arc<TrackMetadata>>,
    pub filtered_tracks: Vec<Arc<TrackMetadata>>,
    pub playlists: Vec<Playlist>,
    pub current_playlist: Option<Playlist>,
    pub current_playlist_tracks: Option<Vec<Arc<TrackMetadata>>>,
    pub list_state: ListState,
    pub playlist_list_state: ListState,
    pub input_mode: InputMode,
    pub previous_mode: InputMode,
    pub search_query: String,
    pub radio_stations: Vec<Arc<crate::core::models::RadioStation>>,
    pub filtered_stations: Vec<Arc<crate::core::models::RadioStation>>,
    pub radio_list_state: ListState,
    pub current_track: Option<Arc<TrackMetadata>>,
    pub playback_track_list: Vec<Arc<TrackMetadata>>,
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
    pub settings: &'a Settings,
    pub theme: crate::core::constants::Theme,
    pub index: &'a LibraryIndex,
    pub metadata_rx: mpsc::UnboundedReceiver<TrackMetadataUpdate>,
    pub metadata_tx: mpsc::UnboundedSender<TrackMetadataUpdate>,
    pub refresh_rx: mpsc::UnboundedReceiver<()>,
    pub needs_redraw: bool,
    pub audio_clock: f64,
    pub radio_loaded: bool,
}

impl<'a> App<'a> {
    pub async fn new(settings: &'a Settings, index: &'a LibraryIndex) -> ChordResult<App<'a>> {
        let (metadata_tx, metadata_rx) = mpsc::unbounded_channel();
        let (_refresh_tx, refresh_rx) = mpsc::unbounded_channel();

        let music_dir = settings.config.read().map_err(|e| ChordError::Config(e.to_string()))?.library.music_dir.clone();
        let _ = index.load_cache(&music_dir).await;

        let mut tracks = index.get_all_tracks().await;
        tracks.sort_by(|a, b| {
            a.artist.cmp(&b.artist).then(
                a.album
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.album.as_deref().unwrap_or("")),
            )
        });

        let p_rows = index.get_playlists().await;
        let playlists = p_rows
            .into_iter()
            .map(|(id, name)| Playlist { id, name })
            .collect::<Vec<_>>();

        let mut list_state = ListState::default();
        if !tracks.is_empty() {
            list_state.select(Some(0));
        }

        let mut playlist_list_state = ListState::default();
        if !playlists.is_empty() {
            playlist_list_state.select(Some(0));
        }

        let audio = AudioPlayer::new();
        {
            let config = settings.config.read().map_err(|e| ChordError::Config(e.to_string()))?;
            audio.set_volume(config.audio.volume);
            audio.set_mode(&config.audio.mode);
            audio.try_init();
        }

        let theme = settings.config.read().map_err(|e| ChordError::Config(e.to_string()))?.theme.to_theme();

        let app = App {
            all_tracks: tracks.clone(),
            filtered_tracks: tracks.clone(),
            playlists,
            current_playlist: None,
            current_playlist_tracks: None,
            list_state,
            playlist_list_state,
            input_mode: InputMode::PlaylistSelect,
            previous_mode: InputMode::Offline,
            search_query: String::new(),
            radio_stations: Vec::new(),
            filtered_stations: Vec::new(),
            radio_list_state: ListState::default(),
            current_track: None,
            playback_track_list: Vec::new(),
            playing_idx: None,
            is_playing: false,
            is_starting: false,
            volume: settings.config.read().map_err(|e| ChordError::Config(e.to_string()))?.audio.volume,
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
            settings,
            theme,
            index,
            metadata_rx,
            metadata_tx,
            refresh_rx,
            needs_redraw: true,
            audio_clock: 0.0,
            radio_loaded: false,
        };

        Ok(app)
    }

    pub async fn select_playlist(&mut self, playlist: Option<Playlist>) {
        self.current_playlist = playlist.clone();
        if let Some(p) = playlist {
            let music_dir = match self.settings.config.read() {
                Ok(c) => c.library.music_dir.clone(),
                Err(e) => {
                    self.last_error = Some(format!("Config Error: {}", e));
                    return;
                }
            };
            let mut tracks = self.index.get_playlist_tracks(&p.id, &music_dir).await;
            tracks.sort_by(|a, b| {
                a.artist.cmp(&b.artist).then(
                    a.album
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.album.as_deref().unwrap_or("")),
                )
            });
            self.current_playlist_tracks = Some(tracks.clone());
            self.filtered_tracks = tracks;
        } else {
            self.current_playlist_tracks = None;
            self.filtered_tracks = Vec::new();
        }
        self.filter_tracks();
    }

    pub async fn refresh_library(&mut self) {
        let mut tracks = self.index.get_all_tracks().await;
        tracks.sort_by(|a, b| {
            a.artist.cmp(&b.artist).then(
                a.album
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.album.as_deref().unwrap_or("")),
            )
        });
        self.all_tracks = tracks.clone();

        let p_rows = self.index.get_playlists().await;
        self.playlists = p_rows
            .into_iter()
            .map(|(id, name)| Playlist { id, name })
            .collect();

        if self.current_playlist.is_none() && !self.playlists.is_empty() {
            let p = self.playlists[0].clone();
            self.select_playlist(Some(p)).await;
        } else if let Some(p) = self.current_playlist.clone() {
            self.select_playlist(Some(p)).await;
        }

        self.filter_tracks();
        self.needs_redraw = true;
    }

    pub fn load_radio_stations(&mut self) {
        use std::io::BufRead;
        let mut stations = Vec::new();
        let paths = vec![
            self.settings.config_dir.join("radio.toml"),
            std::env::current_dir().unwrap_or_default().join("radio.toml"),
        ];

        for radio_file in paths {
            if radio_file.exists() {
                if let Ok(file) = std::fs::File::open(&radio_file) {
                    let reader = std::io::BufReader::new(file);
                    
                    let mut current_name = String::new();
                    let mut current_url = String::new();
                    let mut current_country = String::new();
                    let mut current_tags = String::new();

                    let extract_val = |line: &str, prefix: &str| -> Option<String> {
                        if let Some(stripped) = line.strip_prefix(prefix) {
                            if let Some(end) = stripped.strip_suffix("\"") {
                                return Some(end.replace("\\\"", "\"").replace("\\\\", "\\"));
                            }
                        }
                        None
                    };

                    for line_result in reader.lines() {
                        if let Ok(line) = line_result {
                            let line = line.trim();
                            if line == "[[stations]]" {
                                if !current_name.is_empty() && !current_url.is_empty() {
                                    stations.push(Arc::new(crate::core::models::RadioStation {
                                        name: current_name.clone(),
                                        url: current_url.clone(),
                                        country: current_country.clone(),
                                        tags: Some(current_tags.clone()),
                                    }));
                                }
                                current_name.clear();
                                current_url.clear();
                                current_country.clear();
                                current_tags.clear();
                            } else if let Some(val) = extract_val(line, "name = \"") {
                                current_name = val;
                            } else if let Some(val) = extract_val(line, "url = \"") {
                                current_url = val;
                            } else if let Some(val) = extract_val(line, "country = \"") {
                                current_country = val;
                            } else if let Some(val) = extract_val(line, "tags = \"") {
                                current_tags = val;
                            }
                        }
                    }
                    if !current_name.is_empty() && !current_url.is_empty() {
                        stations.push(Arc::new(crate::core::models::RadioStation {
                            name: current_name,
                            url: current_url,
                            country: current_country,
                            tags: Some(current_tags),
                        }));
                    }
                    
                    if !stations.is_empty() {
                        break;
                    }
                }
            }
        }

        if stations.is_empty() {
            for &(name, url, country, tags) in crate::core::radio_stations::DEFAULT_RADIOS {
                stations.push(Arc::new(crate::core::models::RadioStation {
                    name: name.into(),
                    url: url.into(),
                    country: country.into(),
                    tags: Some(tags.into()),
                }));
            }
        }

        self.radio_stations = stations.clone();
        self.filter_radio();
        self.radio_loaded = true;
    }

    pub fn filter_radio(&mut self) {
        let query = self.search_query.to_lowercase();
        let mut filtered = self.radio_stations.clone();

        if !query.is_empty() {
            filtered.retain(|s| {
                s.name.to_lowercase().contains(&query) || 
                s.country.to_lowercase().contains(&query) ||
                s.tags.as_deref().unwrap_or("").to_lowercase().contains(&query)
            });
        }

        self.filtered_stations = filtered;
        if self.filtered_stations.is_empty() {
            self.radio_list_state.select(None);
        } else {
            self.radio_list_state.select(Some(0));
        }
    }

    pub async fn play_radio(&mut self, idx: usize) {
        if let Some(station) = self.filtered_stations.get(idx) {
            self.is_starting = true;
            self.audio.play_stream(station.url.clone());
            self.is_playing = true;
            self.current_track = None;
            self.accumulated_pos = Duration::from_secs(0);
            self.current_pos = Duration::from_secs(0);
            self.playback_start = Some(Instant::now());
            self.lyrics.clear();
            self.last_error = None;

            self.current_track = Some(Arc::new(crate::core::models::TrackMetadata {
                track_id: "radio".into(),
                isrc: None,
                title: station.name.clone(),
                artist: format!("RADIO: {}", station.country),
                album: station.tags.clone(),
                album_art_url: None,
                release_date: None,
                duration_ms: None,
                track_number: None,
                genres: station.tags.clone(),
                file_size: None,
                file_mtime: None,
                file_path: None,
                last_verified_at: None,
                genre: None,
                label: None,
                bit_depth: None,
                sampling_rate: None,
                downloaded_at: None,
                status: Some("radio".into()),
                search_key: String::new(),
            }));
        }
    }

    pub fn next_playlist(&mut self) {
        let len = self.playlists.len() + 1;
        let i = match self.playlist_list_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.playlist_list_state.select(Some(i));
    }

    pub fn previous_playlist(&mut self) {
        let len = self.playlists.len() + 1;
        let i = match self.playlist_list_state.selected() {
            Some(i) => (i + len - 1) % len,
            None => 0,
        };
        self.playlist_list_state.select(Some(i));
    }

    pub async fn save_config(&self) -> ChordResult<()> {
        let (sr, bms, rq) = {
            let mut config = self.settings.config.write().map_err(|e| ChordError::Internal(e.to_string()))?;
            config.audio.device_name = self.audio.device_name.lock().unwrap().clone();
            config.audio.volume = self.volume;
            config.audio.mode = self.audio.mode.lock().unwrap().clone();

            (
                config.audio.sample_rate,
                config.audio.buffer_ms,
                config.audio.resample_quality,
            )
        };

        self.audio.update_audio_config(sr, bms, rq);
        let config_file = self.settings.config_dir.join("config.toml");
        self.settings.save_config(&config_file).map_err(|e| ChordError::Config(e.to_string()))
    }

    pub async fn update(&mut self) {
        if self.is_playing {
            let amp = self.audio.get_amplitude() as f64;
            let speed = 0.005 + (amp * 0.15);
            self.audio_clock += speed;
        }

        while self.refresh_rx.try_recv().is_ok() {
            let _ = self.refresh_library().await;
        }

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

        if !is_empty && self.is_starting {
            self.is_starting = false;
        }

        if self.is_playing && !is_empty {
            if let Some(start) = self.playback_start {
                self.current_pos = self.accumulated_pos + start.elapsed();
            }
            if self.current_track_duration.as_secs_f64() > 0.0 {
                self.progress = (self.current_pos.as_secs_f64() / self.current_track_duration.as_secs_f64()) as f32;
                self.progress = self.progress.clamp(0.0, 1.0);
            }
            if !self.lyrics.is_empty() {
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

            if self.progress >= 0.999 && !self.playback_track_list.is_empty() && !is_init {
                let is_radio = self.current_track.as_ref().map(|t| t.status.as_deref() == Some("radio")).unwrap_or(false);
                if !is_radio {
                    let next = self.playing_idx.map(|i| (i + 1) % self.playback_track_list.len()).unwrap_or(0);
                    let _ = self.play_track(next).await;
                }
            }
        } else if self.is_playing && is_empty && !is_init && !self.is_starting && !self.playback_track_list.is_empty() {
            let is_radio = self.current_track.as_ref().map(|t| t.status.as_deref() == Some("radio")).unwrap_or(false);
            if !is_radio {
                let next = self.playing_idx.map(|i| (i + 1) % self.playback_track_list.len()).unwrap_or(0);
                if has_error {
                    let detail = self.audio.last_error.lock().unwrap().clone();
                    self.last_error = Some(format!("SKIPPING BROKEN TRACK: {}", detail.unwrap_or_default()));
                }
                let _ = self.play_track(next).await;
            } else if has_error {
                // For radio, try to reconnect by re-triggering play_radio if it was a real error
                if let Some(track) = &self.current_track {
                    if let Some(idx) = self.filtered_stations.iter().position(|s| s.name == track.title) {
                        let _ = self.play_radio(idx).await;
                    }
                }
            }
        }
        self.needs_redraw = true;
    }

    pub fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => if i >= self.filtered_tracks.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => if i == 0 { self.filtered_tracks.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub async fn play_track(&mut self, idx: usize) -> ChordResult<()> {
        self.is_playing = false;
        self.is_starting = true;
        let track = self.filtered_tracks.get(idx).ok_or_else(|| ChordError::Playback("Track not found".into()))?.clone();

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

                let tx = self.metadata_tx.clone();
                tokio::task::spawn_blocking(move || {
                    if let Ok(probed) = lofty::read_from_path(&path) {
                        use lofty::file::AudioFile;
                        use lofty::prelude::*;
                        let props = probed.properties();
                        let duration = props.duration();
                        
                        let mut genre = None;
                        let mut label = None;
                        let mut desc = None;
                        let mut art = None;

                        if let Some(tag) = probed.primary_tag() {
                            genre = tag.genre().map(|s| s.to_string());
                            label = tag.get_string(&lofty::tag::ItemKey::Label).map(|s| s.to_string())
                                .or_else(|| tag.get_string(&lofty::tag::ItemKey::Publisher).map(|s| s.to_string()));
                            desc = tag.get_string(&lofty::tag::ItemKey::Comment).map(|s| s.to_string());
                            if let Some(picture) = tag.pictures().first() {
                                art = Some(picture.data().to_vec());
                            }
                        }

                        let _ = tx.send(TrackMetadataUpdate {
                            idx,
                            sample_rate: props.sample_rate().unwrap_or(0),
                            channels: props.channels().unwrap_or(0),
                            bitrate: props.audio_bitrate().unwrap_or(0),
                            bit_depth: props.bit_depth().unwrap_or(0),
                            genre,
                            label,
                            description: desc,
                            cover_art: art,
                            duration,
                        });
                    }
                });
                Ok(())
            } else {
                Err(ChordError::Playback(format!("FILE NOT FOUND: {}", path_str)))
            }
        } else {
            Err(ChordError::Playback("No file path provided".into()))
        }
    }

    pub async fn toggle_playback(&mut self) {
        if self.audio.is_empty() {
            match self.input_mode {
                InputMode::Online => if let Some(idx) = self.radio_list_state.selected() { let _ = self.play_radio(idx).await; }
                _ => if let Some(idx) = self.list_state.selected() { let _ = self.play_track(idx).await; }
            }
        } else {
            if self.is_playing {
                self.audio.pause();
                self.is_playing = false;
                if let Some(start) = self.playback_start { self.accumulated_pos += start.elapsed(); }
                self.playback_start = None;
            } else {
                self.audio.resume();
                self.is_playing = true;
                self.playback_start = Some(Instant::now());
            }
        }
    }

    pub fn filter_tracks(&mut self) {
        let query = self.search_query.to_lowercase();
        let source = self.current_playlist_tracks.as_ref().unwrap_or(&self.all_tracks);
        if query.is_empty() {
            self.filtered_tracks = source.clone();
        } else {
            self.filtered_tracks = source.iter().filter(|t| t.search_key.contains(&query)).cloned().collect();
        }
        if self.filtered_tracks.is_empty() { self.list_state.select(None); } else { self.list_state.select(Some(0)); }
    }

    fn load_lyrics(&mut self, audio_path: &Path) {
        self.lyrics.clear();
        for ext in ["lrc", "txt"] {
            let lrc_path = audio_path.with_extension(ext);
            if lrc_path.exists() {
                if let Ok(content) = std::fs::read_to_string(lrc_path) {
                    self.parse_lrc(&content);
                    if !self.lyrics.is_empty() { break; }
                }
            }
        }
        if self.lyrics.is_empty() {
            self.lyrics.push(crate::player::audio::LyricLine { time: Duration::from_secs(0), text: "NO LYRICS".to_string() });
        }
    }

    fn parse_lrc(&mut self, content: &str) {
        for line in content.lines() {
            if line.starts_with('[') && line.contains(']') {
                let parts: Vec<&str> = line.splitn(2, ']').collect();
                if parts.len() == 2 {
                    let time_str = parts[0].trim_start_matches('[');
                    let text = parts[1].trim();
                    let time_parts: Vec<&str> = time_str.split(':').collect();
                    if time_parts.len() == 2 {
                        let mins = time_parts[0].parse::<u64>().unwrap_or(0);
                        let secs = time_parts[1].parse::<f64>().unwrap_or(0.0);
                        self.lyrics.push(crate::player::audio::LyricLine {
                            time: Duration::from_secs(mins * 60) + Duration::from_secs_f64(secs),
                            text: text.to_string(),
                        });
                    }
                }
            }
        }
        self.lyrics.sort_by_key(|l| l.time);
    }
}
