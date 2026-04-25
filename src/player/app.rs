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

use smol_str::SmolStr;

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum InputMode {
    Offline,
    Search,
    PlaylistSelect,
    Online,
}

#[derive(Clone, Debug)]
pub struct Playlist {
    pub id: SmolStr,
    pub name: SmolStr,
}

#[derive(Clone, Debug)]
pub struct TrackMetadataUpdate {
    pub idx: usize,
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
    pub bit_depth: u8,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub cover_art: Option<Vec<u8>>,
    pub duration: Duration,
}

#[derive(Clone, Debug)]
pub struct RefreshUpdate {
    pub all_tracks: Box<[Arc<TrackMetadata>]>,
    pub playlists: Box<[Playlist]>,
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

pub struct App {
    pub all_tracks: Arc<[Arc<TrackMetadata>]>,
    pub filtered_tracks: Arc<[Arc<TrackMetadata>]>,
    pub playlists: Box<[Playlist]>,
    pub current_playlist: Option<Playlist>,
    pub current_playlist_tracks: Option<Arc<[Arc<TrackMetadata>]>>,
    pub list_state: ListState,
    pub playlist_list_state: ListState,
    pub input_mode: InputMode,
    pub previous_mode: InputMode,
    pub search_query: String,
    pub last_search_query: String,
    pub search_cache: Arc<[Arc<TrackMetadata>]>,
    pub radio_stations: Box<[Arc<crate::core::models::RadioStation>]>,
    pub filtered_stations: Box<[Arc<crate::core::models::RadioStation>]>,
    pub radio_list_state: ListState,
    pub current_track: Option<Arc<TrackMetadata>>,
    pub playback_track_list: Arc<[Arc<TrackMetadata>]>,
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
    pub cached_image: Option<Arc<image::DynamicImage>>,
    pub image_cache: std::collections::HashMap<String, Arc<image::DynamicImage>>,
    pub image_state: Option<ratatui_image::protocol::StatefulProtocol>,
    pub image_picker: Option<ratatui_image::picker::Picker>,
    pub last_key_event: Option<(crossterm::event::KeyCode, std::time::Instant)>,
    pub last_error: Option<String>,
    pub audio: AudioPlayer,
    pub settings: Arc<Settings>,
    pub theme: crate::core::constants::Theme,
    pub index: Arc<LibraryIndex>,
    pub metadata_rx: mpsc::UnboundedReceiver<TrackMetadataUpdate>,
    pub metadata_tx: mpsc::UnboundedSender<TrackMetadataUpdate>,
    pub refresh_rx: mpsc::UnboundedReceiver<RefreshUpdate>,
    pub refresh_tx: mpsc::UnboundedSender<RefreshUpdate>,
    pub needs_redraw: bool,
    pub audio_clock: f64,
    pub radio_loaded: bool,
    pub visual_state: VisualizationState,
}

impl App {
    #[tracing::instrument(skip(self))]
    pub fn generate_radio_art(&mut self, name: &str) {
        let accent_color = self.theme.accent;
        let cache_key = format!("radio_{}_{:?}", name, accent_color);

        if let Some(img) = self.image_cache.get(&cache_key) {
            tracing::debug!(name, "Using cached radio art");
            if let Some(picker) = &mut self.image_picker {
                self.image_state = Some(picker.new_resize_protocol((**img).clone()));
            }
            self.cached_image = Some(Arc::clone(img));
            return;
        }

        tracing::info!(name, "Generating new radio art");
        let mut img = RgbImage::new(256, 256);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::Hasher;
        hasher.write(name.as_bytes());
        let seed = hasher.finish();

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

        let dynamic_img = Arc::new(image::DynamicImage::ImageRgb8(img));
        self.image_cache.insert(cache_key, Arc::clone(&dynamic_img));
        if let Some(picker) = &mut self.image_picker {
            self.image_state = Some(picker.new_resize_protocol((*dynamic_img).clone()));
        }
        self.cached_image = Some(dynamic_img);
    }

    #[tracing::instrument(skip(settings, index))]
    pub async fn new(
        settings: Arc<Settings>,
        index: Arc<LibraryIndex>,
    ) -> ChordResult<App> {
        tracing::info!("Creating new App instance");
        let (metadata_tx, metadata_rx) = mpsc::unbounded_channel();
        let (refresh_tx, refresh_rx) = mpsc::unbounded_channel();

        let config = settings
            .config
            .read()
            .map_err(|e| ChordError::Config(e.to_string()))?;

        let music_dir = config.library.music_dir.clone();
        let volume = config.audio.volume;
        let mode = config.audio.mode.clone();
        let theme = config.theme.to_theme();
        
        // Drop lock before async calls
        drop(config);

        let _ = index.load_cache(&music_dir, false).await;

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
        let playlists: Box<[Playlist]> = p_rows
            .iter()
            .map(|(id, name)| Playlist { id: id.clone(), name: name.clone() })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let mut list_state = ListState::default();
        if !tracks.is_empty() {
            list_state.select(Some(0));
        }

        let mut playlist_list_state = ListState::default();
        if !playlists.is_empty() {
            playlist_list_state.select(Some(0));
        }

        let audio = AudioPlayer::new();
        audio.set_volume(volume);
        audio.set_mode(&mode);
        audio.try_init();

        let tracks_arc: Arc<[Arc<TrackMetadata>]> = Arc::from(tracks);
        let app = App {
            all_tracks: Arc::clone(&tracks_arc),
            filtered_tracks: Arc::clone(&tracks_arc),
            playlists,
            current_playlist: None,
            current_playlist_tracks: None,
            list_state,
            playlist_list_state,
            input_mode: InputMode::PlaylistSelect,
            previous_mode: InputMode::Offline,
            search_query: String::new(),
            last_search_query: String::new(),
            search_cache: Arc::clone(&tracks_arc),
            radio_stations: Vec::new().into_boxed_slice(),
            filtered_stations: Vec::new().into_boxed_slice(),
            radio_list_state: ListState::default(),
            current_track: None,
            playback_track_list: Arc::from(Vec::new()),
            playing_idx: None,
            is_playing: false,
            is_starting: false,
            volume,
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
            image_cache: std::collections::HashMap::new(),
            image_state: None,
            image_picker: ratatui_image::picker::Picker::from_query_stdio().ok(),
            last_key_event: None,
            last_error: None,
            audio,
            settings: Arc::clone(&settings),
            theme,
            index,
            metadata_rx,
            metadata_tx,
            refresh_rx,
            refresh_tx,
            needs_redraw: false,
            audio_clock: 0.0,
            radio_loaded: false,
            visual_state: VisualizationState::default(),
        };
        Ok(app)
    }

    #[tracing::instrument(skip(self))]
    pub async fn select_playlist(&mut self, playlist: Option<Playlist>) {
        tracing::info!(playlist = ?playlist, "Selecting playlist");
        self.current_playlist = playlist.clone();
        if let Some(p) = playlist {
            let music_dir = match self.settings.config.read() {
                Ok(c) => c.library.music_dir.clone(),
                Err(e) => {
                    tracing::error!(error = %e, "Config Error during playlist selection");
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
            let arc_tracks: Arc<[Arc<TrackMetadata>]> = Arc::from(tracks);
            self.current_playlist_tracks = Some(Arc::clone(&arc_tracks));
            self.filtered_tracks = arc_tracks;
        } else {
            self.current_playlist_tracks = None;
            self.filtered_tracks = Arc::clone(&self.all_tracks);
        }
        
        // Reset scroll position to top UNLESS a track from this playlist is playing
        let mut new_idx = 0;
        if let Some(playing) = &self.current_track {
            if let Some(pos) = self.filtered_tracks.iter().position(|t| t.track_id == playing.track_id) {
                new_idx = pos;
            }
        }
        self.list_state.select(Some(new_idx));
        
        self.filter_tracks();
    }

    #[tracing::instrument(skip(self))]
    pub async fn refresh_library(&mut self) {
        tracing::info!("Refreshing library (background)");
        let music_dir = match self.settings.config.read() {
            Ok(c) => c.library.music_dir.clone(),
            Err(e) => {
                tracing::error!(error = %e, "Config Error during library refresh");
                self.last_error = Some(format!("Config Error: {}", e));
                return;
            }
        };
        
        let index_clone = Arc::clone(&self.index);
        let tx = self.refresh_tx.clone();
        
        tokio::task::spawn(async move {
            let _ = index_clone.load_cache(&music_dir, true).await;
            let mut tracks = index_clone.get_all_tracks().await;
            tracks.sort_by(|a, b| {
                a.artist.cmp(&b.artist).then(
                    a.album
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.album.as_deref().unwrap_or("")),
                )
            });
            
            let p_rows = index_clone.get_playlists().await;
            let playlists = p_rows
                .iter()
                .map(|(id, name)| Playlist { id: id.clone(), name: name.clone() })
                .collect::<Vec<_>>()
                .into_boxed_slice();
                
            let _ = tx.send(RefreshUpdate {
                all_tracks: tracks,
                playlists,
            });
        });
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
                                    name: std::mem::take(&mut current_name).into(),
                                    url: std::mem::take(&mut current_url),
                                    country: std::mem::take(&mut current_country).into(),
                                    tags: Some(std::mem::take(&mut current_tags).into()),
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
                            name: current_name.into(),
                            url: current_url,
                            country: current_country.into(),
                            tags: Some(current_tags.into()),
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
        self.radio_stations = stations.into_boxed_slice();
        self.filter_radio();

        self.radio_loaded = true;
    }

    pub fn filter_radio(&mut self) {
        let query = self.search_query.to_lowercase();
        
        // 1. Save currently selected station name
        let selected_name = self.radio_list_state.selected().and_then(|i| {
            self.filtered_stations.get(i).map(|s| s.name.clone())
        });

        let mut filtered = self.radio_stations.to_vec();

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

        self.filtered_stations = filtered.into_boxed_slice();
        if self.filtered_stations.is_empty() {
            self.radio_list_state.select(None);
        } else {
            // 2. Try to restore selection by Name
            let mut new_idx = None;
            
            if let Some(name) = selected_name {
                new_idx = self.filtered_stations.iter().position(|s| s.name == name);
            }
            
            // 3. If not found, try to select the playing station
            if new_idx.is_none() {
                if let Some(playing) = &self.current_track {
                    if playing.status.as_deref() == Some("radio") {
                        new_idx = self.filtered_stations.iter().position(|s| s.name == playing.title);
                    }
                }
            }

            self.radio_list_state.select(Some(new_idx.unwrap_or(0)));
        }
    }

    #[tracing::instrument(skip(self))]
    pub fn play_radio(&mut self, idx: usize) {
        if let Some(station) = self.filtered_stations.get(idx) {
            tracing::info!(name = %station.name, url = %station.url, "Playing radio station");
            let name = station.name.clone();
            let url = station.url.clone();
            let country = station.country.clone();
            let tags = station.tags.clone();

            self.is_starting = true;
            self.audio.play_stream(url);
            self.is_playing = true;
            self.current_track = None; // Reset before setting new one for radio
            
            self.current_track = Some(Arc::new(crate::core::models::TrackMetadata {
                track_id: "radio".into(),
                title: name.clone(), // Use cloned name
                artist: format!("RADIO: {}", country).into(),
                album: tags.clone(),
                album_art_url: None,
                duration_ms: None,
                genres: tags.map(|s| s.into()),
                file_size: None,
                file_mtime: None,
                file_path: None,
                last_verified_at: None,
                genre: None,
                label: None,
                bit_depth: None,
                sampling_rate: None,
                status: Some("radio".into()),
            }));

            self.clean_image_cache();
            
            self.accumulated_pos = Duration::from_secs(0);
            self.current_pos = Duration::from_secs(0);
            self.playback_start = Some(Instant::now());
            self.lyrics.clear();
            self.last_error = None;
            self.generate_radio_art(&name); // Use cloned name
        }
    }

    pub fn next_playlist(&mut self) {
        let len = self.playlists.len();
        if len == 0 { return; }
        let i = match self.playlist_list_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.playlist_list_state.select(Some(i));
    }

    pub fn previous_playlist(&mut self) {
        let len = self.playlists.len();
        if len == 0 { return; }
        let i = match self.playlist_list_state.selected() {
            Some(i) => (i + len - 1) % len,
            None => 0,
        };
        self.playlist_list_state.select(Some(i));
    }

    pub fn save_config(&self) -> ChordResult<()> {
        {
            let mut guard = self
                .settings
                .config
                .write()
                .map_err(|e| ChordError::Internal(e.to_string()))?;

            guard.audio.volume = self.volume;
            guard.audio.mode = self.audio.mode.lock().unwrap().clone();
        }

        let settings = Arc::clone(&self.settings);
        std::thread::spawn(move || {
            let config_file = config_file_path(&settings.config_dir);
            let _ = settings.save_config(&config_file);
        });

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn update(&mut self) {
        if self.is_playing && (self.input_mode == InputMode::Offline || self.input_mode == InputMode::Online) {
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

                let bass_cutoff = (250.0 / bin_resolution) as usize;
                let mid_cutoff = (4000.0 / bin_resolution) as usize;
                
                let bass_end = bass_cutoff.min(dsp.spectrum.len());
                let mid_end = mid_cutoff.min(dsp.spectrum.len());

                let bass_sum: f32 = dsp.spectrum[..bass_end].iter().sum();
                let mid_sum: f32 = dsp.spectrum[bass_end..mid_end].iter().sum();
                let treble_sum: f32 = dsp.spectrum[mid_end..].iter().sum();

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

        while let Ok(update) = self.refresh_rx.try_recv() {
            tracing::info!("Applying background library refresh update");
            self.all_tracks = Arc::from(update.all_tracks);
            self.playlists = update.playlists;
            
            if let Some(p) = self.current_playlist.as_ref() {
                let p = p.clone();
                self.select_playlist(Some(p)).await;
            } else if !self.playlists.is_empty() {
                let p = self.playlists[0].clone();
                self.select_playlist(Some(p)).await;
            }

            // Update current_track with new metadata if it exists
            if let Some(current) = &self.current_track {
                if let Some(updated) = self.all_tracks.iter().find(|t| t.track_id == current.track_id) {
                    self.current_track = Some(Arc::clone(updated));
                }
            }

            self.filter_tracks();
            self.needs_redraw = true;
        }

        while let Ok(update) = self.metadata_rx.try_recv() {
            tracing::debug!(idx = update.idx, "Received metadata update for track");
            if self.playing_idx == Some(update.idx) {
                self.sample_rate = update.sample_rate;
                self.channels = update.channels;
                self.bitrate = update.bitrate;
                self.bit_depth = update.bit_depth;
                self.current_genre = update.genre.clone();
                self.current_label = update.label.clone();
                self.current_description = update.description.clone();
                self.current_cover_art = update.cover_art;
                self.current_track_duration = update.duration;

                // Update the Arc<TrackMetadata> in the list if it was unverified
                if let Some(track) = self.playback_track_list.get(update.idx) {
                    if track.status.as_deref() == Some("unverified") {
                        let mut new_track = (**track).clone();
                        if let Some(t) = update.title { new_track.title = t.into(); }
                        if let Some(a) = update.artist { new_track.artist = a.into(); }
                        if let Some(al) = update.album { new_track.album = Some(al.into()); }
                        if let Some(g) = update.genre { new_track.genre = Some(g.into()); }
                        new_track.status = Some("verified".into());
                        
                        let arc_track = Arc::new(new_track);
                        self.current_track = Some(Arc::clone(&arc_track));
                        
                        // We would ideally update the whole library here, 
                        // but for now we update the active playback list to reflect names immediately
                        // The next full scan will pick up the updated cache if we were to save it.
                    }
                }

                if let Some(track) = &self.current_track {
                    let cache_key = format!("{}_{:?}", track.track_id, self.theme.accent);

                    if let Some(img) = self.image_cache.get(&cache_key) {
                        self.cached_image = Some(Arc::clone(img));
                        self.image_state = None;
                    } else if let Some(art) = &self.current_cover_art {
                        if let Ok(img) = image::load_from_memory(art) {
                            // Apply theme filter directly here and cache the themed version
                            let mut rgb_img = img.to_rgb8();
                            let (tr, tg, tb) = crate::core::constants::color_to_rgb(self.theme.accent);

                            for pixel in rgb_img.pixels_mut() {
                                let image::Rgb([r, g, b]) = *pixel;
                                let nr = (r as f32 * (tr as f32 / 255.0)) as u8;
                                let ng = (g as f32 * (tg as f32 / 255.0)) as u8;
                                let nb = (b as f32 * (tb as f32 / 255.0)) as u8;
                                *pixel = image::Rgb([nr, ng, nb]);
                            }

                            let themed = Arc::new(image::DynamicImage::ImageRgb8(rgb_img));
                            self.image_cache.insert(cache_key.clone(), Arc::clone(&themed));
                            self.cached_image = Some(themed);
                            self.image_state = None;
                        }
                        // Clear raw bytes after decoding to save RAM
                        self.current_cover_art = None;
                    }
                }

                self.clean_image_cache();

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
                    if let Some(track) = self.playback_track_list.get(next) {
                        let track = track.clone();
                        let _ = self.play_track_at(track, next, false);
                    }
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
                if let Some(track) = self.playback_track_list.get(next) {
                    let track = track.clone();
                    if has_error {
                        let detail = self.audio.last_error.lock().unwrap().clone();
                        self.last_error = Some(format!(
                            "SKIPPING BROKEN TRACK: {}",
                            detail.unwrap_or_default()
                        ));
                    }
                    let _ = self.play_track_at(track, next, false);
                }
            } else if has_error {
                // For radio, try to reconnect by re-triggering play_radio if it was a real error
                if let Some(track) = &self.current_track {
                    if let Some(idx) = self
                        .filtered_stations
                        .iter()
                        .position(|s| s.name == track.title)
                    {
                        self.play_radio(idx);
                    }
                }
            }
        }

        self.needs_redraw = true;
    }

    pub fn next(&mut self) {
        let len = self.filtered_tracks.len();
        if len == 0 {
            self.list_state.select(None);
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
            self.list_state.select(None);
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

    #[tracing::instrument(skip(self))]
    pub fn play_track(&mut self, idx: usize) -> ChordResult<()> {
        let track = self
            .filtered_tracks
            .get(idx)
            .ok_or_else(|| {
                tracing::error!(idx, "Track index out of bounds");
                ChordError::Playback("Track not found".into())
            })?
            .clone();
        tracing::info!(title = %track.title, "Play track requested");
        self.play_track_at(track, idx, true)
    }

    pub fn play_next(&mut self) {
        if self.playback_track_list.is_empty() { return; }
        let next = self.playing_idx
            .map(|i| (i + 1) % self.playback_track_list.len())
            .unwrap_or(0);
        if let Some(track) = self.playback_track_list.get(next) {
            let track = track.clone();
            let _ = self.play_track_at(track, next, false);
            
            // Sync cursor if possible
            if let Some(pos) = self.filtered_tracks.iter().position(|t| t.track_id == self.playback_track_list[next].track_id) {
                self.list_state.select(Some(pos));
            }
        }
    }

    pub fn play_previous(&mut self) {
        if self.playback_track_list.is_empty() { return; }
        let len = self.playback_track_list.len();
        let prev = self.playing_idx
            .map(|i| (i + len - 1) % len)
            .unwrap_or(0);
        if let Some(track) = self.playback_track_list.get(prev) {
            let track = track.clone();
            let _ = self.play_track_at(track, prev, false);

            // Sync cursor if possible
            if let Some(pos) = self.filtered_tracks.iter().position(|t| t.track_id == self.playback_track_list[prev].track_id) {
                self.list_state.select(Some(pos));
            }
        }
    }

    #[tracing::instrument(skip(self, track))]
    fn play_track_at(
        &mut self,
        track: Arc<TrackMetadata>,
        idx: usize,
        update_playback_list: bool,
    ) -> ChordResult<()> {
        tracing::info!(title = %track.title, path = ?track.file_path, "Starting playback");
        self.is_playing = false;
        self.is_starting = true;

        // Reset tech info immediately
        self.current_genre = None;
        self.current_label = None;
        self.current_description = None;
        self.current_cover_art = None;
        self.sample_rate = 0;
        self.channels = 0;
        self.bitrate = 0;
        self.bit_depth = 0;

        // Proactively clean image cache to free RAM during transitions
        self.playing_idx = Some(idx);
        self.current_track = Some(track.clone()); // Update current track BEFORE cleaning
        if update_playback_list {
            self.playback_track_list = Arc::clone(&self.filtered_tracks);
        }
        self.clean_image_cache();

        if let Some(path_str) = &track.file_path {
            let path = PathBuf::from(path_str);
            if path.exists() {
                self.load_lyrics(&path);
                self.audio.play(path.clone());
                self.is_playing = true;
                // self.current_track = Some(track.clone()); // Already updated above
                self.accumulated_pos = Duration::from_secs(0);
                self.current_pos = Duration::from_secs(0);
                self.playback_start = Some(Instant::now());
                self.last_error = None;

                // Optimization: If track is already verified, use cached metadata
                if track.status.as_deref() == Some("verified") {
                    self.sample_rate = track.sampling_rate.unwrap_or(0);
                    self.bit_depth = track.bit_depth.unwrap_or(0);
                    // Channels and bitrate are not in basic metadata, still need probing if we want them,
                    // but we can skip if we prioritize speed/memory.
                    // For now, let's still probe but only if necessary.
                }

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
                        let mut title = None;
                        let mut artist = None;
                        let mut album = None;

                        if let Some(tag) = probed.primary_tag() {
                            title = tag.title().map(|s| s.to_string());
                            artist = tag.artist().map(|s| s.to_string());
                            album = tag.album().map(|s| s.to_string());
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
                            
                            // Only extract art if we don't already have a themed version in some cache?
                            // Actually, metadata extraction is per-play now, so we always check.
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
                            title,
                            artist,
                            album,
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

    fn clean_image_cache(&mut self) {
        let mut keys_to_keep = std::collections::HashSet::new();

        if let Some(track) = &self.current_track {
            keys_to_keep.insert(format!("{}_{:?}", track.track_id, self.theme.accent));

            if !self.playback_track_list.is_empty() {
                let len = self.playback_track_list.len();
                let idx = self.playing_idx.unwrap_or(0);
                let next_idx = (idx + 1) % len;
                let prev_idx = (idx + len - 1) % len;

                if let Some(next_t) = self.playback_track_list.get(next_idx) {
                    keys_to_keep.insert(format!("{}_{:?}", next_t.track_id, self.theme.accent));
                }
                if let Some(prev_t) = self.playback_track_list.get(prev_idx) {
                    keys_to_keep.insert(format!("{}_{:?}", prev_t.track_id, self.theme.accent));
                }
            }
        }

        let current_radio_key = self.current_track.as_ref()
            .and_then(|t| if t.status.as_deref() == Some("radio") {
                Some(format!("radio_{}_{:?}", t.title, self.theme.accent))
            } else { None });

        self.image_cache.retain(|k, _| {
            keys_to_keep.contains(k) || (current_radio_key.as_ref() == Some(k))
        });
        
        // If current image is not in cache anymore, clear it
        if let Some(track) = &self.current_track {
            let cache_key = format!("{}_{:?}", track.track_id, self.theme.accent);
            if !self.image_cache.contains_key(&cache_key) && current_radio_key.as_ref() != Some(&cache_key) {
                self.cached_image = None;
                self.image_state = None;
            }
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn toggle_playback(&mut self) {
        if self.audio.is_empty() {
            tracing::debug!("Toggle playback: audio empty, starting selected");
            match self.input_mode {
                InputMode::Online => {
                    if let Some(idx) = self.radio_list_state.selected() {
                        self.play_radio(idx);
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
                tracing::info!("Pausing playback");
                self.audio.pause();
                self.is_playing = false;
                if let Some(start) = self.playback_start {
                    self.accumulated_pos += start.elapsed();
                }
                self.playback_start = None;
            } else {
                tracing::info!("Resuming playback");
                self.audio.resume();
                self.is_playing = true;
                self.playback_start = Some(Instant::now());
            }
        }
    }

    pub fn filter_tracks(&mut self) {
        let query = self.search_query.to_lowercase();

        // 1. Save currently selected track ID if any
        let selected_id = self.list_state.selected().and_then(|i| {
            self.filtered_tracks.get(i).map(|t| t.track_id.clone())
        });

        let source = self
            .current_playlist_tracks
            .as_ref()
            .unwrap_or(&self.all_tracks);

        if query.is_empty() {
            self.filtered_tracks = Arc::clone(source);
            self.search_cache = Arc::clone(source);
        } else {
            // Smart Caching: If the new query starts with the old query, we can filter the already filtered subset!
            let filter_source = if query.starts_with(&self.last_search_query) && !self.last_search_query.is_empty() {
                &self.search_cache
            } else {
                source
            };

            let filtered: Box<[Arc<TrackMetadata>]> = filter_source
                .iter()
                .filter(|t| {
                    t.title.to_lowercase().contains(&query)
                        || t.artist.to_lowercase().contains(&query)
                        || t.album.as_ref().map(|a| a.to_lowercase().contains(&query)).unwrap_or(false)
                })
                .cloned()
                .collect::<Vec<_>>()
                .into_boxed_slice();

            let arc_filtered = Arc::from(filtered);
            self.filtered_tracks = Arc::clone(&arc_filtered);
            self.search_cache = arc_filtered;
        }

        self.last_search_query = query;

        if self.filtered_tracks.is_empty() {            self.list_state.select(None);
        } else {
            // 2. Try to restore selection by ID
            let mut new_idx = None;
            
            if let Some(id) = selected_id {
                new_idx = self.filtered_tracks.iter().position(|t| t.track_id == id);
            }
            
            // 3. If not found (e.g. filtered out), try to select the playing track
            if new_idx.is_none() {
                if let Some(playing) = &self.current_track {
                    new_idx = self.filtered_tracks.iter().position(|t| t.track_id == playing.track_id);
                }
            }

            // 4. Fallback to first item
            self.list_state.select(Some(new_idx.unwrap_or(0)));
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

    pub fn volume_up(&mut self) {
        self.volume = (self.volume + 0.05).min(1.0);
        self.audio.set_volume(self.volume);
        let _ = self.save_config();
    }

    pub fn volume_down(&mut self) {
        self.volume = (self.volume - 0.05).max(0.0);
        self.audio.set_volume(self.volume);
        let _ = self.save_config();
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
                        if let (Ok(mins), Ok(secs)) = (time_parts[0].parse::<u64>(), time_parts[1].parse::<f64>()) {
                            self.lyrics.push(crate::player::audio::LyricLine {
                                time: Duration::from_secs(mins * 60) + Duration::from_secs_f64(secs),
                                text: text.to_string(),
                            });
                        }
                    }
                }
            }
        }
        self.lyrics.sort_by_key(|l| l.time);
    }
}

fn config_file_path(config_dir: &Path) -> PathBuf {
    config_dir.join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Settings;
    use crate::storage::index::LibraryIndex;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn create_test_app() -> App {
        let (metadata_tx, metadata_rx) = mpsc::unbounded_channel();
        let (refresh_tx, refresh_rx) = mpsc::unbounded_channel();
        let settings = Arc::new(Settings::new().unwrap());
        let index = Arc::new(LibraryIndex::new(&PathBuf::from("/tmp")));
        
        App {
            all_tracks: Arc::from(vec![]),
            filtered_tracks: Arc::from(vec![]),
            playlists: vec![].into_boxed_slice(),
            current_playlist: None,
            current_playlist_tracks: None,
            list_state: ListState::default(),
            playlist_list_state: ListState::default(),
            input_mode: InputMode::Offline,
            previous_mode: InputMode::Offline,
            search_query: String::new(),
            last_search_query: String::new(),
            search_cache: Arc::from(vec![]),
            radio_stations: vec![].into_boxed_slice(),
            filtered_stations: vec![].into_boxed_slice(),
            radio_list_state: ListState::default(),
            current_track: None,
            playback_track_list: Arc::from(vec![]),
            playing_idx: None,
            is_playing: false,
            is_starting: false,
            volume: 1.0,
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
            image_cache: std::collections::HashMap::new(),
            image_state: None,
            image_picker: None,
            last_key_event: None,
            last_error: None,
            audio: AudioPlayer::new_null(),
            settings,
            theme: crate::core::config::ThemeConfig::default().to_theme(),
            index,
            metadata_rx,
            metadata_tx,
            refresh_rx,
            refresh_tx,
            needs_redraw: false,
            audio_clock: 0.0,
            radio_loaded: false,
            visual_state: VisualizationState::default(),
        }
    }

    #[test]
    fn test_visualization_state_default() {
        let state = VisualizationState::default();
        assert_eq!(state.velocity, 0.0);
        assert_eq!(state.camera_zoom, 1.0);
        assert_eq!(state.position, 0.0);
    }

    #[test]
    fn test_app_filter_tracks() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata {
            track_id: "1".into(),
            title: "Song A".into(),
            artist: "Artist X".into(),
            album: Some("Album 1".into()),
            file_path: None,
            album_art_url: None,
            duration_ms: None,
            genres: None,
            file_size: None,
            file_mtime: None,
            last_verified_at: None,
            genre: None,
            label: None,
            bit_depth: None,
            sampling_rate: None,
            status: None,
        });
        let t2 = Arc::new(TrackMetadata {
            track_id: "2".into(),
            title: "Song B".into(),
            artist: "Artist Y".into(),
            album: Some("Album 2".into()),
            ..t1.as_ref().clone()
        });
        app.all_tracks = Arc::from(vec![t1, t2]);
        app.filtered_tracks = Arc::clone(&app.all_tracks);

        // Filter by title
        app.search_query = "Song A".to_string();
        app.filter_tracks();
        assert_eq!(app.filtered_tracks.len(), 1);
        assert_eq!(app.filtered_tracks[0].title, "Song A");

        // Filter by artist
        app.search_query = "Artist Y".to_string();
        app.filter_tracks();
        assert_eq!(app.filtered_tracks.len(), 1);
        assert_eq!(app.filtered_tracks[0].artist, "Artist Y");

        // Filter by album
        app.search_query = "Album 1".to_string();
        app.filter_tracks();
        assert_eq!(app.filtered_tracks.len(), 1);
        assert_eq!(app.filtered_tracks[0].album.as_deref(), Some("Album 1"));

        // Clear filter
        app.search_query = "".to_string();
        app.filter_tracks();
        assert_eq!(app.filtered_tracks.len(), 2);
    }

    #[test]
    fn test_app_filter_tracks_restore_selection() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), title: "A".into(), ..TrackMetadata::default() });
        let t2 = Arc::new(TrackMetadata { track_id: "2".into(), title: "B".into(), ..TrackMetadata::default() });
        app.all_tracks = Arc::from(vec![t1, t2]);
        app.filtered_tracks = Arc::clone(&app.all_tracks);
        app.list_state.select(Some(1));

        app.search_query = "B".to_string();
        app.filter_tracks();
        assert_eq!(app.list_state.selected(), Some(0));
        assert_eq!(app.filtered_tracks[0].track_id, "2");
    }

    #[test]
    fn test_app_filter_radio() {
        let mut app = create_test_app();
        let s1 = Arc::new(crate::core::models::RadioStation {
            name: "Radio A".into(),
            url: "url1".into(),
            country: "Country X".into(),
            tags: Some("Tag 1".into()),
        });
        let s2 = Arc::new(crate::core::models::RadioStation {
            name: "Radio B".into(),
            url: "url2".into(),
            country: "Country Y".into(),
            tags: Some("Tag 2".into()),
        });
        app.radio_stations = vec![s1, s2].into_boxed_slice();
        app.filter_radio();

        app.search_query = "Radio A".to_string();
        app.filter_radio();
        assert_eq!(app.filtered_stations.len(), 1);
        assert_eq!(app.filtered_stations[0].name, "Radio A");

        app.search_query = "Country Y".to_string();
        app.filter_radio();
        assert_eq!(app.filtered_stations.len(), 1);
        assert_eq!(app.filtered_stations[0].country, "Country Y");

        app.search_query = "Tag 1".to_string();
        app.filter_radio();
        assert_eq!(app.filtered_stations.len(), 1);
        assert_eq!(app.filtered_stations[0].tags.as_deref(), Some("Tag 1"));
    }

    #[test]
    fn test_app_filter_radio_restore_selection() {
        let mut app = create_test_app();
        let s1 = Arc::new(crate::core::models::RadioStation { name: "A".into(), url: "u".into(), country: "c".into(), tags: None });
        let s2 = Arc::new(crate::core::models::RadioStation { name: "B".into(), url: "u".into(), country: "c".into(), tags: None });
        app.radio_stations = vec![s1, s2].into_boxed_slice();
        app.filtered_stations = app.radio_stations.clone();
        app.radio_list_state.select(Some(1));

        app.search_query = "B".to_string();
        app.filter_radio();
        assert_eq!(app.radio_list_state.selected(), Some(0));
        assert_eq!(app.filtered_stations[0].name, "B");
    }

    #[test]
    fn test_app_parse_lrc() {
        let mut app = create_test_app();
        let content = "[00:01.00]Line 1\n[00:02.50]Line 2\nInvalid Line\n[01:00]Line 3";
        app.parse_lrc(content);
        
        assert_eq!(app.lyrics.len(), 3);
        assert_eq!(app.lyrics[0].time, Duration::from_secs(1));
        assert_eq!(app.lyrics[0].text, "Line 1");
        assert_eq!(app.lyrics[1].time, Duration::from_millis(2500));
        assert_eq!(app.lyrics[1].text, "Line 2");
        assert_eq!(app.lyrics[2].time, Duration::from_secs(60));
        assert_eq!(app.lyrics[2].text, "Line 3");
    }

    #[test]
    fn test_app_parse_lrc_complex() {
        let mut app = create_test_app();
        let content = "[00:10.00] Second\n[00:05.00] First\n[invalid]";
        app.parse_lrc(content);
        assert_eq!(app.lyrics.len(), 2);
        assert_eq!(app.lyrics[0].text, "First");
        assert_eq!(app.lyrics[1].text, "Second");
    }

    #[test]
    fn test_app_circular_navigation() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), ..TrackMetadata::default() });
        let t2 = Arc::new(TrackMetadata { track_id: "2".into(), ..TrackMetadata::default() });
        app.filtered_tracks = Arc::from(vec![t1, t2]);
        app.list_state.select(Some(0));

        app.next();
        assert_eq!(app.list_state.selected(), Some(1));
        app.next();
        assert_eq!(app.list_state.selected(), Some(0));

        app.previous();
        assert_eq!(app.list_state.selected(), Some(1));
        app.previous();
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn test_app_circular_playlist_navigation() {
        let mut app = create_test_app();
        app.playlists = vec![
            Playlist { id: "1".into(), name: "P1".into() },
            Playlist { id: "2".into(), name: "P2".into() },
        ].into_boxed_slice();
        app.playlist_list_state.select(Some(0));

        app.next_playlist();
        assert_eq!(app.playlist_list_state.selected(), Some(1));
        app.next_playlist();
        assert_eq!(app.playlist_list_state.selected(), Some(0));

        app.previous_playlist();
        assert_eq!(app.playlist_list_state.selected(), Some(1));
        app.previous_playlist();
        assert_eq!(app.playlist_list_state.selected(), Some(0));
    }

    #[test]
    fn test_input_mode_transitions() {
        let mut app = create_test_app();
        assert_eq!(app.input_mode, InputMode::Offline);
        
        app.input_mode = InputMode::Search;
        assert_eq!(app.input_mode, InputMode::Search);
        
        app.input_mode = InputMode::PlaylistSelect;
        assert_eq!(app.input_mode, InputMode::PlaylistSelect);
        
        app.input_mode = InputMode::Online;
        assert_eq!(app.input_mode, InputMode::Online);
    }

    #[test]
    fn test_app_play_track_out_of_bounds() {
        let mut app = create_test_app();
        let result = app.play_track(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_app_parse_lrc_invalid_formats() {
        let mut app = create_test_app();
        let content = "[not:a.time]Text\nNoBracketAtAll\n[00:01.00]Valid";
        app.parse_lrc(content);
        assert_eq!(app.lyrics.len(), 1);
        assert_eq!(app.lyrics[0].text, "Valid");
    }

    #[test]
    fn test_app_parse_lrc_empty() {
        let mut app = create_test_app();
        app.parse_lrc("");
        assert!(app.lyrics.is_empty());
    }

    #[test]
    fn test_app_next_previous_empty() {
        let mut app = create_test_app();
        app.next();
        assert_eq!(app.list_state.selected(), None);
        app.previous();
        assert_eq!(app.list_state.selected(), None);
    }

    #[test]
    fn test_app_next_playlist_empty() {
        let mut app = create_test_app();
        app.next_playlist();
        assert_eq!(app.playlist_list_state.selected(), None);
    }

    #[test]
    fn test_app_generate_radio_art_cache() {
        let mut app = create_test_app();
        let name = "Test Radio";
        app.generate_radio_art(name);
        assert!(app.cached_image.is_some());
        
        let initial_img = Arc::clone(app.cached_image.as_ref().unwrap());
        app.generate_radio_art(name);
        assert!(Arc::ptr_eq(app.cached_image.as_ref().unwrap(), &initial_img));
    }

    #[tokio::test]
    async fn test_app_toggle_playback_flow() {
        let mut app = create_test_app();
        app.audio.is_empty.store(false, std::sync::atomic::Ordering::Relaxed);
        app.is_playing = true;
        
        app.toggle_playback().await;
        assert!(!app.is_playing);
        
        app.toggle_playback().await;
        assert!(app.is_playing);
    }

    #[test]
    fn test_app_filter_tracks_no_match() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), title: "A".into(), ..TrackMetadata::default() });
        app.all_tracks = Arc::from(vec![t1]);
        app.search_query = "Nonexistent".to_string();
        app.filter_tracks();
        assert!(app.filtered_tracks.is_empty());
        assert_eq!(app.list_state.selected(), None);
    }

    #[test]
    fn test_app_filter_radio_no_match() {
        let mut app = create_test_app();
        let s1 = Arc::new(crate::core::models::RadioStation { name: "A".into(), url: "u".into(), country: "c".into(), tags: None });
        app.radio_stations = vec![s1].into_boxed_slice();
        app.search_query = "Nonexistent".to_string();
        app.filter_radio();
        assert!(app.filtered_stations.is_empty());
        assert_eq!(app.radio_list_state.selected(), None);
    }

    #[test]
    fn test_app_play_radio_basic() {
        let mut app = create_test_app();
        let s1 = Arc::new(crate::core::models::RadioStation { name: "R1".into(), url: "u".into(), country: "c".into(), tags: None });
        app.filtered_stations = vec![s1].into_boxed_slice();
        app.play_radio(0);
        assert!(app.is_starting);
        assert!(app.is_playing);
        assert!(app.current_track.is_some());
        assert_eq!(app.current_track.as_ref().unwrap().title, "R1");
    }

    #[test]
    fn test_app_next_previous_playlist_bounds() {
        let mut app = create_test_app();
        app.playlists = vec![
            Playlist { id: "1".into(), name: "P1".into() },
        ].into_boxed_slice();
        app.playlist_list_state.select(Some(0));
        
        app.next_playlist();
        assert_eq!(app.playlist_list_state.selected(), Some(0));
        app.previous_playlist();
        assert_eq!(app.playlist_list_state.selected(), Some(0));
    }

    #[test]
    fn test_app_filter_tracks_case_insensitive() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), title: "ABC".into(), ..TrackMetadata::default() });
        app.all_tracks = Arc::from(vec![t1]);
        app.search_query = "abc".to_string();
        app.filter_tracks();
        assert_eq!(app.filtered_tracks.len(), 1);
    }

    #[test]
    fn test_app_filter_radio_case_insensitive() {
        let mut app = create_test_app();
        let s1 = Arc::new(crate::core::models::RadioStation { name: "XYZ".into(), url: "u".into(), country: "c".into(), tags: None });
        app.radio_stations = vec![s1].into_boxed_slice();
        app.search_query = "xyz".to_string();
        app.filter_radio();
        assert_eq!(app.filtered_stations.len(), 1);
    }

    #[test]
    fn test_app_parse_lrc_out_of_order() {
        let mut app = create_test_app();
        let content = "[00:10.00]Late\n[00:01.00]Early";
        app.parse_lrc(content);
        assert_eq!(app.lyrics[0].text, "Early");
        assert_eq!(app.lyrics[1].text, "Late");
    }

    #[test]
    fn test_app_circular_navigation_single_item() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), ..TrackMetadata::default() });
        app.filtered_tracks = Arc::from(vec![t1]);
        app.list_state.select(Some(0));
        app.next();
        assert_eq!(app.list_state.selected(), Some(0));
        app.previous();
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[tokio::test]
    async fn test_app_toggle_playback_radio() {
        let mut app = create_test_app();
        app.input_mode = InputMode::Online;
        let s1 = Arc::new(crate::core::models::RadioStation { name: "R1".into(), url: "http://test".into(), country: "c".into(), tags: None });
        app.filtered_stations = vec![s1].into_boxed_slice();
        app.radio_list_state.select(Some(0));
        app.audio.is_empty.store(true, std::sync::atomic::Ordering::Relaxed);
        
        app.toggle_playback().await;
        assert!(app.is_playing);
        assert_eq!(app.current_track.as_ref().unwrap().title, "R1");
    }

    #[test]
    fn test_app_play_next_previous_bounds() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), ..TrackMetadata::default() });
        app.playback_track_list = Arc::from(vec![t1]);
        app.playing_idx = Some(0);
        
        app.play_next();
        assert_eq!(app.playing_idx, Some(0));
        app.play_previous();
        assert_eq!(app.playing_idx, Some(0));
    }

    #[tokio::test]
    async fn test_app_filter_tracks_partial() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), title: "Testing".into(), ..TrackMetadata::default() });
        app.all_tracks = Arc::from(vec![t1]);
        app.search_query = "test".to_string();
        app.filter_tracks();
        assert_eq!(app.filtered_tracks.len(), 1);
    }

    #[tokio::test]
    async fn test_app_filter_tracks_special_chars() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), title: "Rock & Roll!".into(), ..TrackMetadata::default() });
        app.all_tracks = Arc::from(vec![t1]);
        app.search_query = "&".to_string();
        app.filter_tracks();
        assert_eq!(app.filtered_tracks.len(), 1);
        app.search_query = "!".to_string();
        app.filter_tracks();
        assert_eq!(app.filtered_tracks.len(), 1);
    }

    #[tokio::test]
    async fn test_app_filter_radio_partial() {
        let mut app = create_test_app();
        let s1 = Arc::new(crate::core::models::RadioStation { name: "Jazz FM".into(), url: "u".into(), country: "c".into(), tags: None });
        app.radio_stations = vec![s1].into_boxed_slice();
        app.search_query = "jazz".to_string();
        app.filter_radio();
        assert_eq!(app.filtered_stations.len(), 1);
    }

    #[tokio::test]
    async fn test_app_filter_radio_special_chars() {
        let mut app = create_test_app();
        let s1 = Arc::new(crate::core::models::RadioStation { name: "R-A-D-I-O".into(), url: "u".into(), country: "c".into(), tags: None });
        app.radio_stations = vec![s1].into_boxed_slice();
        app.search_query = "-".to_string();
        app.filter_radio();
        assert_eq!(app.filtered_stations.len(), 1);
    }

    #[tokio::test]
    async fn test_app_refresh_library_persistence_of_selection() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), title: "A".into(), ..TrackMetadata::default() });
        let t2 = Arc::new(TrackMetadata { track_id: "2".into(), title: "B".into(), ..TrackMetadata::default() });
        app.all_tracks = Arc::from(vec![t1.clone(), t2.clone()]);
        app.filtered_tracks = Arc::clone(&app.all_tracks);
        app.list_state.select(Some(1)); // Select B

        // Simulate refresh where B still exists but at a different index
        let t3 = Arc::new(TrackMetadata { track_id: "3".into(), title: "0".into(), ..TrackMetadata::default() });
        let new_tracks = vec![t3, t1, t2]; // Now B is at index 2
        app.refresh_tx.send(RefreshUpdate { all_tracks: new_tracks.into_boxed_slice(), playlists: vec![].into_boxed_slice() }).unwrap();
        
        app.update().await;
        
        assert_eq!(app.list_state.selected(), Some(2));
        assert_eq!(app.filtered_tracks[2].track_id, "2");
    }

    #[tokio::test]
    async fn test_app_refresh_library_selection_lost_if_removed() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), title: "A".into(), ..TrackMetadata::default() });
        app.all_tracks = Arc::from(vec![t1.clone()]);
        app.filtered_tracks = Arc::clone(&app.all_tracks);
        app.list_state.select(Some(0));

        // Simulate refresh where A is gone
        let new_tracks = vec![];
        app.refresh_tx.send(RefreshUpdate { all_tracks: new_tracks.into_boxed_slice(), playlists: vec![].into_boxed_slice() }).unwrap();
        
        app.update().await;
        
        assert_eq!(app.list_state.selected(), None);
    }

    #[tokio::test]
    async fn test_app_refresh_library_updates_current_track_metadata() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), title: "Old Title".into(), ..TrackMetadata::default() });
        app.current_track = Some(t1.clone());
        
        // Refresh with updated title
        let t1_new = Arc::new(TrackMetadata { track_id: "1".into(), title: "New Title".into(), ..TrackMetadata::default() });
        app.refresh_tx.send(RefreshUpdate { all_tracks: vec![t1_new].into_boxed_slice(), playlists: vec![].into_boxed_slice() }).unwrap();
        
        app.update().await;
        
        assert_eq!(app.current_track.unwrap().title, "New Title");
    }

    #[test]
    fn test_input_mode_transition_full_cycle() {
        let mut app = create_test_app();
        app.input_mode = InputMode::PlaylistSelect;
        app.input_mode = InputMode::Offline;
        app.input_mode = InputMode::Online;
        app.input_mode = InputMode::Search;
        app.input_mode = InputMode::Offline;
        assert_eq!(app.input_mode, InputMode::Offline);
    }

    #[test]
    fn test_input_mode_search_to_previous_online() {
        let mut app = create_test_app();
        app.input_mode = InputMode::Online;
        app.previous_mode = InputMode::Online;
        app.input_mode = InputMode::Search;
        // Simulate Esc or Confirm
        app.input_mode = app.previous_mode;
        assert_eq!(app.input_mode, InputMode::Online);
    }

    #[tokio::test]
    async fn test_visual_state_kinematics_velocity() {
        let mut app = create_test_app();
        app.is_playing = true;
        app.input_mode = InputMode::Offline;
        
        // Simulate amplitude increase
        {
            let mut dsp = app.audio.dsp_state.write().unwrap();
            dsp.amplitude = 0.5;
        }
        app.update().await;
        let v1 = app.visual_state.velocity;
        assert!(v1 > 0.0);
        
        // Update again, velocity should continue to affect position
        let p1 = app.visual_state.position;
        app.update().await;
        assert!(app.visual_state.position != p1);
    }

    #[tokio::test]
    async fn test_visual_state_kinematics_angular() {
        let mut app = create_test_app();
        app.is_playing = true;
        app.input_mode = InputMode::Online;
        
        {
            let mut dsp = app.audio.dsp_state.write().unwrap();
            dsp.spectrum = vec![0.1; 1024]; // High spectrum sum
        }
        app.update().await;
        assert!(app.visual_state.angular_velocity > 0.0);
        let r1 = app.visual_state.rotation;
        app.update().await;
        assert!(app.visual_state.rotation != r1);
    }

    #[tokio::test]
    async fn test_metadata_update_ignores_wrong_index() {
        let mut app = create_test_app();
        app.playing_idx = Some(1);
        
        app.metadata_tx.send(TrackMetadataUpdate {
            idx: 2, // Wrong index
            sample_rate: 44100,
            channels: 2,
            bitrate: 320,
            bit_depth: 16,
            title: Some("New".into()),
            artist: None, album: None, genre: None, label: None, description: None, cover_art: None,
            duration: Duration::from_secs(100),
        }).unwrap();
        
        app.update().await;
        assert_eq!(app.sample_rate, 0); // Not updated
    }

    #[tokio::test]
    async fn test_metadata_update_updates_verified_status() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), title: "Unverified".into(), status: Some("unverified".into()), ..TrackMetadata::default() });
        app.playback_track_list = Arc::from(vec![t1]);
        app.playing_idx = Some(0);
        app.current_track = Some(app.playback_track_list[0].clone());

        app.metadata_tx.send(TrackMetadataUpdate {
            idx: 0,
            sample_rate: 44100,
            channels: 2,
            bitrate: 320,
            bit_depth: 16,
            title: Some("Verified Title".into()),
            artist: None, album: None, genre: None, label: None, description: None, cover_art: None,
            duration: Duration::from_secs(100),
        }).unwrap();

        app.update().await;
        assert_eq!(app.current_track.as_ref().unwrap().title, "Verified Title");
        assert_eq!(app.current_track.as_ref().unwrap().status.as_deref(), Some("verified"));
    }

    #[test]
    fn test_generate_radio_art_caches_correctly() {
        let mut app = create_test_app();
        app.generate_radio_art("Radio1");
        let cache_len = app.image_cache.len();
        assert_eq!(cache_len, 1);
        
        app.generate_radio_art("Radio1");
        assert_eq!(app.image_cache.len(), 1); // Same name, same accent
    }

    #[test]
    fn test_generate_radio_art_reloads_on_accent_change() {
        let mut app = create_test_app();
        app.theme.accent = ratatui::style::Color::Red;
        app.generate_radio_art("Radio1");
        
        app.theme.accent = ratatui::style::Color::Blue;
        app.generate_radio_art("Radio1");
        assert_eq!(app.image_cache.len(), 2); // Different cache keys due to accent
    }

    #[test]
    fn test_volume_up_clamping() {
        let mut app = create_test_app();
        app.volume = 0.98;
        app.volume_up();
        assert!(app.volume <= 1.0);
        
        app.volume = 1.0;
        app.volume_up();
        assert_eq!(app.volume, 1.0);
    }

    #[test]
    fn test_volume_down_clamping() {
        let mut app = create_test_app();
        app.volume = 0.02;
        app.volume_down();
        assert!(app.volume >= 0.0);
        
        app.volume = 0.0;
        app.volume_down();
        assert_eq!(app.volume, 0.0);
    }

    #[tokio::test]
    async fn test_lyrics_auto_scroll_behavior() {
        let mut app = create_test_app();
        app.audio.is_empty.store(false, std::sync::atomic::Ordering::Relaxed);
        for i in 0..20 {
            app.lyrics.push(crate::player::audio::LyricLine { 
                time: Duration::from_secs(i as u64 * 10), 
                text: format!("L{}", i) 
            });
        }
        app.is_playing = true;
        app.current_track_duration = Duration::from_secs(1000);
        app.playback_start = Some(Instant::now() - Duration::from_secs(111));
        app.auto_scroll = true;
        
        app.update().await;
        assert!(app.current_lyric_idx >= 11);
        assert!(app.lyrics_scroll >= 6);
    }

    #[tokio::test]
    async fn test_lyrics_auto_scroll_disabled() {
        let mut app = create_test_app();
        app.auto_scroll = false;
        app.lyrics_scroll = 42;
        app.current_lyric_idx = 10;
        app.update().await;
        assert_eq!(app.lyrics_scroll, 42); // Should not change
    }

    #[tokio::test]
    async fn test_app_refresh_library_new_track_added() {
        let mut app = create_test_app();
        let t1 = Arc::new(TrackMetadata { track_id: "1".into(), title: "A".into(), ..TrackMetadata::default() });
        app.all_tracks = Arc::from(vec![t1.clone()]);
        
        let t2 = Arc::new(TrackMetadata { track_id: "2".into(), title: "B".into(), ..TrackMetadata::default() });
        app.refresh_tx.send(RefreshUpdate { all_tracks: vec![t1, t2].into_boxed_slice(), playlists: vec![].into_boxed_slice() }).unwrap();
        
        app.update().await;
        assert_eq!(app.all_tracks.len(), 2);
    }

    #[tokio::test]
    async fn test_visual_state_beat_flash_decay() {
        let mut app = create_test_app();
        app.is_playing = true;
        app.input_mode = InputMode::Offline;
        app.visual_state.beat_flash = 1.0;
        
        {
            let mut dsp = app.audio.dsp_state.write().unwrap();
            dsp.is_beat = false;
        }
        app.update().await;
        assert!(app.visual_state.beat_flash < 1.0);
    }
}
