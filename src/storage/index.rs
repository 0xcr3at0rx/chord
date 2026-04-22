use crate::core::models::TrackMetadata;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Serialize, Deserialize, Default)]
struct LibraryCache {
    /// Key: Full Path as String
    tracks: BTreeMap<String, TrackMetadata>,
    last_scan: Option<chrono::DateTime<Utc>>,
}

pub struct LibraryIndex {
    cache_path: PathBuf,
    /// Ordered by file path for stable and fast prefix matching (playlists)
    tracks: Arc<RwLock<BTreeMap<String, Arc<TrackMetadata>>>>,
    playlists: Arc<RwLock<Vec<(String, String)>>>,
}

impl LibraryIndex {
    pub fn new(config_dir: &Path) -> Self {
        let cache_path = config_dir.join("library.toml");
        tracing::info!(path = ?cache_path, "Initializing LibraryIndex");
        Self {
            cache_path,
            tracks: Arc::new(RwLock::new(BTreeMap::new())),
            playlists: Arc::new(RwLock::new(Vec::new())),
        }
    }

    #[tracing::instrument(skip(self, music_dir))]
    pub async fn load_cache(&self, music_dir: &Path, force_rescan: bool) -> Result<()> {
        let cache_path = self.cache_path.clone();
        let music_dir_owned = music_dir.to_path_buf();
        tracing::debug!(force = force_rescan, "Starting cache load");

        // Canonicalize music_dir for reliable prefix matching
        let canonical_music_dir = music_dir
            .canonicalize()
            .unwrap_or_else(|_| {
                tracing::warn!(path = ?music_dir, "Could not canonicalize music_dir, using raw path");
                music_dir.to_path_buf()
            });

        let (tracks_map, playlists) = tokio::task::spawn_blocking(move || {
            let mut cache: LibraryCache = if cache_path.exists() {
                tracing::info!(path = ?cache_path, "Reading cache from disk");
                std::fs::read_to_string(&cache_path)
                    .ok()
                    .and_then(|s| {
                        let parsed = toml::from_str::<LibraryCache>(&s);
                        match &parsed {
                            Ok(c) => tracing::debug!(track_count = c.tracks.len(), "Successfully parsed cache"),
                            Err(e) => tracing::error!(error = %e, "Failed to parse cache file"),
                        }
                        parsed.ok()
                    })
                    .unwrap_or_else(|| {
                        tracing::warn!("Cache file invalid or unreadable, creating new");
                        LibraryCache::default()
                    })
            } else {
                tracing::info!("No cache file found, will perform fresh scan");
                LibraryCache::default()
            };

            // If cache is empty or force_rescan is true, perform a scan
            if (cache.tracks.is_empty() || force_rescan) && music_dir_owned.exists() {
                tracing::info!(path = ?music_dir_owned, "Scanning music directory");
                let mut new_tracks_found = 0;
                let mut skipped_tracks = 0;

                for entry in walkdir::WalkDir::new(&music_dir_owned)
                    .into_iter()
                    .flatten()
                {
                    let path = entry.path();
                    if path.is_file() {
                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        if matches!(ext.as_str(), "mp3" | "flac" | "ogg" | "wav" | "m4a") {
                            let path_str = path.to_string_lossy().to_string();
                            
                            // If not force_rescan and already in cache, skip
                            if !force_rescan && cache.tracks.contains_key(&path_str) {
                                skipped_tracks += 1;
                                continue;
                            }

                            tracing::trace!(path = %path_str, "Indexing new file");
                            let mut metadata = TrackMetadata {
                                track_id: uuid::Uuid::new_v4().to_string(),
                                file_path: Some(path_str.clone()),
                                ..Default::default()
                            };

                            // Try to extract metadata
                            if let Ok(probed) = lofty::read_from_path(path) {
                                use lofty::prelude::*;
                                if let Some(tag) = probed.primary_tag() {
                                    metadata.title = tag
                                        .title()
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| {
                                            path.file_name()
                                                .and_then(|f| f.to_str())
                                                .unwrap_or("Unknown")
                                                .to_string()
                                        });
                                    metadata.artist = tag
                                        .artist()
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| "Unknown Artist".to_string());
                                    metadata.album = tag.album().map(|s| s.to_string());
                                    metadata.genre = tag.genre().map(|s| s.to_string());
                                }
                            }

                            if metadata.title.is_empty() {
                                metadata.title = path
                                    .file_name()
                                    .and_then(|f| f.to_str())
                                    .unwrap_or("Unknown")
                                    .to_string();
                            }
                            if metadata.artist.is_empty() {
                                metadata.artist = "Unknown Artist".to_string();
                            }

                            metadata.search_key = format!(
                                "{} {} {}",
                                metadata.title,
                                metadata.artist,
                                metadata.album.as_deref().unwrap_or("")
                            )
                            .to_lowercase();
                            cache.tracks.insert(path_str, metadata);
                            new_tracks_found += 1;
                        }
                    }
                }
                
                tracing::info!(new = new_tracks_found, skipped = skipped_tracks, "Scan complete");

                // Save scan results
                if let Ok(toml_str) = toml::to_string(&cache) {
                    tracing::debug!(path = ?cache_path, "Saving cache to disk");
                    let _ = std::fs::write(&cache_path, toml_str);
                }
            }

            let mut needs_save = false;
            let mut updated_metadata = 0;

            for track in cache.tracks.values_mut() {
                // If metadata is unknown, try to re-read it
                if track.artist == "Unknown Artist" {
                    if let Some(path_str) = &track.file_path {
                        let path = Path::new(path_str);
                        if path.exists() {
                            tracing::trace!(path = %path_str, "Attempting to recover missing metadata");
                            if let Ok(probed) = lofty::read_from_path(path) {
                                use lofty::prelude::*;
                                if let Some(tag) = probed.primary_tag() {
                                    track.title = tag
                                        .title()
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| track.title.clone());
                                    track.artist = tag
                                        .artist()
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| "Unknown Artist".to_string());
                                    track.album = tag.album().map(|s| s.to_string());
                                    track.genre = tag.genre().map(|s| s.to_string());
                                    needs_save = true;
                                    updated_metadata += 1;

                                    // Force update search_key since metadata changed
                                    track.search_key = format!(
                                        "{} {} {}",
                                        track.title,
                                        track.artist,
                                        track.album.as_deref().unwrap_or("")
                                    )
                                    .to_lowercase();
                                }
                            }
                        }
                    }
                }

                if track.search_key.is_empty() {
                    track.search_key = format!(
                        "{} {} {}",
                        track.title,
                        track.artist,
                        track.album.as_deref().unwrap_or("")
                    )
                    .to_lowercase();
                    needs_save = true;
                }
            }

            if updated_metadata > 0 {
                tracing::info!(count = updated_metadata, "Recovered metadata for tracks");
            }

            if needs_save {
                if let Ok(toml_str) = toml::to_string(&cache) {
                    tracing::debug!(path = ?cache_path, "Updating cache file with new metadata");
                    let _ = std::fs::write(&cache_path, toml_str);
                }
            }

            let mut music_folders = std::collections::HashSet::new();
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

            tracing::debug!(folder_count = music_folders.len(), "Processing playlists/folders");

            let mut found_playlists: Vec<(String, String)> = music_folders
                .into_iter()
                .map(|p| {
                    let path_str = p.to_string_lossy().to_string();
                    (path_str.clone(), path_str)
                })
                .collect();
            found_playlists.sort_by(|a, b| a.0.cmp(&b.0));

            let arc_tracks: BTreeMap<String, Arc<TrackMetadata>> = cache
                .tracks
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect();
            (arc_tracks, found_playlists)
        })
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Background cache task failed");
            anyhow::anyhow!("Background task failed: {}", e)
        })?;

        let track_count = tracks_map.len();
        let playlist_count = playlists.len();

        let mut t_guard = self.tracks.write().await;
        *t_guard = tracks_map;

        let mut p_guard = self.playlists.write().await;
        *p_guard = playlists;

        tracing::info!(tracks = track_count, playlists = playlist_count, "LibraryIndex update complete");

        Ok(())
    }

    pub async fn get_all_tracks(&self) -> Vec<Arc<TrackMetadata>> {
        self.tracks.read().await.values().cloned().collect()
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
            .map(|(_, t)| t.clone())
            .collect()
    }
}
