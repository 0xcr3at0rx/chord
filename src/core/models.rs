use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrackMetadata {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub album_art_url: Option<String>,
    pub duration_ms: Option<i64>,
    pub genres: Option<String>,
    pub file_size: Option<i64>,
    pub file_mtime: Option<f64>,
    pub file_path: Option<String>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub bit_depth: Option<u8>,
    pub sampling_rate: Option<u32>,
    pub status: Option<String>,
    #[serde(default)]
    pub search_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioStation {
    pub name: String,
    pub url: String,
    pub country: String,
    pub tags: Option<String>,
}
