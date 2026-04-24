use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RadioStation {
    pub name: SmolStr,
    pub url: String,
    pub country: SmolStr,
    pub tags: Option<SmolStr>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use toml;

    #[test]
    fn test_track_metadata_serialization_happy_path() {
        let track = TrackMetadata {
            track_id: SmolStr::new("123"),
            title: SmolStr::new("Song Title"),
            artist: SmolStr::new("Artist Name"),
            album: Some(SmolStr::new("Album Name")),
            duration_ms: Some(300000),
            ..Default::default()
        };
        let serialized = serde_json::to_string(&track).unwrap();
        assert!(serialized.contains("\"track_id\":\"123\""));
        assert!(serialized.contains("\"title\":\"Song Title\""));
        assert!(serialized.contains("\"duration_ms\":300000"));
    }

    #[test]
    fn test_track_metadata_deserialization_happy_path() {
        let json = r#"{
            "track_id": "123",
            "title": "Song Title",
            "artist": "Artist Name",
            "album": "Album Name",
            "duration_ms": 300000
        }"#;
        let deserialized: TrackMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.track_id, "123");
        assert_eq!(deserialized.title, "Song Title");
        assert_eq!(deserialized.duration_ms, Some(300000));
    }

    #[test]
    fn test_track_metadata_default() {
        let track = TrackMetadata::default();
        assert_eq!(track.track_id, "");
        assert_eq!(track.title, "");
        assert_eq!(track.artist, "");
        assert!(track.album.is_none());
        assert!(track.duration_ms.is_none());
    }

    #[test]
    fn test_track_metadata_partial_fields() {
        let json = r#"{
            "track_id": "456",
            "title": "Another Song",
            "artist": "Another Artist"
        }"#;
        let deserialized: TrackMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.track_id, "456");
        assert!(deserialized.album.is_none());
        assert!(deserialized.duration_ms.is_none());
    }

    #[test]
    fn test_track_metadata_all_none() {
        let track = TrackMetadata {
            track_id: SmolStr::new("id"),
            title: SmolStr::new("title"),
            artist: SmolStr::new("artist"),
            album: None,
            album_art_url: None,
            duration_ms: None,
            genres: None,
            file_size: None,
            file_mtime: None,
            file_path: None,
            last_verified_at: None,
            genre: None,
            label: None,
            bit_depth: None,
            sampling_rate: None,
            status: None,
        };
        let serialized = serde_json::to_string(&track).unwrap();
        assert!(serialized.contains("\"album\":null"));
        assert!(serialized.contains("\"duration_ms\":null"));
    }

    #[test]
    fn test_smolstr_empty() {
        let track = TrackMetadata {
            track_id: SmolStr::new(""),
            ..Default::default()
        };
        assert_eq!(track.track_id, "");
        assert!(track.track_id.is_empty());
    }

    #[test]
    fn test_smolstr_large() {
        let large_string = "a".repeat(1000);
        let track = TrackMetadata {
            track_id: SmolStr::new(&large_string),
            ..Default::default()
        };
        assert_eq!(track.track_id, large_string);
    }

    #[test]
    fn test_smolstr_special_chars() {
        let special = "🦀 Rocket 🚀";
        let track = TrackMetadata {
            title: SmolStr::new(special),
            ..Default::default()
        };
        assert_eq!(track.title, special);
    }

    #[test]
    fn test_radio_station_serialization() {
        let station = RadioStation {
            name: SmolStr::new("Station"),
            url: "http://example.com".to_string(),
            country: SmolStr::new("Country"),
            tags: Some(SmolStr::new("tag1,tag2")),
        };
        let serialized = serde_json::to_string(&station).unwrap();
        assert!(serialized.contains("\"name\":\"Station\""));
        assert!(serialized.contains("\"url\":\"http://example.com\""));
    }

    #[test]
    fn test_radio_station_deserialization() {
        let json = r#"{
            "name": "Station",
            "url": "http://example.com",
            "country": "Country",
            "tags": "tag1,tag2"
        }"#;
        let deserialized: RadioStation = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.name, "Station");
        assert_eq!(deserialized.url, "http://example.com");
        assert_eq!(deserialized.tags, Some(SmolStr::new("tag1,tag2")));
    }

    #[test]
    fn test_radio_station_missing_optional() {
        let json = r#"{
            "name": "Station",
            "url": "http://example.com",
            "country": "Country"
        }"#;
        let deserialized: RadioStation = serde_json::from_str(json).unwrap();
        assert!(deserialized.tags.is_none());
    }

    #[test]
    fn test_track_metadata_negative_duration() {
        let json = r#"{
            "track_id": "1",
            "title": "T",
            "artist": "A",
            "duration_ms": -1000
        }"#;
        let deserialized: TrackMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.duration_ms, Some(-1000));
    }

    #[test]
    fn test_track_metadata_zero_duration() {
        let json = r#"{
            "track_id": "1",
            "title": "T",
            "artist": "A",
            "duration_ms": 0
        }"#;
        let deserialized: TrackMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.duration_ms, Some(0));
    }

    #[test]
    fn test_track_metadata_large_duration() {
        let json = r#"{
            "track_id": "1",
            "title": "T",
            "artist": "A",
            "duration_ms": 9223372036854775807
        }"#;
        let deserialized: TrackMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.duration_ms, Some(i64::MAX));
    }

    #[test]
    fn test_track_metadata_large_file_size() {
        let json = r#"{
            "track_id": "1",
            "title": "T",
            "artist": "A",
            "file_size": 9223372036854775807
        }"#;
        let deserialized: TrackMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.file_size, Some(i64::MAX));
    }

    #[test]
    fn test_invalid_json_track_metadata() {
        let json = r#"{ "track_id": 123 }"#;
        let result: Result<TrackMetadata, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_toml_track_metadata() {
        let toml_str = r#"track_id = 123"#;
        let result: Result<TrackMetadata, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_json_radio_station() {
        let json = r#"{ "name": "Station" }"#;
        let result: Result<RadioStation, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_radio_station_empty_url() {
        let json = r#"{
            "name": "Station",
            "url": "",
            "country": "Country"
        }"#;
        let deserialized: RadioStation = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.url, "");
    }

    #[test]
    fn test_track_metadata_datetime_serialization() {
        let now = Utc::now();
        let track = TrackMetadata {
            last_verified_at: Some(now),
            ..Default::default()
        };
        let serialized = serde_json::to_string(&track).unwrap();
        let deserialized: TrackMetadata = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.last_verified_at.unwrap().to_rfc3339(), now.to_rfc3339());
    }

    #[test]
    fn test_track_metadata_file_mtime() {
        let json = r#"{
            "track_id": "1",
            "title": "T",
            "artist": "A",
            "file_mtime": 123456789.0
        }"#;
        let deserialized: TrackMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.file_mtime, Some(123456789.0));
    }

    #[test]
    fn test_track_metadata_toml_happy_path() {
        let toml_str = r#"
            track_id = "123"
            title = "Song Title"
            artist = "Artist Name"
            duration_ms = 300000
        "#;
        let deserialized: TrackMetadata = toml::from_str(toml_str).unwrap();
        assert_eq!(deserialized.track_id, "123");
        assert_eq!(deserialized.duration_ms, Some(300000));
    }

    #[test]
    fn test_track_metadata_massive_metadata() {
        let massive = "a".repeat(100_000);
        let track = TrackMetadata {
            title: SmolStr::new(&massive),
            artist: SmolStr::new(&massive),
            album: Some(SmolStr::new(&massive)),
            ..Default::default()
        };
        let serialized = serde_json::to_string(&track).unwrap();
        let deserialized: TrackMetadata = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.title, massive);
        assert_eq!(deserialized.artist, massive);
        assert_eq!(deserialized.album, Some(SmolStr::new(&massive)));
    }

    #[test]
    fn test_track_metadata_deeply_nested_genres() {
        let genres = "Rock, Metal, Power Metal, Symphonic, Epic, Melodic, Progressive, Neo-classical";
        let track = TrackMetadata {
            genres: Some(SmolStr::new(genres)),
            ..Default::default()
        };
        assert_eq!(track.genres.unwrap(), genres);
    }

    #[test]
    fn test_track_metadata_mtime_precision() {
        let mtime = 1715432100.123456789;
        let track = TrackMetadata {
            file_mtime: Some(mtime),
            ..Default::default()
        };
        let serialized = serde_json::to_string(&track).unwrap();
        let deserialized: TrackMetadata = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.file_mtime, Some(mtime));
    }

    #[test]
    fn test_track_metadata_custom_fields_special_chars() {
        let label = "© 2024 Records ™ ℗";
        let track = TrackMetadata {
            label: Some(SmolStr::new(label)),
            status: Some(SmolStr::new("Verified ✓")),
            ..Default::default()
        };
        assert_eq!(track.label.unwrap(), label);
        assert_eq!(track.status.unwrap(), "Verified ✓");
    }

    #[test]
    fn test_track_metadata_extreme_audio_values() {
        let track = TrackMetadata {
            sampling_rate: Some(192000),
            bit_depth: Some(32),
            ..Default::default()
        };
        assert_eq!(track.sampling_rate, Some(192000));
        assert_eq!(track.bit_depth, Some(32));
    }

    #[test]
    fn test_track_metadata_comparison() {
        let track1 = TrackMetadata {
            track_id: "1".into(),
            title: "Title".into(),
            ..Default::default()
        };
        let track2 = TrackMetadata {
            track_id: "1".into(),
            title: "Title".into(),
            ..Default::default()
        };
        assert_eq!(track1, track2);
        
        let track3 = TrackMetadata {
            track_id: "2".into(),
            ..Default::default()
        };
        assert_ne!(track1, track3);
    }
}
