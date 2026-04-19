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
    pub last_mode: crate::player::app::InputMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub device_name: Option<String>,
    pub volume: f32,
    pub mode: String,
    pub visualizer: crate::core::visualizer::VisualizerMode,
    pub sample_rate: u32,
    pub buffer_ms: u32,
    pub resample_quality: u32,
    pub bit_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub bg: String,
    pub fg: String,
    pub cursor_bg: String,
    pub cursor_fg: String,
    pub accent: String,
    pub accent_dim: String,
    pub critical: String,
    pub dim: String,
    pub status_bg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub library: LibraryConfig,
    pub audio: AudioConfig,
    pub theme: ThemeConfig,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            bg: "#121212".to_string(),
            fg: "#CCCCCC".to_string(),
            cursor_bg: "#2A2A2A".to_string(),
            cursor_fg: "#DDDDDD".to_string(),
            accent: "#1BFD9C".to_string(),
            accent_dim: "#66B2B2".to_string(),
            critical: "#BA0959".to_string(),
            dim: "#7A7A7A".to_string(),
            status_bg: "#2A2A2A".to_string(),
        }
    }
}

impl ThemeConfig {
    fn parse_hex(hex: &str) -> ratatui::style::Color {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            ratatui::style::Color::Rgb(r, g, b)
        } else {
            ratatui::style::Color::Reset
        }
    }

    pub fn to_theme(&self) -> crate::core::constants::Theme {
        crate::core::constants::Theme {
            bg: Self::parse_hex(&self.bg),
            fg: Self::parse_hex(&self.fg),
            cursor_bg: Self::parse_hex(&self.cursor_bg),
            cursor_fg: Self::parse_hex(&self.cursor_fg),
            accent: Self::parse_hex(&self.accent),
            accent_dim: Self::parse_hex(&self.accent_dim),
            critical: Self::parse_hex(&self.critical),
            dim: Self::parse_hex(&self.dim),
            status_bg: Self::parse_hex(&self.status_bg),
        }
    }
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
            last_mode: crate::player::app::InputMode::Normal,
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            volume: 1.0,
            mode: "PIPEWIRE".to_string(),
            visualizer: crate::core::visualizer::VisualizerMode::Bars,
            sample_rate: 48000,
            buffer_ms: 100,
            resample_quality: 4,
            bit_depth: 32,
        }
    }
}

use std::sync::RwLock;

#[derive(Debug)]
pub struct Settings {
    pub config: RwLock<Config>,
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

        let settings = Self {
            config: RwLock::new(Config::default()),
            config_dir,
        };

        settings.load_config(&config_file)?;

        Ok(settings)
    }

    fn load_config(&self, path: &Path) -> Result<()> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            if let Ok(config) = toml::from_str::<Config>(&content) {
                let mut guard = self.config.write().unwrap();
                *guard = config;
            }
        } else {
            let _ = self.save_config(path);
        }
        Ok(())
    }

    pub fn save_config(&self, path: &Path) -> Result<()> {
        let guard = self.config.read().unwrap();
        fs::write(path, toml::to_string_pretty(&*guard)?)?;
        Ok(())
    }
}
