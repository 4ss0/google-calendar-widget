//! Persistent configuration: OAuth credentials, calendar id, refresh token,
//! and window geometry.
//!
//! Sensitive files (config.bin, refresh_token.bin) are encrypted with DPAPI
//! (see crypto.rs). Legacy plaintext files (config.json, refresh_token.txt)
//! are read once and migrated automatically.
//!
//! Files live under the platform config dir, e.g. on Windows:
//! `%APPDATA%\example\gcal-widget\config\`.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub client_id: String,
    pub client_secret: String,
    pub calendar_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredToken {
    pub access_token: String,
    /// Long-lived token used to silently refresh `access_token`.
    pub refresh_token: Option<String>,
    /// Unix timestamp (seconds) of access token expiry.
    pub expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WindowState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Theme preference persisted alongside geometry.
    #[serde(default)]
    pub dark_mode: bool,
    /// True if the window position was never explicitly observed (the OS
    /// picked it). In that case `x`/`y` are meaningless and the next launch
    /// should ask the OS to choose the position again, instead of pinning
    /// the window to (0,0).
    #[serde(default)]
    pub position_is_default: bool,
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        ProjectDirs::from("com", "example", "gcal-widget")
            .expect("Unable to determine config directory")
            .config_dir()
            .to_path_buf()
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.bin")
    }

    pub fn legacy_config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn token_path() -> PathBuf {
        Self::config_dir().join("refresh_token.bin")
    }

    pub fn legacy_token_path() -> PathBuf {
        Self::config_dir().join("refresh_token.txt")
    }

    pub fn window_state_path() -> PathBuf {
        Self::config_dir().join("window.json")
    }

    /// Loads config from disk. Order of precedence:
    ///   1. Encrypted config.bin
    ///   2. Legacy plaintext config.json (migrated on success)
    ///   3. GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET env vars
    pub fn load() -> Option<Self> {
        if let Some(bytes) = crate::crypto::load_encrypted(&Self::config_path()) {
            if let Ok(cfg) = serde_json::from_slice::<AppConfig>(&bytes) {
                return Some(cfg);
            }
            // Corrupted payload: drop it so we don't loop forever.
            let _ = std::fs::remove_file(Self::config_path());
        }

        if let Ok(content) = std::fs::read_to_string(Self::legacy_config_path()) {
            if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                if cfg.save().is_ok() {
                    let _ = std::fs::remove_file(Self::legacy_config_path());
                }
                return Some(cfg);
            }
        }

        if let (Ok(client_id), Ok(client_secret)) = (
            std::env::var("GOOGLE_CLIENT_ID"),
            std::env::var("GOOGLE_CLIENT_SECRET"),
        ) {
            return Some(Self {
                client_id,
                client_secret,
                calendar_id: std::env::var("GOOGLE_CALENDAR_ID")
                    .unwrap_or_else(|_| "primary".into()),
            });
        }
        None
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        crate::crypto::save_encrypted(&Self::config_path(), &json)?;
        Ok(())
    }
}

impl WindowState {
    pub fn load() -> Option<Self> {
        let path = AppConfig::window_state_path();
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = AppConfig::window_state_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}