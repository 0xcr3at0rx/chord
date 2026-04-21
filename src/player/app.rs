use crate::core::config::Settings;
use crate::core::error::{ChordError, ChordResult};
use crate::core::models::TrackMetadata;
use crate::player::audio::AudioPlayer;
use crate::storage::index::LibraryIndex;
use image::{Rgb, RgbImage};
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

pub struct VisualizationState {
    pub velocity: f64,
    pub acceleration: f64,
    pub rotation: f64,
    pub angular_velocity: f64,
    pub angular_acceleration: f64,
    pub position: f64,
    pub last_amplitude: f32,
    pub bass: f64,
    pub mid: f64,
    pub treble: f64,
    pub camera_zoom: f64,
    pub beat_flash: f64,
}

impl Default for VisualizationState {
    fn default() -> Self {
        Self {
            velocity: 0.0,
            acceleration: 0.0,
            rotation: 0.0,
            angular_velocity: 0.0,
            angular_acceleration: 0.0,
            position: 0.0,
            last_amplitude: 0.0,
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            camera_zoom: 1.0,
            beat_flash: 0.0,
        }
    }
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
    pub visual_state: VisualizationState,
}

impl<'a> App<'a> {
    pub fn apply_theme_filter(&self, img: image::DynamicImage) -> image::DynamicImage {
        let mut rgb_img = img.to_rgb8();
        let (tr, tg, tb) = crate::core::constants::color_to_rgb(self.theme.accent);
        
        for pixel in rgb_img.pixels_mut() {
            let image::Rgb([r, g, b]) = *pixel;
            // Simple multiply tint
            let nr = (r as f32 * (tr as f32 / 255.0)) as u8;
            let ng = (g as f32 * (tg as f32 / 255.0)) as u8;
            let nb = (b as f32 * (tb as f32 / 255.0)) as u8;
            *pixel = image::Rgb([nr, ng, nb]);
        }
        image::DynamicImage::ImageRgb8(rgb_img)
    }

    pub fn generate_radio_art(&mut self, name: &str) {
        let mut img = RgbImage::new(256, 256);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::Hasher;
        hasher.write(name.as_bytes());
        let seed = hasher.finish();

        let accent_color = self.theme.accent;
        let critical_color = self.theme.critical;
        let (ar, ag, ab) = crate::core::constants::color_to_rgb(accent_color);
        let (cr, cg, cb) = crate::core::constants::color_to_rgb(critical_color);

        for x in 0..256 {
            for y in 0..256 {
                let dx = x as f32 - 128.0;
                let dy = y as f32 - 128.0;
                let dist = (dx * dx + dy * dy).sqrt();
                let angle = dy.atan2(dx);

                let val = (dist * 0.1 + (angle * (3.0 + (seed % 5) as f32)).sin() * 10.0).sin();
                let t = (0.5 + 0.5 * val) as f64;
                
                // Mix accent and critical based on the pattern
                let r = (ar as f64 * (1.0 - t) + cr as f64 * t) as u8;
                let g = (ag as f64 * (1.0 - t) + cg as f64 * t) as u8;
                let b = (ab as f64 * (1.0 - t) + cb as f64 * t) as u8;

                img.put_pixel(x, y, Rgb([r, g, b]));
            }
        }

        let dynamic_img = image::DynamicImage::ImageRgb8(img);
        if let Some(picker) = &mut self.image_picker {
            self.image_state = Some(picker.new_resize_protocol(dynamic_img.clone()));
        }
        self.cached_image = Some(dynamic_img);
    }

    pub async fn new(settings: &'a Settings, index: &'a LibraryIndex) -> ChordResult<App<'a>> {
        let (metadata_tx, metadata_rx) = mpsc::unbounded_channel();
        let (_refresh_tx, refresh_rx) = mpsc::unbounded_channel();

        let music_dir = settings
            .config
            .read()
            .map_err(|e| ChordError::Config(e.to_string()))?
            .library
            .music_dir
            .clone();
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
            let config = settings
                .config
                .read()
                .map_err(|e| ChordError::Config(e.to_string()))?;
            audio.set_volume(config.audio.volume);
            audio.set_mode(&config.audio.mode);
            audio.try_init();
        }

        let theme = settings
            .config
            .read()
            .map_err(|e| ChordError::Config(e.to_string()))?
            .theme
            .to_theme();

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
            volume: settings
                .config
                .read()
                .map_err(|e| ChordError::Config(e.to_string()))?
                .audio
                .volume,
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
            visual_state: VisualizationState::default(),
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
            std::env::current_dir()
                .unwrap_or_default()
                .join("radio.toml"),
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

                    for line in reader.lines().map_while(Result::ok) {
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
                s.name.to_lowercase().contains(&query)
                    || s.country.to_lowercase().contains(&query)
                    || s.tags
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
            });
        }

        self.filtered_stations = filtered;
        if self.filtered_stations.is_empty() {
            self.radio_list_state.select(None);
        } else {
            self.radio_list_state.select(Some(0));
        }
    }

    pub fn play_radio(&mut self, idx: usize) {
        if let Some(station) = self.filtered_stations.get(idx) {
            let name = station.name.clone();
            let url = station.url.clone();
            let country = station.country.clone();
            let tags = station.tags.clone();

            self.is_starting = true;
            self.audio.play_stream(url);
            self.is_playing = true;
            self.current_track = None;
            self.accumulated_pos = Duration::from_secs(0);
            self.current_pos = Duration::from_secs(0);
            self.playback_start = Some(Instant::now());
            self.lyrics.clear();
            self.last_error = None;
            self.generate_radio_art(&name);

            self.current_track = Some(Arc::new(crate::core::models::TrackMetadata {
                track_id: "radio".into(),
                title: name,
                artist: format!("RADIO: {}", country),
                album: tags.clone(),
                album_art_url: None,
                duration_ms: None,
                genres: tags,
                file_size: None,
                file_mtime: None,
                file_path: None,
                last_verified_at: None,
                genre: None,
                label: None,
                bit_depth: None,
                sampling_rate: None,
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

    pub fn save_config(&self) -> ChordResult<()> {
        let config_to_save = {
            let mut guard = self
                .settings
                .config
                .write()
                .map_err(|e| ChordError::Internal(e.to_string()))?;

            guard.audio.device_name = self.audio.device_name.lock().unwrap().clone();
            guard.audio.volume = self.volume;
            guard.audio.mode = self.audio.mode.lock().unwrap().clone();

            guard.clone()
        };

        let config_dir = self.settings.config_dir.clone();
        std::thread::spawn(move || {
            let config_file = config_dir.join("config.toml");
            if let Ok(toml_str) = toml::to_string_pretty(&config_to_save) {
                let _ = std::fs::write(config_file, toml_str);
            }
        });

        Ok(())
    }


    pub async fn update(&mut self) {
        if self.is_playing {
            let amp = self.audio.get_amplitude();

            // Audio-Kinematics
            let d_amp = (amp - self.visual_state.last_amplitude) as f64;
            self.visual_state.acceleration = d_amp * 2.0;
            self.visual_state.velocity =
                (self.visual_state.velocity * 0.9) + (self.visual_state.acceleration * 0.1);
            self.visual_state.position += self.visual_state.velocity;

            // Angular kinematics based on spectrum/amplitude
            {
                let dsp = self.audio.dsp_state.read().unwrap();
                let spectrum_sum: f32 = dsp.spectrum.iter().sum();
                self.visual_state.angular_acceleration =
                    (spectrum_sum as f64 * 0.001) - (self.visual_state.angular_velocity * 0.05);
                self.visual_state.angular_velocity += self.visual_state.angular_acceleration;
                self.visual_state.rotation += self.visual_state.angular_velocity;

                // Calculate energy bands
                let sample_rate = if self.sample_rate > 0 {
                    self.sample_rate as f32
                } else {
                    48000.0
                };
                let fft_size = (dsp.spectrum.len() * 2) as f32;
                let bin_resolution = sample_rate / fft_size;

                let mut bass_sum = 0.0;
                let mut mid_sum = 0.0;
                let mut treble_sum = 0.0;

                for (i, &mag) in dsp.spectrum.iter().enumerate() {
                    let freq = i as f32 * bin_resolution;
                    if freq < 250.0 {
                        bass_sum += mag;
                    } else if freq < 4000.0 {
                        mid_sum += mag;
                    } else {
                        treble_sum += mag;
                    }
                }

                // Smooth energy
                self.visual_state.bass = self.visual_state.bass * 0.8 + (bass_sum as f64) * 0.2;
                self.visual_state.mid = self.visual_state.mid * 0.8 + (mid_sum as f64) * 0.2;
                self.visual_state.treble =
                    self.visual_state.treble * 0.8 + (treble_sum as f64) * 0.2;

                // Update beat flash
                if dsp.is_beat {
                    self.visual_state.beat_flash = 1.0;
                } else {
                    self.visual_state.beat_flash = (self.visual_state.beat_flash - 0.05).max(0.0);
                }

                // Camera zoom reacts to bass
                let target_zoom = 1.0 + (self.visual_state.bass * 0.05).min(0.5);
                self.visual_state.camera_zoom =
                    self.visual_state.camera_zoom * 0.9 + target_zoom * 0.1;
            }

            self.visual_state.last_amplitude = amp;

            let speed = 0.005 + (amp as f64 * 0.15) + (self.visual_state.treble * 0.01);
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
                        self.cached_image = Some(self.apply_theme_filter(img));
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
                self.progress = (self.current_pos.as_secs_f64()
                    / self.current_track_duration.as_secs_f64())
                    as f32;
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
                let is_radio = self
                    .current_track
                    .as_ref()
                    .map(|t| t.status.as_deref() == Some("radio"))
                    .unwrap_or(false);
                if !is_radio {
                    let next = self
                        .playing_idx
                        .map(|i| (i + 1) % self.playback_track_list.len())
                        .unwrap_or(0);
                    let _ = self.play_track(next);
                }
            }
        } else if self.is_playing
            && is_empty
            && !is_init
            && !self.is_starting
            && !self.playback_track_list.is_empty()
        {
            let is_radio = self
                .current_track
                .as_ref()
                .map(|t| t.status.as_deref() == Some("radio"))
                .unwrap_or(false);
            if !is_radio {
                let next = self
                    .playing_idx
                    .map(|i| (i + 1) % self.playback_track_list.len())
                    .unwrap_or(0);
                if has_error {
                    let detail = self.audio.last_error.lock().unwrap().clone();
                    self.last_error = Some(format!(
                        "SKIPPING BROKEN TRACK: {}",
                        detail.unwrap_or_default()
                    ));
                }
                let _ = self.play_track(next);
            } else if has_error {
                // For radio, try to reconnect by re-triggering play_radio if it was a real error
                if let Some(track) = &self.current_track {
                    if let Some(idx) = self
                        .filtered_stations
                        .iter()
                        .position(|s| s.name == track.title)
                    {
                        let _ = self.play_radio(idx);
                    }
                }
            }
        }
        self.needs_redraw = true;
    }

    pub fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.filtered_tracks.len() - 1 {
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
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.filtered_tracks.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn play_track(&mut self, idx: usize) -> ChordResult<()> {
        self.is_playing = false;
        self.is_starting = true;
        let track = self
            .filtered_tracks
            .get(idx)
            .ok_or_else(|| ChordError::Playback("Track not found".into()))?
            .clone();

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
                            label = tag
                                .get_string(&lofty::tag::ItemKey::Label)
                                .map(|s| s.to_string())
                                .or_else(|| {
                                    tag.get_string(&lofty::tag::ItemKey::Publisher)
                                        .map(|s| s.to_string())
                                });
                            desc = tag
                                .get_string(&lofty::tag::ItemKey::Comment)
                                .map(|s| s.to_string());
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
                Err(ChordError::Playback(format!(
                    "FILE NOT FOUND: {}",
                    path_str
                )))
            }
        } else {
            Err(ChordError::Playback("No file path provided".into()))
        }
    }

    pub async fn toggle_playback(&mut self) {
        if self.audio.is_empty() {
            match self.input_mode {
                InputMode::Online => {
                    if let Some(idx) = self.radio_list_state.selected() {
                        let _ = self.play_radio(idx);
                    }
                }
                _ => {
                    if let Some(idx) = self.list_state.selected() {
                        let _ = self.play_track(idx);
                    }
                }
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

    pub fn filter_tracks(&mut self) {
        let query = self.search_query.to_lowercase();
        let source = self
            .current_playlist_tracks
            .as_ref()
            .unwrap_or(&self.all_tracks);
        if query.is_empty() {
            self.filtered_tracks = source.clone();
        } else {
            self.filtered_tracks = source
                .iter()
                .filter(|t| t.search_key.contains(&query))
                .cloned()
                .collect();
        }
        if self.filtered_tracks.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    fn load_lyrics(&mut self, audio_path: &Path) {
        self.lyrics.clear();
        for ext in ["lrc", "txt"] {
            let lrc_path = audio_path.with_extension(ext);
            if lrc_path.exists() {
                if let Ok(content) = std::fs::read_to_string(lrc_path) {
                    self.parse_lrc(&content);
                    if !self.lyrics.is_empty() {
                        break;
                    }
                }
            }
        }
        if self.lyrics.is_empty() {
            self.lyrics.push(crate::player::audio::LyricLine {
                time: Duration::from_secs(0),
                text: "NO LYRICS".to_string(),
            });
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
