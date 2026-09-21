//! Small helpers that bridge the UI layer with on-disk state:
//!   - `ensure_token`: refreshes the access token if it's about to expire.
//!   - `save_window_state`: writes geometry + theme preference.

use crate::auth::oauth::refresh_access_token;
use crate::config::{StoredToken, WindowState};
use iced::{Point, Size};

/// Returns a valid access token, refreshing it if it expires within 60s.
///
/// Returns `(access_token, expires_at, Option<StoredToken>)` where the last
/// element is present only when a refresh actually happened, so the caller
/// can persist the new refresh_token (Google may rotate it).
pub async fn ensure_token(
    client_id: &str,
    client_secret: &str,
    token: &str,
    expires_at: i64,
    refresh_token: Option<&str>,
) -> Result<(String, i64, Option<StoredToken>), String> {
    let now = chrono::Utc::now().timestamp();
    // 60s safety margin so a slow request doesn't cross the expiry boundary.
    if expires_at - now > 60 {
        return Ok((token.to_string(), expires_at, None));
    }
    let rt = refresh_token
        .map(|s| s.to_string())
        .ok_or_else(|| "Refresh token not found".to_string())?;
    let new_token = refresh_access_token(client_id, client_secret, &rt)
        .await
        .map_err(|e| e.to_string())?;
    Ok((
        new_token.access_token.clone(),
        new_token.expires_at,
        Some(new_token),
    ))
}

/// Persists the window geometry + theme.
///
/// `position` is `Some(p)` when the real on-screen position of the window is
/// known (loaded from disk or observed via a `WindowMoved` event), and `None`
/// otherwise. When `None`, the saved state is marked as
/// `position_is_default = true` and `x`/`y` are zeroed, so the next launch
/// asks the OS to choose the position again instead of pinning the window to
/// the top-left corner.
pub fn save_window_state(position: Option<Point>, size: Size, dark_mode: bool) {
    let (x, y, position_is_default) = match position {
        Some(p) => (p.x, p.y, false),
        None => (0.0, 0.0, true),
    };
    let state = WindowState {
        x,
        y,
        width: size.width,
        height: size.height,
        dark_mode,
        position_is_default,
    };
    let _ = state.save();
}