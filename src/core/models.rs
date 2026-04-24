use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrackMetadata {
    pub track_id: SmolStr,
    pub title: SmolStr,
    pub artist: SmolStr,
    pub album: Option<SmolStr>,
    pub album_art_url: Option<SmolStr>,
    pub duration_ms: Option<i64>,
    pub genres: Option<SmolStr>,
    pub file_size: Option<i64>,
    pub file_mtime: Option<f64>,
    pub file_path: Option<SmolStr>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub genre: Option<SmolStr>,
    pub label: Option<SmolStr>,
    pub bit_depth: Option<u8>,
    pub sampling_rate: Option<u32>,
    pub status: Option<SmolStr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioStation {
    pub name: SmolStr,
    pub url: String,
    pub country: SmolStr,
    pub tags: Option<SmolStr>,
}
