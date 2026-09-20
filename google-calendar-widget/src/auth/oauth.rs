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

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    error: Option<String>,
    error_description: Option<String>,
}

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

pub async fn run_full_auth_flow(
    client_id: &str,
    client_secret: &str,
) -> anyhow::Result<StoredToken> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{}/oauth/callback", port);

    let (verifier, challenge) = generate_pkce();

    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        AUTH_URL,
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(SCOPE),
        urlencoding::encode(&challenge),
    );

    open::that(&auth_url)?;

    let (mut stream, _) = listener.accept().await?;
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);

    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");
    let query = path.split('?').nth(1).unwrap_or("");
    let mut code: Option<String> = None;
    let mut error: Option<String> = None;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let k = kv.next().unwrap_or("");
        let v = kv.next().unwrap_or("");
        let v = urlencoding::decode(v).unwrap_or_default().to_string();
        if k == "code" {
            code = Some(v);
        } else if k == "error" {
            error = Some(v);
        }
    }

    let body = if code.is_some() {
        "<html><body style=\"font-family:sans-serif;text-align:center;padding:40px\"><h2>Autorizzazione completata</h2><p>Puoi chiudere questa finestra e tornare al widget.</p></body></html>"
    } else {
        "<html><body style=\"font-family:sans-serif;text-align:center;padding:40px\"><h2>Errore di autorizzazione</h2><p>Controlla il widget per i dettagli.</p></body></html>"
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;

    if let Some(e) = error {
        anyhow::bail!("Errore autorizzazione da Google: {}", e);
    }
    let code = code.ok_or_else(|| anyhow::anyhow!("Codice non trovato nel redirect"))?;

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
        .map_err(|e| anyhow::anyhow!("Risposta token non valida: {} - {}", e, text))?;

    if let Some(err) = &token.error {
        anyhow::bail!(
            "{} - {}",
            err,
            token.error_description.clone().unwrap_or_default()
        );
    }

    let access_token = token
        .access_token
        .ok_or_else(|| anyhow::anyhow!("access_token mancante"))?;
    let expires_at = chrono::Utc::now().timestamp() + token.expires_in.unwrap_or(3600);

    Ok(StoredToken {
        access_token,
        refresh_token: token.refresh_token,
        expires_at,
    })
}

pub async fn refresh_access_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> anyhow::Result<StoredToken> {
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
        .map_err(|e| anyhow::anyhow!("Risposta refresh non valida: {} - {}", e, text))?;

    if let Some(err) = &token.error {
        anyhow::bail!(
            "{} - {}",
            err,
            token.error_description.clone().unwrap_or_default()
        );
    }

    let access_token = token
        .access_token
        .ok_or_else(|| anyhow::anyhow!("access_token mancante nel refresh"))?;
    let expires_at = chrono::Utc::now().timestamp() + token.expires_in.unwrap_or(3600);

    Ok(StoredToken {
        access_token,
        refresh_token: Some(refresh_token.to_string()),
        expires_at,
    })
}

pub fn save_refresh_token(token: &str) -> anyhow::Result<()> {
    let path = AppConfig::token_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, token)?;
    Ok(())
}

pub fn load_refresh_token() -> Option<String> {
    std::fs::read_to_string(AppConfig::token_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn delete_refresh_token() {
    let path = AppConfig::token_path();
    let _ = std::fs::remove_file(path);
}