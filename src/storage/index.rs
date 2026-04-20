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
}
