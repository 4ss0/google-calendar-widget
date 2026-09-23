//! Google Calendar REST v3 client.

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

/// Consumes a response, returning it on 2xx or an error containing status
/// and body on failure. Centralizes the repeated error-handling pattern.
async fn check(resp: reqwest::Response) -> anyhow::Result<reqwest::Response> {
    check_ctx(resp, "").await
}

/// Same as `check` but tags the error with a context label (e.g. "truncate")
/// so multi-step operations can distinguish which call failed.
async fn check_ctx(
    resp: reqwest::Response,
    ctx: &str,
) -> anyhow::Result<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if ctx.is_empty() {
        anyhow::bail!("API error {}: {}", status, body);
    }
    anyhow::bail!("API error ({}) {}: {}", ctx, status, body);
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
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
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
    pub recurrence: Option<Vec<String>>,
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
        // Midnight may not exist during a DST spring-forward; try 00/01/02.
        let local = chrono::Local
            .from_local_datetime(&naive)
            .earliest()
            .or_else(|| {
                let alt = naive + Duration::hours(1);
                chrono::Local.from_local_datetime(&alt).earliest()
            })
            .or_else(|| {
                let alt = naive + Duration::hours(2);
                chrono::Local.from_local_datetime(&alt).earliest()
            })?;
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
        recurrence: e.recurrence,
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

fn extract_until(rrule: &str) -> Option<String> {
    rrule
        .split(';')
        .find(|p| p.starts_with("UNTIL="))
        .map(|s| s.to_string())
}

fn rrule_with_until(rrule: &str, until: DateTime<Utc>, all_day: bool) -> String {
    let parts: Vec<&str> = rrule.split(';').collect();
    let mut filtered: Vec<String> = parts
        .into_iter()
        .filter(|p| !p.starts_with("UNTIL="))
        .map(|s| s.to_string())
        .collect();
    if all_day {
        let local = until.with_timezone(&chrono::Local);
        filtered.push(format!("UNTIL={}", local.format("%Y%m%d")));
    } else {
        filtered.push(format!("UNTIL={}", until.format("%Y%m%dT%H%M%SZ")));
    }
    filtered.join(";")
}

fn rrule_without_until(rrule: &str) -> String {
    rrule
        .split(';')
        .filter(|p| !p.starts_with("UNTIL="))
        .collect::<Vec<_>>()
        .join(";")
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
    let resp = check(http().get(&url).bearer_auth(access_token).send().await?).await?;
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

    let mut all_events: Vec<CalendarEvent> = Vec::new();
    let mut page_token: Option<String> = None;
    let mut page_count: u32 = 0;

    loop {
        page_count += 1;
        if page_count > 20 {
            crate::log::write("fetch_events: hit page limit (20), stopping");
            break;
        }

        let mut query: Vec<(String, String)> = vec![
            ("timeMin".to_string(), time_min.to_rfc3339()),
            ("timeMax".to_string(), time_max.to_rfc3339()),
            ("singleEvents".to_string(), "true".to_string()),
            ("orderBy".to_string(), "startTime".to_string()),
            ("maxResults".to_string(), "2500".to_string()),
        ];
        if let Some(t) = &page_token {
            query.push(("pageToken".to_string(), t.clone()));
        }

        let resp = check(
            http()
                .get(&url)
                .bearer_auth(access_token)
                .query(&query)
                .send()
                .await?,
        )
        .await?;

        let body: EventsResponse = resp.json().await?;
        if let Some(items) = body.items {
            all_events.extend(items.into_iter().filter_map(convert));
        }

        match body.next_page_token {
            Some(t) if !t.is_empty() => page_token = Some(t),
            _ => break,
        }
    }

    crate::log::write(&format!(
        "fetch_events: {} events across {} page(s)",
        all_events.len(),
        page_count
    ));

    Ok(all_events)
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
    let resp = check(
        http()
            .post(&url)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await?,
    )
    .await?;
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
    let resp = check(
        http()
            .patch(&url)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await?,
    )
    .await?;
    let event: GoogleEvent = resp.json().await?;
    convert(event).ok_or_else(|| anyhow::anyhow!("Invalid event returned"))
}

pub async fn update_event_all_occurrences(
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

    let (base_start, _) = parse_datetime(&base.start)
        .ok_or_else(|| anyhow::anyhow!("Base event has no valid start"))?;

    let delta = start - original_start;
    let new_master_start = base_start + delta;
    let new_duration = end - start;
    let new_master_end = new_master_start + new_duration;

    let url = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(base_event_id)
    );
    let (sb, eb) = datetime_body(new_master_start, new_master_end, all_day);
    let body = EventBody {
        summary: summary.to_string(),
        start: sb,
        end: eb,
        color_id: color_id.map(|s| s.to_string()),
        recurrence: None,
    };
    let resp = check(
        http()
            .patch(&url)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await?,
    )
    .await?;
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
    let truncated = rrule_with_until(&rrule, split_until, all_day);

    let original_until = extract_until(&rrule);
    let base_rrule = rrule_without_until(&rrule);
    let new_series_rrule = match original_until {
        Some(until) => format!("{};{}", base_rrule, until),
        None => base_rrule,
    };

    let url_base = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(base_event_id)
    );
    let patch_body = RecurrencePatchBody {
        recurrence: vec![truncated],
    };
    check_ctx(
        http()
            .patch(&url_base)
            .bearer_auth(access_token)
            .json(&patch_body)
            .send()
            .await?,
        "truncate",
    )
    .await?;

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
    let resp = check_ctx(
        http()
            .post(&url_create)
            .bearer_auth(access_token)
            .json(&create_body)
            .send()
            .await?,
        "new series",
    )
    .await?;
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

    let (base_start, all_day) = parse_datetime(&base.start)
        .ok_or_else(|| anyhow::anyhow!("Base event has no valid start"))?;

    if base_start >= original_start {
        return delete_event(access_token, calendar_id, base_event_id).await;
    }

    let split_until = original_start - Duration::seconds(1);
    let truncated = rrule_with_until(&rrule, split_until, all_day);

    let url_base = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(base_event_id)
    );
    let body = RecurrencePatchBody {
        recurrence: vec![truncated],
    };
    check(
        http()
            .patch(&url_base)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await?,
    )
    .await?;
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
    check(http().delete(&url).bearer_auth(access_token).send().await?).await?;
    Ok(())
}