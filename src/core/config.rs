use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryConfig {
    pub music_dir: PathBuf,
    pub scan_at_startup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FormatConfig {
    pub filename_format: String,
    pub services: Vec<String>,
    pub use_artist_subfolders: bool,
    pub use_album_subfolders: bool,
    pub use_playlist_folders: bool,
    pub playlist_parent_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub device_name: Option<String>,
    pub volume: f32,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub library: LibraryConfig,
    pub format: FormatConfig,
    pub audio: AudioConfig,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        let music_dir = directories::UserDirs::new()
            .and_then(|u| u.audio_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| {
                if let Some(proj_dirs) = ProjectDirs::from("", "", "chord") {
                    proj_dirs.data_dir().join("Music")
                } else {
                    PathBuf::from("Music")
                }
            });

        Self {
            music_dir,
            scan_at_startup: true,
        }
    }
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            filename_format: "{artist}/{year} - {album}/{track}. {title}".to_string(),
            services: vec!["qobuz".into(), "amazon".into(), "tidal".into(), "spoti".into(), "youtube".into()],
            use_artist_subfolders: true,
            use_album_subfolders: true,
            use_playlist_folders: true,
            playlist_parent_dir: String::new(),
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            volume: 1.0,
            mode: "PIPEWIRE".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct Settings {
    pub config: Config,
    pub config_dir: PathBuf,
}

impl Settings {
    pub fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("", "", "chord")
            .context("Failed to determine project directories")?;

        let config_dir = proj_dirs.config_dir().to_path_buf();
        let cache_dir = proj_dirs.cache_dir().to_path_buf();

        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&cache_dir)?;

        let config_file = config_dir.join("config.toml");

        let mut settings = Self {
            config: Config::default(),
            config_dir,
        };

        settings.load_config(&config_file)?;

        Ok(settings)
    }

    fn load_config(&mut self, path: &Path) -> Result<()> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            if let Ok(config) = toml::from_str::<Config>(&content) {
                self.config = config;
            }
        } else {
            let _ = self.save_config(path);
        }
        Ok(())
    }

    pub fn save_config(&self, path: &Path) -> Result<()> {
        fs::write(path, toml::to_string_pretty(&self.config)?)?;
        Ok(())
    }
}
