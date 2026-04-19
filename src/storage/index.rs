use crate::core::models::TrackMetadata;
use anyhow::Result;
use chrono::Utc;
use lofty::file::AudioFile;
use lofty::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Default)]
struct LibraryCache {
    /// Key: Full Path as String
    tracks: BTreeMap<String, TrackMetadata>,
    last_scan: Option<chrono::DateTime<Utc>>,
}

pub struct LibraryIndex {
    cache_path: PathBuf,
    /// Ordered by file path for stable and fast prefix matching (playlists)
    tracks: Arc<RwLock<BTreeMap<String, TrackMetadata>>>,
    playlists: Arc<RwLock<Vec<(String, String)>>>,
}

impl LibraryIndex {
    pub fn new(config_dir: &Path) -> Self {
        let cache_path = config_dir.join("library_cache.toml");
        Self {
            cache_path,
            tracks: Arc::new(RwLock::new(BTreeMap::new())),
            playlists: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn load_cache(&self, music_dir: &Path) -> Result<()> {
        if !self.cache_path.exists() {
            return Ok(());
        }

        let cache_path = self.cache_path.clone();
        // Canonicalize music_dir for reliable prefix matching
        let music_dir = music_dir.canonicalize().unwrap_or_else(|_| music_dir.to_path_buf());

        let (tracks_map, playlists) = tokio::task::spawn_blocking(move || {
            let mut cache: LibraryCache = std::fs::read_to_string(&cache_path)
                .ok()
                .and_then(|s| toml::from_str(&s).ok())
                .unwrap_or_default();

            for track in cache.tracks.values_mut() {
                if track.search_key.is_empty() {
                    track.search_key = format!(
                        "{} {} {}",
                        track.title,
                        track.artist,
                        track.album.as_deref().unwrap_or("")
                    )
                    .to_lowercase();
                }
            }

            let mut music_folders = std::collections::HashSet::new();
            let canonical_music_dir = music_dir.canonicalize().unwrap_or_else(|_| music_dir.clone());
            for path_str in cache.tracks.keys() {
                let path = Path::new(path_str);
                if let Some(parent) = path.parent() {
                    if let Ok(rel_path) = parent.strip_prefix(&canonical_music_dir) {
                        if rel_path.as_os_str() != "" {
                            music_folders.insert(rel_path.to_path_buf());
                        }
                    }
                }
            }

            let mut found_playlists: Vec<(String, String)> = music_folders
                .into_iter()
                .map(|p| {
                    let path_str = p.to_string_lossy().to_string();
                    (path_str.clone(), path_str)
                })
                .collect();
            found_playlists.sort_by(|a, b| a.0.cmp(&b.0));

            (cache.tracks, found_playlists)
        })
        .await
        .map_err(|e| anyhow::anyhow!("Background task failed: {}", e))?;

        let mut t_guard = self.tracks.write().await;
        *t_guard = tracks_map;

        let mut p_guard = self.playlists.write().await;
        *p_guard = playlists;

        Ok(())
    }

    pub async fn get_all_tracks(&self) -> Vec<Arc<TrackMetadata>> {
        self.tracks
            .read()
            .await
            .values()
            .map(|t| Arc::new(t.clone()))
            .collect()
    }

    pub async fn get_playlists(&self) -> Vec<(String, String)> {
        self.playlists.read().await.clone()
    }

    pub async fn get_playlist_tracks(
        &self,
        playlist_name: &str,
        music_dir: &Path,
    ) -> Vec<Arc<TrackMetadata>> {
        let tracks = self.tracks.read().await;
        let mut playlist_path = music_dir.join(playlist_name).to_string_lossy().to_string();

        // Ensure the path ends with a separator to avoid matching similar prefixes (e.g. "Rock" matching "RockNRoll")
        if !playlist_path.ends_with(std::path::MAIN_SEPARATOR) {
            playlist_path.push(std::path::MAIN_SEPARATOR);
        }

        // Efficient prefix scan using BTreeMap range
        tracks
            .range(playlist_path.clone()..)
            .take_while(|(path, _)| path.starts_with(&playlist_path))
            .map(|(_, t)| Arc::new(t.clone()))
            .collect()
    }

    pub async fn update_index(&self, music_dir: &Path) -> Result<()> {
        if !music_dir.exists() {
            return Ok(());
        }

        let cache_path = self.cache_path.clone();
        // Canonicalize music_dir for reliable prefix matching
        let music_dir = music_dir.canonicalize().unwrap_or_else(|_| music_dir.to_path_buf());

        let (tracks_map, playlists) = tokio::task::spawn_blocking(move || {
            let mut cache: LibraryCache = if cache_path.exists() {
                std::fs::read_to_string(&cache_path)
                    .ok()
                    .and_then(|s| toml::from_str(&s).ok())
                    .unwrap_or_default()
            } else {
                LibraryCache::default()
            };

            let mut current_scan_paths = std::collections::HashSet::new();
            let mut music_folders = std::collections::HashSet::new();
            let mut pending_files = Vec::new();

            // 1. Scan Disk recursively for music files
            let canonical_music_dir = music_dir.canonicalize().unwrap_or_else(|_| music_dir.clone());
            for entry in WalkDir::new(&canonical_music_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let path = entry.path();
                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();

                    if ["flac", "mp3", "m4a", "wav", "ogg", "opus"].contains(&ext.as_str()) {
                        let path_str = path.to_string_lossy().to_string();
                        current_scan_paths.insert(path_str.clone());

                        // Track the folder this music file is in
                        if let Some(parent) = path.parent() {
                            if let Ok(rel_path) = parent.strip_prefix(&canonical_music_dir) {
                                if rel_path.as_os_str() != "" {
                                    music_folders.insert(rel_path.to_path_buf());
                                }
                            }
                        }

                        if let Ok(meta) = entry.metadata() {
                            let size = meta.len() as i64;
                            let mtime = meta
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs_f64())
                                .unwrap_or(0.0);

                            // Cache hit check
                            let mut needs_update = true;
                            if let Some(cached) = cache.tracks.get(&path_str) {
                                if cached.file_size == Some(size)
                                    && (cached.file_mtime.unwrap_or(0.0) - mtime).abs() < 0.1
                                {
                                    needs_update = false;
                                }
                            }

                            if needs_update {
                                pending_files.push((path.to_path_buf(), path_str, size, mtime));
                            }
                        }
                    }
                }
            }

            // Parse metadata in parallel for massive performance boost
            use rayon::prelude::*;
            let parsed_tracks: Vec<TrackMetadata> = pending_files
                .into_par_iter()
                .map(|(path, path_str, size, mtime)| {
                    let mut track = TrackMetadata {
                        track_id: Uuid::new_v4().to_string(),
                        isrc: None,
                        title: path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown")
                            .to_string(),
                        artist: "Unknown Artist".to_string(),
                        album: None,
                        album_art_url: None,
                        release_date: None,
                        duration_ms: None,
                        track_number: None,
                        genres: None,
                        genre: None,
                        label: None,
                        bit_depth: None,
                        sampling_rate: None,
                        file_size: Some(size),
                        file_mtime: Some(mtime),
                        file_path: Some(path_str.clone()),
                        last_verified_at: Some(Utc::now()),
                        downloaded_at: Some(Utc::now()),
                        status: Some("local".to_string()),
                        search_key: String::new(),
                    };

                    if let Ok(probed) = lofty::read_from_path(&path) {
                        let props = probed.properties();
                        track.duration_ms = Some(props.duration().as_millis() as i64);
                        track.bit_depth = props.bit_depth();
                        track.sampling_rate = props.sample_rate();

                        if let Some(tag) = probed.primary_tag() {
                            if let Some(title) = tag.title() {
                                track.title = title.to_string();
                            }
                            if let Some(artist) = tag.artist() {
                                track.artist = artist.to_string();
                            }
                            track.album = tag.album().map(|s| s.to_string());
                            track.genre = tag.genre().map(|s| s.to_string());
                            track.track_number = tag.track().map(|n| n as i32);
                        }
                    }

                    track.search_key = format!(
                        "{} {} {}",
                        track.title,
                        track.artist,
                        track.album.as_deref().unwrap_or("")
                    )
                    .to_lowercase();
                    track
                })
                .collect();

            // Insert newly parsed tracks into cache
            for track in parsed_tracks {
                if let Some(path_str) = &track.file_path {
                    cache.tracks.insert(path_str.clone(), track);
                }
            }

            // 2. Build playlists from found music folders
            let mut found_playlists: Vec<(String, String)> = music_folders
                .into_iter()
                .map(|p| {
                    let path_str = p.to_string_lossy().to_string();
                    (path_str.clone(), path_str)
                })
                .collect();
            found_playlists.sort_by(|a, b| a.0.cmp(&b.0));

            // 3. Cleanup stale entries
            cache
                .tracks
                .retain(|path, _| current_scan_paths.contains(path));

            // Save cache
            cache.last_scan = Some(Utc::now());
            if let Ok(toml_str) = toml::to_string_pretty(&cache) {
                let _ = std::fs::write(&cache_path, toml_str);
            }

            (cache.tracks, found_playlists)
        })
        .await
        .map_err(|e| anyhow::anyhow!("Background task failed: {}", e))?;

        let mut t_guard = self.tracks.write().await;
        *t_guard = tracks_map;

        let mut p_guard = self.playlists.write().await;
        *p_guard = playlists;

        Ok(())
    }
}
