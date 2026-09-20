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
    pub refresh_token: Option<String>,
    pub expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WindowState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub dark_mode: bool,
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        ProjectDirs::from("com", "example", "gcal-widget")
            .expect("Unable to determine config directory")
            .config_dir()
            .to_path_buf()
    }

    pub fn token_path() -> PathBuf {
        Self::config_dir().join("refresh_token.txt")
    }

    pub fn window_state_path() -> PathBuf {
        Self::config_dir().join("window.json")
    }

    pub fn load() -> anyhow::Result<Self> {
        let client_id = std::env::var("GOOGLE_CLIENT_ID")
            .map_err(|_| anyhow::anyhow!("GOOGLE_CLIENT_ID non impostata"))?;
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
            .map_err(|_| anyhow::anyhow!("GOOGLE_CLIENT_SECRET non impostata"))?;
        let calendar_id =
            std::env::var("GOOGLE_CALENDAR_ID").unwrap_or_else(|_| "primary".into());
        Ok(Self {
            client_id,
            client_secret,
            calendar_id,
        })
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