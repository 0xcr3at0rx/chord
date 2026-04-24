use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryConfig {
    pub music_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub volume: f32,
    pub mode: String,
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
        let hex = hex.trim().trim_start_matches('#');
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
        let music_dir = directories::BaseDirs::new()
            .map(|d| d.home_dir().join("music"))
            .unwrap_or_else(|| PathBuf::from("music"));

        Self { music_dir }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            volume: 1.0,
            mode: "PIPEWIRE".to_string(),
            sample_rate: 48000,
            buffer_ms: 200,
            resample_quality: 4,
            bit_depth: 32,
        }
    }
}


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
            if let Ok(mut config) = toml::from_str::<Config>(&content) {
                // Expand ~ in music_dir if present
                if config.library.music_dir.to_string_lossy().starts_with("~") {
                    if let Some(home) =
                        directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())
                    {
                        let path_str = config.library.music_dir.to_string_lossy().to_string();
                        if path_str.len() > 2 {
                            config.library.music_dir = home.join(&path_str[2..]);
                        } else {
                            config.library.music_dir = home;
                        }
                    }
                }

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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;
    use std::fs;
    use std::path::PathBuf;

    fn create_temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("chord_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_parse_hex_valid() {
        assert_eq!(ThemeConfig::parse_hex("#FFFFFF"), Color::Rgb(255, 255, 255));
        assert_eq!(ThemeConfig::parse_hex("#000000"), Color::Rgb(0, 0, 0));
        assert_eq!(ThemeConfig::parse_hex("#123456"), Color::Rgb(0x12, 0x34, 0x56));
    }

    #[test]
    fn test_parse_hex_no_hash() {
        assert_eq!(ThemeConfig::parse_hex("FFFFFF"), Color::Rgb(255, 255, 255));
        assert_eq!(ThemeConfig::parse_hex("123456"), Color::Rgb(0x12, 0x34, 0x56));
    }

    #[test]
    fn test_parse_hex_mixed_case() {
        assert_eq!(ThemeConfig::parse_hex("#aBcDeF"), Color::Rgb(0xAB, 0xCD, 0xEF));
        assert_eq!(ThemeConfig::parse_hex("AbCdEf"), Color::Rgb(0xAB, 0xCD, 0xEF));
    }

    #[test]
    fn test_parse_hex_too_short() {
        assert_eq!(ThemeConfig::parse_hex("#FFF"), Color::Reset);
        assert_eq!(ThemeConfig::parse_hex("FF"), Color::Reset);
    }

    #[test]
    fn test_parse_hex_too_long() {
        assert_eq!(ThemeConfig::parse_hex("#FFFFFFF"), Color::Reset);
        assert_eq!(ThemeConfig::parse_hex("ABCDEF0"), Color::Reset);
    }

    #[test]
    fn test_parse_hex_invalid_chars() {
        // u8::from_str_radix will fail and unwrap_or(0) will be used
        assert_eq!(ThemeConfig::parse_hex("#GGGGGG"), Color::Rgb(0, 0, 0));
        assert_eq!(ThemeConfig::parse_hex("#FFGG00"), Color::Rgb(255, 0, 0));
    }

    #[test]
    fn test_parse_hex_empty() {
        assert_eq!(ThemeConfig::parse_hex(""), Color::Reset);
        assert_eq!(ThemeConfig::parse_hex("#"), Color::Reset);
    }

    #[test]
    fn test_parse_hex_whitespace() {
        assert_eq!(ThemeConfig::parse_hex("  #FFFFFF  "), Color::Rgb(255, 255, 255));
        assert_eq!(ThemeConfig::parse_hex("\n123456\t"), Color::Rgb(0x12, 0x34, 0x56));
    }

    #[test]
    fn test_theme_config_to_theme() {
        let config = ThemeConfig {
            bg: "#111111".into(),
            fg: "#222222".into(),
            cursor_bg: "#333333".into(),
            cursor_fg: "#444444".into(),
            accent: "#555555".into(),
            accent_dim: "#666666".into(),
            critical: "#777777".into(),
            dim: "#888888".into(),
            status_bg: "#999999".into(),
        };
        let theme = config.to_theme();
        assert_eq!(theme.bg, Color::Rgb(0x11, 0x11, 0x11));
        assert_eq!(theme.fg, Color::Rgb(0x22, 0x22, 0x22));
        assert_eq!(theme.cursor_bg, Color::Rgb(0x33, 0x33, 0x33));
        assert_eq!(theme.cursor_fg, Color::Rgb(0x44, 0x44, 0x44));
        assert_eq!(theme.accent, Color::Rgb(0x55, 0x55, 0x55));
        assert_eq!(theme.accent_dim, Color::Rgb(0x66, 0x66, 0x66));
        assert_eq!(theme.critical, Color::Rgb(0x77, 0x77, 0x77));
        assert_eq!(theme.dim, Color::Rgb(0x88, 0x88, 0x88));
        assert_eq!(theme.status_bg, Color::Rgb(0x99, 0x99, 0x99));
    }

    #[test]
    fn test_library_config_default() {
        let config = LibraryConfig::default();
        // Should end with "music" or "music" folder in home
        assert!(config.music_dir.to_string_lossy().ends_with("music"));
    }

    #[test]
    fn test_audio_config_default() {
        let config = AudioConfig::default();
        assert_eq!(config.volume, 1.0);
        assert_eq!(config.mode, "PIPEWIRE");
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.buffer_ms, 200);
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.audio.volume, 1.0);
        assert!(config.library.music_dir.to_string_lossy().ends_with("music"));
        assert_eq!(config.theme.bg, "#121212");
    }

    #[test]
    fn test_tilde_expansion_with_subpath() {
        let temp_dir = create_temp_dir();
        let config_path = temp_dir.join("config.toml");
        
        let settings = Settings {
            config: RwLock::new(Config::default()),
            config_dir: temp_dir.clone(),
        };

        let content = "[library]\nmusic_dir = \"~/my_music\"";
        fs::write(&config_path, content).unwrap();
        settings.load_config(&config_path).unwrap();

        let config = settings.config.read().unwrap();
        let music_dir = config.library.music_dir.to_string_lossy();
        assert!(!music_dir.starts_with("~"));
        assert!(music_dir.contains("my_music"));
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_tilde_expansion_root() {
        let temp_dir = create_temp_dir();
        let config_path = temp_dir.join("config.toml");
        
        let settings = Settings {
            config: RwLock::new(Config::default()),
            config_dir: temp_dir.clone(),
        };

        let content = "[library]\nmusic_dir = \"~\"";
        fs::write(&config_path, content).unwrap();
        settings.load_config(&config_path).unwrap();

        let config = settings.config.read().unwrap();
        let music_dir = config.library.music_dir.to_string_lossy();
        assert!(!music_dir.starts_with("~"));
        
        if let Some(home) = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()) {
            assert_eq!(config.library.music_dir, home);
        }
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_load_config_missing_file() {
        let temp_dir = create_temp_dir();
        let config_path = temp_dir.join("config.toml");
        
        let settings = Settings {
            config: RwLock::new(Config::default()),
            config_dir: temp_dir.clone(),
        };

        settings.load_config(&config_path).unwrap();
        // Should create a default config file
        assert!(config_path.exists());
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("volume = 1.0"));
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_load_config_invalid_toml() {
        let temp_dir = create_temp_dir();
        let config_path = temp_dir.join("config.toml");
        fs::write(&config_path, "this is not toml").unwrap();
        
        let settings = Settings {
            config: RwLock::new(Config::default()),
            config_dir: temp_dir.clone(),
        };

        let initial_volume = settings.config.read().unwrap().audio.volume;
        settings.load_config(&config_path).unwrap();
        // Should keep default values if TOML is invalid
        assert_eq!(settings.config.read().unwrap().audio.volume, initial_volume);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_load_config_valid_toml() {
        let temp_dir = create_temp_dir();
        let config_path = temp_dir.join("config.toml");
        fs::write(&config_path, "[audio]\nvolume = 0.5\nmode = \"ALSA\"").unwrap();
        
        let settings = Settings {
            config: RwLock::new(Config::default()),
            config_dir: temp_dir.clone(),
        };

        settings.load_config(&config_path).unwrap();
        let config = settings.config.read().unwrap();
        assert_eq!(config.audio.volume, 0.5);
        assert_eq!(config.audio.mode, "ALSA");
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_load_config_partial_fields() {
        let temp_dir = create_temp_dir();
        let config_path = temp_dir.join("config.toml");
        // Only override one field
        fs::write(&config_path, "[audio]\nsample_rate = 44100").unwrap();
        
        let settings = Settings {
            config: RwLock::new(Config::default()),
            config_dir: temp_dir.clone(),
        };

        settings.load_config(&config_path).unwrap();
        let config = settings.config.read().unwrap();
        assert_eq!(config.audio.sample_rate, 44100);
        assert_eq!(config.audio.volume, 1.0); // kept default
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_save_config_creates_valid_toml() {
        let temp_dir = create_temp_dir();
        let config_path = temp_dir.join("config.toml");
        
        let settings = Settings {
            config: RwLock::new(Config::default()),
            config_dir: temp_dir.clone(),
        };
        
        {
            let mut config = settings.config.write().unwrap();
            config.audio.volume = 0.123;
        }

        settings.save_config(&config_path).unwrap();
        assert!(config_path.exists());
        
        let content = fs::read_to_string(&config_path).unwrap();
        let parsed: Config = toml::from_str(&content).unwrap();
        assert_eq!(parsed.audio.volume, 0.123);
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_save_config_preserves_other_fields() {
        let temp_dir = create_temp_dir();
        let config_path = temp_dir.join("config.toml");
        
        let settings = Settings {
            config: RwLock::new(Config::default()),
            config_dir: temp_dir.clone(),
        };
        
        settings.save_config(&config_path).unwrap();
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("sample_rate = 48000"));
        assert!(content.contains("bg = \"#121212\""));
        
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_load_config_no_tilde_expansion() {
        let temp_dir = create_temp_dir();
        let config_path = temp_dir.join("config.toml");
        
        let settings = Settings {
            config: RwLock::new(Config::default()),
            config_dir: temp_dir.clone(),
        };

        let path_str = if cfg!(windows) { "C:\\music" } else { "/music" };
        let content = format!("[library]\nmusic_dir = \"{}\"", path_str.replace("\\", "\\\\"));
        fs::write(&config_path, content).unwrap();
        settings.load_config(&config_path).unwrap();

        let config = settings.config.read().unwrap();
        assert_eq!(config.library.music_dir, PathBuf::from(path_str));
        
        fs::remove_dir_all(&temp_dir).ok();
    }
}
