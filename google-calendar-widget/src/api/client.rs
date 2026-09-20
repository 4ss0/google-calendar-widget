use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const BASE: &str = "https://www.googleapis.com/calendar/v3";

static HTTP: OnceLock<reqwest::Client> = OnceLock::new();

fn http() -> &'static reqwest::Client {
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(5))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .expect("client http")
    })
}

#[derive(Debug, Deserialize, Clone)]
struct EventDateTime {
    #[serde(rename = "dateTime")]
    date_time: Option<String>,
    date: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct GoogleEvent {
    id: String,
    summary: Option<String>,
    start: EventDateTime,
    end: Option<EventDateTime>,
    #[serde(rename = "colorId")]
    color_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EventsResponse {
    items: Option<Vec<GoogleEvent>>,
}

#[derive(Debug, Clone)]
pub struct CalendarEvent {
    pub id: String,
    pub summary: String,
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
    pub color_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct EventDateTimeBody {
    #[serde(rename = "dateTime")]
    date_time: String,
}

#[derive(Debug, Serialize)]
struct EventBody {
    summary: String,
    start: EventDateTimeBody,
    end: EventDateTimeBody,
    #[serde(rename = "colorId", skip_serializing_if = "Option::is_none")]
    color_id: Option<String>,
}

fn parse_datetime(dt: &EventDateTime) -> Option<DateTime<Utc>> {
    if let Some(s) = &dt.date_time {
        return DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.with_timezone(&Utc));
    }
    if let Some(d) = &dt.date {
        let nd = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()?;
        return Some(Utc.from_utc_datetime(&nd.and_hms_opt(0, 0, 0)?));
    }
    None
}

fn convert(e: GoogleEvent) -> Option<CalendarEvent> {
    let start = parse_datetime(&e.start)?;
    let end = e.end.as_ref().and_then(parse_datetime);
    Some(CalendarEvent {
        id: e.id,
        summary: e.summary.unwrap_or_else(|| "(senza titolo)".into()),
        start,
        end,
        color_id: e.color_id,
    })
}

pub async fn fetch_events(
    access_token: &str,
    calendar_id: &str,
    time_min: DateTime<Utc>,
    time_max: DateTime<Utc>,
) -> anyhow::Result<Vec<CalendarEvent>> {
    let url = format!(
        "{}/calendars/{}/events",
        BASE,
        urlencoding::encode(calendar_id)
    );

    let resp = http()
        .get(&url)
        .bearer_auth(access_token)
        .query(&[
            ("timeMin", time_min.to_rfc3339()),
            ("timeMax", time_max.to_rfc3339()),
            ("singleEvents", "true".to_string()),
            ("orderBy", "startTime".to_string()),
            ("maxResults", "2500".to_string()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error {}: {}", status, body);
    }

    let body: EventsResponse = resp.json().await?;
    let items = body.items.unwrap_or_default();
    let events = items.into_iter().filter_map(convert).collect();
    Ok(events)
}

pub async fn create_event(
    access_token: &str,
    calendar_id: &str,
    summary: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    color_id: Option<&str>,
) -> anyhow::Result<CalendarEvent> {
    let url = format!(
        "{}/calendars/{}/events",
        BASE,
        urlencoding::encode(calendar_id)
    );
    let body = EventBody {
        summary: summary.to_string(),
        start: EventDateTimeBody {
            date_time: start.to_rfc3339(),
        },
        end: EventDateTimeBody {
            date_time: end.to_rfc3339(),
        },
        color_id: color_id.map(|s| s.to_string()),
    };
    let resp = http()
        .post(&url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error {}: {}", status, body);
    }
    let event: GoogleEvent = resp.json().await?;
    convert(event).ok_or_else(|| anyhow::anyhow!("Evento restituito non valido"))
}

pub async fn update_event(
    access_token: &str,
    calendar_id: &str,
    event_id: &str,
    summary: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    color_id: Option<&str>,
) -> anyhow::Result<CalendarEvent> {
    let url = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(event_id)
    );
    let body = EventBody {
        summary: summary.to_string(),
        start: EventDateTimeBody {
            date_time: start.to_rfc3339(),
        },
        end: EventDateTimeBody {
            date_time: end.to_rfc3339(),
        },
        color_id: color_id.map(|s| s.to_string()),
    };
    let resp = http()
        .put(&url)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error {}: {}", status, body);
    }
    let event: GoogleEvent = resp.json().await?;
    convert(event).ok_or_else(|| anyhow::anyhow!("Evento restituito non valido"))
}

pub async fn delete_event(
    access_token: &str,
    calendar_id: &str,
    event_id: &str,
) -> anyhow::Result<()> {
    let url = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(event_id)
    );
    let resp = http()
        .delete(&url)
        .bearer_auth(access_token)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error {}: {}", status, body);
    }
    Ok(())
}