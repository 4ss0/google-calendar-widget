//! OAuth 2.0 authorization code flow with PKCE, plus refresh-token storage.
//!
//! Flow:
//!   1. Bind a loopback TcpListener on an ephemeral port.
//!   2. Build the Google consent URL (PKCE S256) and open the browser.
//!   3. Accept a single HTTP request on the loopback address; extract `code`.
//!   4. Exchange `code` for tokens at the token endpoint.
//!   5. Persist the refresh token (DPAPI-encrypted) via save_refresh_token.
//!
//! Only the `calendar.events` scope is requested; `access_type=offline` plus
//! `prompt=consent` are needed to reliably get a refresh_token.

use crate::config::{AppConfig, StoredToken};
use base64::Engine;
use rand::Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events";
const AUTH_TIMEOUT_SECS: u64 = 300;
const CALLBACK_PATH: &str = "/oauth/callback";

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Generates a PKCE verifier (43-128 chars, base62 is safe) and its S256
/// challenge (base64url-encoded SHA-256 of the verifier, unpadded).
fn generate_pkce() -> (String, String) {
    let mut rng = rand::thread_rng();
    let verifier: String = (0..64)
        .map(|_| {
            let c: u8 = rng.gen_range(0..62);
            match c {
                0..=9 => (b'0' + c) as char,
                10..=35 => (b'a' + (c - 10)) as char,
                _ => (b'A' + (c - 36)) as char,
            }
        })
        .collect();
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    (verifier, challenge)
}

/// Full interactive OAuth flow. Opens the browser, waits for the loopback
/// callback (up to AUTH_TIMEOUT_SECS), and exchanges the code for tokens.
pub async fn run_full_auth_flow(
    client_id: &str,
    client_secret: &str,
) -> anyhow::Result<StoredToken> {
    crate::log::write("auth: starting full auth flow");

    // Port 0 => OS picks an ephemeral port. Google desktop clients allow any
    // loopback port as redirect_uri.
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{}{}", port, CALLBACK_PATH);
    crate::log::write(&format!("auth: redirect_uri = {}", redirect_uri));

    let (verifier, challenge) = generate_pkce();

    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        AUTH_URL,
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(SCOPE),
        urlencoding::encode(&challenge),
    );

    if let Err(e) = open::that(&auth_url) {
        crate::log::write(&format!("auth: failed to open browser: {}", e));
        anyhow::bail!("Failed to open browser: {}", e);
    }

    let deadline = tokio::time::Instant::now()
        + std::time::Duration::from_secs(AUTH_TIMEOUT_SECS);

    let mut code: Option<String> = None;
    let mut error: Option<String> = None;

    // Accept connections until we get the callback (favicon requests etc. may
    // arrive first; those are answered with 404 and ignored).
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            crate::log::write("auth: timeout waiting for callback");
            anyhow::bail!("OAuth timeout: no callback received");
        }

        let (mut stream, _) = tokio::time::timeout(remaining, listener.accept())
            .await
            .map_err(|_| anyhow::anyhow!("OAuth timeout: no callback received"))??;

        let mut buf = vec![0u8; 8192];
        let n = match stream.read(&mut buf).await {
            Ok(n) => n,
            Err(_) => continue,
        };
        let request = String::from_utf8_lossy(&buf[..n]);
        let first_line = request.lines().next().unwrap_or("");
        let path = first_line.split_whitespace().nth(1).unwrap_or("");

        if !path.starts_with(CALLBACK_PATH) {
            let body = "<html><body></body></html>";
            let response = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
            continue;
        }

        // Parse query string manually: only `code` and `error` matter.
        // `application/x-www-form-urlencoded` encodes spaces as '+', so we
        // replace those before percent-decoding.
        let query = path.split('?').nth(1).unwrap_or("");
        for pair in query.split('&') {
            let mut kv = pair.splitn(2, '=');
            let k = kv.next().unwrap_or("");
            let v = kv.next().unwrap_or("");
            let v = urlencoding::decode(&v.replace('+', " "))
                .unwrap_or_default()
                .to_string();
            if k == "code" {
                code = Some(v);
            } else if k == "error" {
                error = Some(v);
            }
        }

        let body = if code.is_some() {
            "<html><body style=\"font-family:sans-serif;text-align:center;padding:40px\"><h2>Authorization complete</h2><p>You can close this window and return to the widget.</p></body></html>"
        } else {
            "<html><body style=\"font-family:sans-serif;text-align:center;padding:40px\"><h2>Authorization error</h2><p>Check the widget for details.</p></body></html>"
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;

        if code.is_some() || error.is_some() {
            break;
        }
    }

    if let Some(e) = error {
        crate::log::write(&format!("auth: error from google: {}", e));
        anyhow::bail!("Authorization error from Google: {}", e);
    }
    let code = code.ok_or_else(|| anyhow::anyhow!("Code not found in redirect"))?;
    crate::log::write("auth: received authorization code");

    // Exchange code for tokens.
    let client = reqwest::Client::new();
    let params = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", &code),
        ("code_verifier", &verifier),
        ("grant_type", "authorization_code"),
        ("redirect_uri", &redirect_uri),
    ];
    let resp = client.post(TOKEN_URL).form(&params).send().await?;
    let text = resp.text().await?;
    let token: TokenResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("Invalid token response: {} - {}", e, text))?;

    if let Some(err) = &token.error {
        crate::log::write(&format!("auth: token error: {}", err));
        anyhow::bail!(
            "{} - {}",
            err,
            token.error_description.clone().unwrap_or_default()
        );
    }

    let access_token = token
        .access_token
        .ok_or_else(|| anyhow::anyhow!("missing access_token"))?;
    let expires_at = chrono::Utc::now().timestamp() + token.expires_in.unwrap_or(3600);
    crate::log::write(&format!(
        "auth: token received, expires_at={}, has_refresh={}",
        expires_at,
        token.refresh_token.is_some()
    ));

    Ok(StoredToken {
        access_token,
        refresh_token: token.refresh_token,
        expires_at,
    })
}

/// Exchanges a refresh token for a fresh access token. Google may return a new
/// refresh token; if not, we keep the old one.
pub async fn refresh_access_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> anyhow::Result<StoredToken> {
    crate::log::write("auth: refreshing access token");

    let client = reqwest::Client::new();
    let params = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    let resp = client.post(TOKEN_URL).form(&params).send().await?;
    let text = resp.text().await?;
    let token: TokenResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("Invalid refresh response: {} - {}", e, text))?;

    if let Some(err) = &token.error {
        crate::log::write(&format!("auth: refresh error: {} - {:?}", err, token.error_description));
        anyhow::bail!(
            "{} - {}",
            err,
            token.error_description.clone().unwrap_or_default()
        );
    }

    let access_token = token
        .access_token
        .ok_or_else(|| anyhow::anyhow!("missing access_token in refresh"))?;
    let expires_at = chrono::Utc::now().timestamp() + token.expires_in.unwrap_or(3600);
    crate::log::write(&format!("auth: refreshed, expires_at={}", expires_at));

    Ok(StoredToken {
        access_token,
        refresh_token: Some(refresh_token.to_string()),
        expires_at,
    })
}

/// Persists the refresh token encrypted with DPAPI.
pub fn save_refresh_token(token: &str) -> anyhow::Result<()> {
    let path = AppConfig::token_path();
    crate::log::write(&format!(
        "save_refresh_token: path={:?} len={}",
        path,
        token.len()
    ));
    match crate::crypto::save_encrypted(&path, token.as_bytes()) {
        Ok(()) => {
            crate::log::write("save_refresh_token: OK");
            Ok(())
        }
        Err(e) => {
            crate::log::write(&format!("save_refresh_token: ERROR {}", e));
            Err(e)
        }
    }
}

/// Loads the refresh token from disk. Tries the encrypted file first, then the
/// legacy plaintext one (migrating it if found). Returns None if neither
/// exists or can be decrypted.
pub fn load_refresh_token() -> Option<String> {
    let primary = AppConfig::token_path();
    crate::log::write(&format!(
        "load_refresh_token: primary path={:?} exists={}",
        primary,
        primary.exists()
    ));

    if let Some(bytes) = crate::crypto::load_encrypted(&primary) {
        crate::log::write(&format!(
            "load_refresh_token: decrypted {} bytes",
            bytes.len()
        ));
        match String::from_utf8(bytes) {
            Ok(s) => {
                let trimmed = s.trim().to_string();
                if !trimmed.is_empty() {
                    crate::log::write(&format!(
                        "load_refresh_token: OK ({} chars)",
                        trimmed.len()
                    ));
                    return Some(trimmed);
                }
                crate::log::write("load_refresh_token: decrypted string is empty");
            }
            Err(e) => {
                crate::log::write(&format!(
                    "load_refresh_token: invalid UTF-8: {}",
                    e
                ));
            }
        }
        // Deliberately do NOT delete the file on failure: the user may have a
        // temporary DPAPI issue (e.g. profile corruption) and we don't want to
        // destroy the only copy of their token.
        crate::log::write(
            "load_refresh_token: NOT deleting file on failure (fix), returning None",
        );
    } else {
        crate::log::write("load_refresh_token: load_encrypted returned None");
    }

    // Legacy plaintext file (pre-DPAPI builds).
    let legacy = AppConfig::legacy_token_path();
    if let Ok(s) = std::fs::read_to_string(&legacy) {
        let trimmed = s.trim().to_string();
        if !trimmed.is_empty() {
            crate::log::write("load_refresh_token: migrated from legacy file");
            let _ = save_refresh_token(&trimmed);
            let _ = std::fs::remove_file(&legacy);
            return Some(trimmed);
        }
    }

    crate::log::write("load_refresh_token: NONE");
    None
}

/// Deletes both the encrypted and legacy token files. Used by "Re-authenticate".
pub fn delete_refresh_token() {
    crate::log::write("delete_refresh_token: called");
    let _ = std::fs::remove_file(AppConfig::token_path());
    let _ = std::fs::remove_file(AppConfig::legacy_token_path());
}