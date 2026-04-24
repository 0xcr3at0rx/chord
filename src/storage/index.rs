use crate::core::models::TrackMetadata;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Serialize, Deserialize, Default)]
struct LibraryCache {
    /// Key: Full Path as SmolStr
    tracks: BTreeMap<SmolStr, TrackMetadata>,
    last_scan: Option<chrono::DateTime<Utc>>,
}

pub struct LibraryIndex {
    cache_path: PathBuf,
    /// Ordered by file path for stable and fast prefix matching (playlists)
    tracks: Arc<RwLock<BTreeMap<SmolStr, Arc<TrackMetadata>>>>,
    playlists: Arc<RwLock<Box<[(SmolStr, SmolStr)]>>>,
}

impl LibraryIndex {
    pub fn new(config_dir: &Path) -> Self {
        let cache_path = config_dir.join("library.toml");
        tracing::info!(path = ?cache_path, "Initializing LibraryIndex");
        Self {
            cache_path,
            tracks: Arc::new(RwLock::new(BTreeMap::new())),
            playlists: Arc::new(RwLock::new(Vec::new().into_boxed_slice())),
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
                            let path_str = SmolStr::from(path.to_string_lossy().to_string());
                            
                            let mtime = entry.metadata().ok().and_then(|m| m.modified().ok())
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs_f64());

                            // If not force_rescan and already in cache with same mtime, skip
                            if !force_rescan {
                                if let Some(existing) = cache.tracks.get(&path_str) {
                                    if existing.file_mtime == mtime && mtime.is_some() {
                                        skipped_tracks += 1;
                                        continue;
                                    }
                                }
                            }

                            tracing::trace!(path = %path_str, "Indexing file");
                            
                            // Preserve existing track_id if available in old cache
                            let existing_id = cache.tracks.get(&path_str).map(|t| t.track_id.clone());
                            
                            let mut metadata = TrackMetadata {
                                track_id: existing_id.unwrap_or_else(|| SmolStr::from(uuid::Uuid::new_v4().to_string())),
                                file_path: Some(path_str.clone()),
                                file_mtime: mtime,
                                ..Default::default()
                            };

                            // Lazy Metadata: Only extract basic info during scan
                            // Full tag extraction happens on-demand when playing or viewing details
                            metadata.title = path
                                .file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or("Unknown")
                                .to_string()
                                .into();
                            metadata.artist = "Unknown Artist".to_string().into();
                            metadata.status = Some("unverified".into());

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
                cache.last_scan = Some(Utc::now());
            }

            let mut music_folders = std::collections::HashSet::new();
            for path_str in cache.tracks.keys() {
                let path = Path::new(path_str.as_str());
                if let Some(parent) = path.parent() {
                    if let Ok(rel_path) = parent.strip_prefix(&canonical_music_dir) {
                        if rel_path.as_os_str() != "" {
                            music_folders.insert(rel_path.to_path_buf());
                        }
                    }
                }
            }

            tracing::debug!(folder_count = music_folders.len(), "Processing playlists/folders");

            let mut found_playlists: Vec<(SmolStr, SmolStr)> = music_folders
                .into_iter()
                .map(|p| {
                    let path_str = SmolStr::from(p.to_string_lossy().to_string());
                    (path_str.clone(), path_str)
                })
                .collect();
            found_playlists.sort_by(|a, b| a.0.cmp(&b.0));

            let arc_tracks: BTreeMap<SmolStr, Arc<TrackMetadata>> = cache
                .tracks
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect();
            (arc_tracks, found_playlists.into_boxed_slice())
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

    pub async fn get_all_tracks(&self) -> Box<[Arc<TrackMetadata>]> {
        self.tracks.read().await.values().cloned().collect::<Vec<_>>().into_boxed_slice()
    }

    pub async fn get_playlists(&self) -> Box<[(SmolStr, SmolStr)]> {
        self.playlists.read().await.clone()
    }

    pub async fn get_playlist_tracks(
        &self,
        playlist_name: &str,
        music_dir: &Path,
    ) -> Box<[Arc<TrackMetadata>]> {
        let tracks = self.tracks.read().await;
        let mut playlist_path = music_dir.join(playlist_name).to_string_lossy().to_string();

        // Ensure the path ends with a separator to avoid matching similar prefixes (e.g. "Rock" matching "RockNRoll")
        if !playlist_path.ends_with(std::path::MAIN_SEPARATOR) {
            playlist_path.push(std::path::MAIN_SEPARATOR);
        }

        // Efficient prefix scan using BTreeMap range
        tracks
            .range(SmolStr::from(playlist_path.as_str())..)
            .take_while(|(path, _)| path.starts_with(&playlist_path))
            .map(|(_, t)| t.clone())
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::TrackMetadata;
    use std::fs;

    fn create_temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("chord_test_index_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_library_index_new() {
        let temp_dir = create_temp_dir();
        let index = LibraryIndex::new(&temp_dir);
        assert_eq!(index.cache_path, temp_dir.join("library.toml"));
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_library_cache_serialization() {
        let mut cache = LibraryCache::default();
        let track = TrackMetadata {
            track_id: "test-id".into(),
            title: "Test Title".into(),
            artist: "Test Artist".into(),
            ..Default::default()
        };
        cache.tracks.insert("/path/to/track.mp3".into(), track);
        cache.last_scan = Some(Utc::now());

        let toml_str = toml::to_string(&cache).unwrap();
        assert!(toml_str.contains("test-id"));
        assert!(toml_str.contains("Test Title"));

        let deserialized: LibraryCache = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.tracks.len(), 1);
        assert_eq!(deserialized.tracks.get(&SmolStr::from("/path/to/track.mp3")).unwrap().track_id, "test-id");
    }

    #[tokio::test]
    async fn test_get_all_tracks() {
        let temp_dir = create_temp_dir();
        let index = LibraryIndex::new(&temp_dir);
        
        let track1 = Arc::new(TrackMetadata {
            track_id: "id1".into(),
            ..Default::default()
        });
        let track2 = Arc::new(TrackMetadata {
            track_id: "id2".into(),
            ..Default::default()
        });

        {
            let mut tracks = index.tracks.write().await;
            tracks.insert("/a/1.mp3".into(), track1);
            tracks.insert("/a/2.mp3".into(), track2);
        }

        let all_tracks = index.get_all_tracks().await;
        assert_eq!(all_tracks.len(), 2);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_get_playlists() {
        let temp_dir = create_temp_dir();
        let index = LibraryIndex::new(&temp_dir);
        
        let playlists = vec![
            ("Rock".into(), "Rock".into()),
            ("Jazz".into(), "Jazz".into()),
        ];
        
        {
            let mut p_guard = index.playlists.write().await;
            *p_guard = playlists.into_boxed_slice();
        }

        let result = index.get_playlists().await;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "Rock");
        assert_eq!(result[1].0, "Jazz");
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_get_playlist_tracks_prefix_matching() {
        let temp_dir = create_temp_dir();
        let music_dir = Path::new("/music");
        let index = LibraryIndex::new(&temp_dir);
        
        let track1 = Arc::new(TrackMetadata { track_id: "1".into(), ..Default::default() });
        let track2 = Arc::new(TrackMetadata { track_id: "2".into(), ..Default::default() });
        let track3 = Arc::new(TrackMetadata { track_id: "3".into(), ..Default::default() });

        {
            let mut tracks = index.tracks.write().await;
            tracks.insert("/music/Rock/song1.mp3".into(), track1);
            tracks.insert("/music/Rock/song2.mp3".into(), track2);
            tracks.insert("/music/Jazz/song1.mp3".into(), track3);
        }

        let rock_tracks = index.get_playlist_tracks("Rock", music_dir).await;
        assert_eq!(rock_tracks.len(), 2);

        let jazz_tracks = index.get_playlist_tracks("Jazz", music_dir).await;
        assert_eq!(jazz_tracks.len(), 1);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_get_playlist_tracks_trailing_separator() {
        let temp_dir = create_temp_dir();
        let music_dir = Path::new("/music");
        let index = LibraryIndex::new(&temp_dir);
        
        let track1 = Arc::new(TrackMetadata { track_id: "1".into(), ..Default::default() });
        let track2 = Arc::new(TrackMetadata { track_id: "2".into(), ..Default::default() });

        {
            let mut tracks = index.tracks.write().await;
            tracks.insert("/music/Rock/song1.mp3".into(), track1);
            // This should NOT be matched when looking for "Rock"
            tracks.insert("/music/RockAndRoll/song1.mp3".into(), track2);
        }

        let rock_tracks = index.get_playlist_tracks("Rock", music_dir).await;
        assert_eq!(rock_tracks.len(), 1);
        assert_eq!(rock_tracks[0].track_id, "1");

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_get_playlist_tracks_empty_or_nonexistent() {
        let temp_dir = create_temp_dir();
        let music_dir = Path::new("/music");
        let index = LibraryIndex::new(&temp_dir);

        let tracks = index.get_playlist_tracks("NonExistent", music_dir).await;
        assert_eq!(tracks.len(), 0);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_get_playlist_tracks_nested_folders() {
        let temp_dir = create_temp_dir();
        let music_dir = Path::new("/music");
        let index = LibraryIndex::new(&temp_dir);

        let track1 = Arc::new(TrackMetadata { track_id: "1".into(), ..Default::default() });
        let track2 = Arc::new(TrackMetadata { track_id: "2".into(), ..Default::default() });

        {
            let mut tracks = index.tracks.write().await;
            tracks.insert("/music/Rock/Classic/song1.mp3".into(), track1);
            tracks.insert("/music/Rock/Modern/song1.mp3".into(), track2);
        }

        // Searching for "Rock" should return BOTH nested folders
        let rock_tracks = index.get_playlist_tracks("Rock", music_dir).await;
        assert_eq!(rock_tracks.len(), 2);

        // Searching for "Rock/Classic" should return ONLY one
        let classic_rock = index.get_playlist_tracks("Rock/Classic", music_dir).await;
        assert_eq!(classic_rock.len(), 1);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_load_cache_non_existent_music_dir() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("non_existent");
        let index = LibraryIndex::new(&temp_dir);

        // Should not fail, just skip scanning
        let result = index.load_cache(&music_dir, false).await;
        assert!(result.is_ok());

        assert_eq!(index.get_all_tracks().await.len(), 0);
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_load_cache_empty_music_dir() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, false).await.unwrap();

        assert_eq!(index.get_all_tracks().await.len(), 0);
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_library_cache_default() {
        let cache = LibraryCache::default();
        assert!(cache.tracks.is_empty());
        assert!(cache.last_scan.is_none());
    }

    #[tokio::test]
    async fn test_library_index_initial_state() {
        let temp_dir = create_temp_dir();
        let index = LibraryIndex::new(&temp_dir);

        assert_eq!(index.get_all_tracks().await.len(), 0);
        assert_eq!(index.get_playlists().await.len(), 0);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_load_cache_force_rescan() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();

        let song_path = music_dir.join("song.mp3");
        fs::write(&song_path, "dummy").unwrap();

        let index = LibraryIndex::new(&temp_dir);

        // First scan
        index.load_cache(&music_dir, false).await.unwrap();
        assert_eq!(index.get_all_tracks().await.len(), 1);

        // Add another song
        let song2_path = music_dir.join("song2.mp3");
        fs::write(&song2_path, "dummy").unwrap();

        index.load_cache(&music_dir, false).await.unwrap();
        assert_eq!(index.get_all_tracks().await.len(), 1); // Still 1 because cache was not empty

        index.load_cache(&music_dir, true).await.unwrap();
        assert_eq!(index.get_all_tracks().await.len(), 2); // Now 2

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_get_playlist_tracks_case_sensitivity() {
        let temp_dir = create_temp_dir();
        let music_dir = Path::new("/music");
        let index = LibraryIndex::new(&temp_dir);
        
        let track1 = Arc::new(TrackMetadata { track_id: "1".into(), ..Default::default() });

        {
            let mut tracks = index.tracks.write().await;
            tracks.insert("/music/Rock/song1.mp3".into(), track1);
        }

        // On Linux it's case-sensitive
        let rock_tracks = index.get_playlist_tracks("rock", music_dir).await;
        if cfg!(windows) {
            // Windows is more complex due to canonicalization but let's assume case-insensitive for now if we were on windows
            // However, SmolStr and BTreeMap are case-sensitive.
        }
        assert_eq!(rock_tracks.len(), 0);

        let rock_tracks_correct = index.get_playlist_tracks("Rock", music_dir).await;
        assert_eq!(rock_tracks_correct.len(), 1);

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_load_cache_and_playlist_identification() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        let rock_dir = music_dir.join("Rock");
        fs::create_dir_all(&rock_dir).unwrap();
        
        let song_path = rock_dir.join("song.mp3");
        fs::write(&song_path, "dummy content").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        // We can't easily test load_cache's scanning because it depends on walkdir and real files
        // But we can test the logic after loading.
        
        index.load_cache(&music_dir, true).await.unwrap();
        
        let playlists = index.get_playlists().await;
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].0, "Rock");
        
        let tracks = index.get_playlist_tracks("Rock", &music_dir).await;
        assert_eq!(tracks.len(), 1);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_concurrent_load_cache_stress() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();
        for i in 0..20 {
            fs::write(music_dir.join(format!("song{}.mp3", i)), "dummy").unwrap();
        }

        let index = Arc::new(LibraryIndex::new(&temp_dir));
        let mut handles = vec![];

        for _ in 0..10 {
            let idx = index.clone();
            let m_dir = music_dir.clone();
            handles.push(tokio::spawn(async move {
                idx.load_cache(&m_dir, false).await
            }));
        }

        for h in handles {
            h.await.unwrap().unwrap();
        }

        assert_eq!(index.get_all_tracks().await.len(), 20);
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_load_cache_invalid_toml_format() {
        let temp_dir = create_temp_dir();
        let cache_path = temp_dir.join("library.toml");
        fs::write(&cache_path, "not a toml { [ [").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();
        fs::write(music_dir.join("test.mp3"), "dummy").unwrap();

        index.load_cache(&music_dir, false).await.unwrap();
        // Should have ignored the bad TOML and performed a fresh scan
        assert_eq!(index.get_all_tracks().await.len(), 1);
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_load_cache_missing_track_fields() {
        let temp_dir = create_temp_dir();
        let cache_path = temp_dir.join("library.toml");
        // Minimum fields to maybe pass? Actually all fields in TrackMetadata are optional except via Default.
        // But let's see if we can break it with missing data.
        fs::write(&cache_path, "[[tracks]]\ntrack_id = \"id1\"\n").unwrap(); // Missing title/artist which are SmolStr (default to empty)

        let index = LibraryIndex::new(&temp_dir);
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();

        index.load_cache(&music_dir, false).await.unwrap();
        // LibraryCache expects a BTreeMap for tracks, not a list. This is "invalid" TOML for LibraryCache.
        // It should handle it and start fresh.
        assert_eq!(index.get_all_tracks().await.len(), 0);
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_load_cache_invalid_field_types() {
        let temp_dir = create_temp_dir();
        let cache_path = temp_dir.join("library.toml");
        // tracks should be a table of TrackMetadata. Here we make it a string.
        fs::write(&cache_path, "tracks = \"oops\"").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();

        index.load_cache(&music_dir, false).await.unwrap();
        assert_eq!(index.get_all_tracks().await.len(), 0);
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_indexing_speed_2000_files() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();
        
        for i in 0..2000 {
            fs::write(music_dir.join(format!("song{}.mp3", i)), "dummy").unwrap();
        }

        let index = LibraryIndex::new(&temp_dir);
        let start = std::time::Instant::now();
        index.load_cache(&music_dir, false).await.unwrap();
        let duration = start.elapsed();
        
        assert_eq!(index.get_all_tracks().await.len(), 2000);
        // On modern systems 2000 empty files should be indexed in < 5 seconds even in debug mode
        assert!(duration.as_secs() < 10, "Indexing took too long: {:?}", duration);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_unicode_filenames() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();
        
        let files = vec!["🎵.mp3", "音楽.mp3", "música.mp3", "şarkı.mp3"];
        for f in &files {
            fs::write(music_dir.join(f), "dummy").unwrap();
        }

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, false).await.unwrap();
        
        let all_tracks = index.get_all_tracks().await;
        assert_eq!(all_tracks.len(), 4);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_unicode_playlist_names() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        let unicode_dir = music_dir.join("ジャンル");
        fs::create_dir_all(&unicode_dir).unwrap();
        fs::write(unicode_dir.join("test.mp3"), "dummy").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, false).await.unwrap();
        
        let playlists = index.get_playlists().await;
        assert!(playlists.iter().any(|p| p.0 == "ジャンル"));
        
        let tracks = index.get_playlist_tracks("ジャンル", &music_dir).await;
        assert_eq!(tracks.len(), 1);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_deeply_nested_directories_20() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        let mut current_dir = music_dir.clone();
        for i in 0..20 {
            current_dir = current_dir.join(format!("level{}", i));
        }
        fs::create_dir_all(&current_dir).unwrap();
        fs::write(current_dir.join("deep.mp3"), "dummy").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, false).await.unwrap();
        
        assert_eq!(index.get_all_tracks().await.len(), 1);
        
        // Test playlist retrieval of the deep folder
        let rel_path = current_dir.strip_prefix(&music_dir).unwrap().to_string_lossy();
        let tracks = index.get_playlist_tracks(&rel_path, &music_dir).await;
        assert_eq!(tracks.len(), 1);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_symlink_behavior() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();
        
        let real_file = temp_dir.join("real.mp3");
        fs::write(&real_file, "dummy").unwrap();
        
        #[cfg(unix)]
        {
            let symlink_file = music_dir.join("link.mp3");
            std::os::unix::fs::symlink(&real_file, &symlink_file).unwrap();
            
            let real_dir = temp_dir.join("real_dir");
            fs::create_dir_all(&real_dir).unwrap();
            fs::write(real_dir.join("nested.mp3"), "dummy").unwrap();
            
            let symlink_dir = music_dir.join("link_dir");
            std::os::unix::fs::symlink(&real_dir, &symlink_dir).unwrap();

            let index = LibraryIndex::new(&temp_dir);
            index.load_cache(&music_dir, false).await.unwrap();
            
            // WalkDir by default does not follow symlinks to directories,
            // but path.is_file() follows symlinks to files.
            // So link.mp3 IS found, but link_dir/nested.mp3 IS NOT.
            assert_eq!(index.get_all_tracks().await.len(), 1);
        }
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_mtime_handling_updates_on_change() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();
        
        let song_path = music_dir.join("song.mp3");
        fs::write(&song_path, "version 1").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, false).await.unwrap();
        
        let tracks = index.get_all_tracks().await;
        assert_eq!(tracks.len(), 1);
        let first_id = tracks[0].track_id.clone();
        
        // Wait a bit to ensure mtime changes
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        fs::write(&song_path, "version 2").unwrap();
        
        // Reload cache without force. It should detect mtime change and update.
        // (Wait, current implementation preserves track_id if path matches, but it SHOULD rescan metadata)
        index.load_cache(&music_dir, false).await.unwrap();
        
        let tracks2 = index.get_all_tracks().await;
        assert_eq!(tracks2.len(), 1);
        assert_eq!(tracks2[0].track_id, first_id); // ID should be preserved
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_cache_persistence_between_instances() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();
        fs::write(music_dir.join("song.mp3"), "dummy").unwrap();

        {
            let index1 = LibraryIndex::new(&temp_dir);
            index1.load_cache(&music_dir, false).await.unwrap();
            assert_eq!(index1.get_all_tracks().await.len(), 1);
        }

        // New instance with same config dir
        let index2 = LibraryIndex::new(&temp_dir);
        index2.load_cache(&music_dir, false).await.unwrap();
        assert_eq!(index2.get_all_tracks().await.len(), 1);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_playlist_special_chars_plus() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        let special_dir = music_dir.join("Playlist+");
        fs::create_dir_all(&special_dir).unwrap();
        fs::write(special_dir.join("song.mp3"), "dummy").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, false).await.unwrap();
        
        let tracks = index.get_playlist_tracks("Playlist+", &music_dir).await;
        assert_eq!(tracks.len(), 1);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_playlist_special_chars_star() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        let special_dir = music_dir.join("Playlist*");
        fs::create_dir_all(&special_dir).unwrap();
        fs::write(special_dir.join("song.mp3"), "dummy").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, false).await.unwrap();
        
        let tracks = index.get_playlist_tracks("Playlist*", &music_dir).await;
        assert_eq!(tracks.len(), 1);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_playlist_special_chars_brackets() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        let special_dir = music_dir.join("(Folder)");
        fs::create_dir_all(&special_dir).unwrap();
        fs::write(special_dir.join("song.mp3"), "dummy").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, false).await.unwrap();
        
        let tracks = index.get_playlist_tracks("(Folder)", &music_dir).await;
        assert_eq!(tracks.len(), 1);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_playlist_special_chars_mixed() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        let special_dir = music_dir.join("!@#$%^&()_+-=");
        fs::create_dir_all(&special_dir).unwrap();
        fs::write(special_dir.join("song.mp3"), "dummy").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, false).await.unwrap();
        
        let tracks = index.get_playlist_tracks("!@#$%^&()_+-=", &music_dir).await;
        assert_eq!(tracks.len(), 1);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_load_cache_empty_cache_file() {
        let temp_dir = create_temp_dir();
        let cache_path = temp_dir.join("library.toml");
        fs::write(&cache_path, "").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();
        fs::write(music_dir.join("test.mp3"), "dummy").unwrap();

        index.load_cache(&music_dir, false).await.unwrap();
        assert_eq!(index.get_all_tracks().await.len(), 1);
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_get_playlist_tracks_exact_match_only() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        let dir1 = music_dir.join("Rock");
        let dir2 = music_dir.join("RockAndRoll");
        fs::create_dir_all(&dir1).unwrap();
        fs::create_dir_all(&dir2).unwrap();
        fs::write(dir1.join("s1.mp3"), "d").unwrap();
        fs::write(dir2.join("s2.mp3"), "d").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, true).await.unwrap();
        
        let rock = index.get_playlist_tracks("Rock", &music_dir).await;
        assert_eq!(rock.len(), 1);
        
        let rock_and_roll = index.get_playlist_tracks("RockAndRoll", &music_dir).await;
        assert_eq!(rock_and_roll.len(), 1);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_load_cache_preserves_track_ids() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();
        let song_path = music_dir.join("song.mp3");
        fs::write(&song_path, "d").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, true).await.unwrap();
        let id1 = index.get_all_tracks().await[0].track_id.clone();

        // Second scan should preserve ID
        index.load_cache(&music_dir, true).await.unwrap();
        let id2 = index.get_all_tracks().await[0].track_id.clone();
        
        assert_eq!(id1, id2);
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_get_playlist_tracks_with_spaces() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        let special_dir = music_dir.join("My Favorite Music");
        fs::create_dir_all(&special_dir).unwrap();
        fs::write(special_dir.join("song.mp3"), "dummy").unwrap();

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, false).await.unwrap();
        
        let tracks = index.get_playlist_tracks("My Favorite Music", &music_dir).await;
        assert_eq!(tracks.len(), 1);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[tokio::test]
    async fn test_load_cache_multiple_extensions() {
        let temp_dir = create_temp_dir();
        let music_dir = temp_dir.join("music");
        fs::create_dir_all(&music_dir).unwrap();
        fs::write(music_dir.join("1.mp3"), "d").unwrap();
        fs::write(music_dir.join("2.flac"), "d").unwrap();
        fs::write(music_dir.join("3.ogg"), "d").unwrap();
        fs::write(music_dir.join("4.wav"), "d").unwrap();
        fs::write(music_dir.join("5.m4a"), "d").unwrap();
        fs::write(music_dir.join("6.txt"), "d").unwrap(); // Should ignore

        let index = LibraryIndex::new(&temp_dir);
        index.load_cache(&music_dir, true).await.unwrap();
        
        assert_eq!(index.get_all_tracks().await.len(), 5);
        fs::remove_dir_all(&temp_dir).ok();
    }
}
