use crate::auth::oauth::{load_refresh_token, refresh_access_token};
use crate::config::{StoredToken, WindowState};
use iced::{Point, Size};

pub async fn ensure_token(
    client_id: &str,
    client_secret: &str,
    token: &str,
    expires_at: i64,
) -> Result<(String, i64, Option<StoredToken>), String> {
    let now = chrono::Utc::now().timestamp();
    if expires_at - now > 60 {
        return Ok((token.to_string(), expires_at, None));
    }
    let rt = load_refresh_token().ok_or_else(|| "Refresh token non trovato".to_string())?;
    let new_token = refresh_access_token(client_id, client_secret, &rt)
        .await
        .map_err(|e| e.to_string())?;
    Ok((
        new_token.access_token.clone(),
        new_token.expires_at,
        Some(new_token),
    ))
}

pub fn save_window_state(position: Point, size: Size, dark_mode: bool) {
    let state = WindowState {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        dark_mode,
    };
    let _ = state.save();
}