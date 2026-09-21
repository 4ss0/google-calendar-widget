use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const BASE: &str = "https://www.googleapis.com/calendar/v3";
const USER_AGENT: &str = "google-calendar-widget/0.1.0";

static HTTP: OnceLock<reqwest::Client> = OnceLock::new();

fn http() -> &'static reqwest::Client {
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(5))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .expect("client http")
    })
}

fn local_timezone_name() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
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
    #[serde(rename = "recurringEventId")]
    recurring_event_id: Option<String>,
    #[serde(rename = "originalStartTime")]
    original_start_time: Option<EventDateTime>,
    recurrence: Option<Vec<String>>,
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
    pub all_day: bool,
    pub recurring_event_id: Option<String>,
    pub original_start_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum EventDateTimeBody {
    DateTime {
        #[serde(rename = "dateTime")]
        date_time: String,
        #[serde(rename = "timeZone")]
        time_zone: String,
    },
    Date {
        date: String,
    },
}

#[derive(Debug, Serialize)]
struct EventBody {
    summary: String,
    start: EventDateTimeBody,
    end: EventDateTimeBody,
    #[serde(rename = "colorId", skip_serializing_if = "Option::is_none")]
    color_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recurrence: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct RecurrencePatchBody {
    recurrence: Vec<String>,
}

fn parse_datetime(dt: &EventDateTime) -> Option<(DateTime<Utc>, bool)> {
    if let Some(s) = &dt.date_time {
        let d = DateTime::parse_from_rfc3339(s).ok()?.with_timezone(&Utc);
        return Some((d, false));
    }
    if let Some(s) = &dt.date {
        let nd = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
        let naive = nd.and_hms_opt(0, 0, 0)?;
        let local = chrono::Local.from_local_datetime(&naive).earliest()?;
        return Some((local.with_timezone(&Utc), true));
    }
    None
}

fn convert(e: GoogleEvent) -> Option<CalendarEvent> {
    let (start, all_day) = parse_datetime(&e.start)?;
    let end = e
        .end
        .as_ref()
        .and_then(|edt| parse_datetime(edt).map(|(d, _)| d));
    let original_start_time = e
        .original_start_time
        .as_ref()
        .and_then(|edt| parse_datetime(edt).map(|(d, _)| d));
    Some(CalendarEvent {
        id: e.id,
        summary: e.summary.unwrap_or_else(|| "(no title)".into()),
        start,
        end,
        color_id: e.color_id,
        all_day,
        recurring_event_id: e.recurring_event_id,
        original_start_time,
    })
}

fn datetime_body(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    all_day: bool,
) -> (EventDateTimeBody, EventDateTimeBody) {
    if all_day {
        let s = start
            .with_timezone(&chrono::Local)
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let e = end
            .with_timezone(&chrono::Local)
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        (EventDateTimeBody::Date { date: s }, EventDateTimeBody::Date { date: e })
    } else {
        let tz = local_timezone_name();
        (
            EventDateTimeBody::DateTime {
                date_time: start.to_rfc3339(),
                time_zone: tz.clone(),
            },
            EventDateTimeBody::DateTime {
                date_time: end.to_rfc3339(),
                time_zone: tz,
            },
        )
    }
}

fn rrule_with_until(rrule: &str, until: DateTime<Utc>) -> String {
    let parts: Vec<&str> = rrule.split(';').collect();
    let mut filtered: Vec<String> = parts
        .into_iter()
        .filter(|p| !p.starts_with("UNTIL="))
        .map(|s| s.to_string())
        .collect();
    filtered.push(format!("UNTIL={}", until.format("%Y%m%dT%H%M%SZ")));
    filtered.join(";")
}

fn rrule_without_until(rrule: &str) -> String {
    let parts: Vec<&str> = rrule.split(';').collect();
    let filtered: Vec<&str> = parts
        .into_iter()
        .filter(|p| !p.starts_with("UNTIL="))
        .collect();
    filtered.join(";")
}

async fn get_event_raw(
    access_token: &str,
    calendar_id: &str,
    event_id: &str,
) -> anyhow::Result<GoogleEvent> {
    let url = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(event_id)
    );
    let resp = http()
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error {}: {}", status, body);
    }
    Ok(resp.json().await?)
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
    all_day: bool,
    recurrence: Option<Vec<String>>,
) -> anyhow::Result<CalendarEvent> {
    let url = format!(
        "{}/calendars/{}/events",
        BASE,
        urlencoding::encode(calendar_id)
    );
    let (start_body, end_body) = datetime_body(start, end, all_day);
    let body = EventBody {
        summary: summary.to_string(),
        start: start_body,
        end: end_body,
        color_id: color_id.map(|s| s.to_string()),
        recurrence,
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
    convert(event).ok_or_else(|| anyhow::anyhow!("Invalid event returned"))
}

pub async fn update_event(
    access_token: &str,
    calendar_id: &str,
    event_id: &str,
    summary: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    color_id: Option<&str>,
    all_day: bool,
) -> anyhow::Result<CalendarEvent> {
    let url = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(event_id)
    );
    let (start_body, end_body) = datetime_body(start, end, all_day);
    let body = EventBody {
        summary: summary.to_string(),
        start: start_body,
        end: end_body,
        color_id: color_id.map(|s| s.to_string()),
        recurrence: None,
    };
    let resp = http()
        .patch(&url)
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
    convert(event).ok_or_else(|| anyhow::anyhow!("Invalid event returned"))
}

pub async fn update_event_this_and_following(
    access_token: &str,
    calendar_id: &str,
    base_event_id: &str,
    original_start: DateTime<Utc>,
    summary: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    color_id: Option<&str>,
    all_day: bool,
) -> anyhow::Result<CalendarEvent> {
    let base = get_event_raw(access_token, calendar_id, base_event_id).await?;

    let rrule = base
        .recurrence
        .as_ref()
        .and_then(|v| v.first().cloned())
        .ok_or_else(|| anyhow::anyhow!("Event is not recurring"))?;

    let (base_start, _) = parse_datetime(&base.start)
        .ok_or_else(|| anyhow::anyhow!("Base event has no valid start"))?;

    if base_start >= original_start {
        return update_event(
            access_token,
            calendar_id,
            base_event_id,
            summary,
            start,
            end,
            color_id,
            all_day,
        )
        .await;
    }

    let split_until = original_start - Duration::seconds(1);
    let truncated = rrule_with_until(&rrule, split_until);
    let new_series_rrule = rrule_without_until(&rrule);

    let url_base = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(base_event_id)
    );
    let (sb, eb) = datetime_body(start, end, all_day);
    let patch_body = EventBody {
        summary: summary.to_string(),
        start: sb,
        end: eb,
        color_id: color_id.map(|s| s.to_string()),
        recurrence: Some(vec![truncated]),
    };
    let resp = http()
        .patch(&url_base)
        .bearer_auth(access_token)
        .json(&patch_body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error (truncate) {}: {}", status, body);
    }

    let url_create = format!(
        "{}/calendars/{}/events",
        BASE,
        urlencoding::encode(calendar_id)
    );
    let (sb2, eb2) = datetime_body(start, end, all_day);
    let create_body = EventBody {
        summary: summary.to_string(),
        start: sb2,
        end: eb2,
        color_id: color_id.map(|s| s.to_string()),
        recurrence: Some(vec![new_series_rrule]),
    };
    let resp = http()
        .post(&url_create)
        .bearer_auth(access_token)
        .json(&create_body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error (new series) {}: {}", status, body);
    }
    let event: GoogleEvent = resp.json().await?;
    convert(event).ok_or_else(|| anyhow::anyhow!("Invalid event returned"))
}

pub async fn delete_event_this_and_following(
    access_token: &str,
    calendar_id: &str,
    base_event_id: &str,
    original_start: DateTime<Utc>,
) -> anyhow::Result<()> {
    let base = get_event_raw(access_token, calendar_id, base_event_id).await?;

    let rrule = base
        .recurrence
        .as_ref()
        .and_then(|v| v.first().cloned())
        .ok_or_else(|| anyhow::anyhow!("Event is not recurring"))?;

    let (base_start, _) = parse_datetime(&base.start)
        .ok_or_else(|| anyhow::anyhow!("Base event has no valid start"))?;

    if base_start >= original_start {
        return delete_event(access_token, calendar_id, base_event_id).await;
    }

    let split_until = original_start - Duration::seconds(1);
    let truncated = rrule_with_until(&rrule, split_until);

    let url_base = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(base_event_id)
    );
    let body = RecurrencePatchBody {
        recurrence: vec![truncated],
    };
    let resp = http()
        .patch(&url_base)
        .bearer_auth(access_token)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("API error {}: {}", status, body);
    }
    Ok(())
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