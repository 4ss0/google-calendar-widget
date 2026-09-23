//! Google Calendar REST v3 client.
//!
//! Responsibilities:
//!   - Fetch events in a time range (paginated, singleEvents=true so recurring
//!     events are expanded into individual instances).
//!   - Create / update / delete single events.
//!   - Update / delete a recurring series, including the "this and following"
//!     case which is implemented by truncating the master with UNTIL and
//!     creating a brand-new series.
//!
//! Google returns "dateTime" (RFC3339) for timed events and "date" (YYYY-MM-DD)
//! for all-day events. `parse_datetime` normalizes both to a UTC DateTime plus
//! an `all_day` flag. This is important because all-day end dates are
//! *exclusive* in the API (an all-day event on 21 Sep has end 22 Sep).
//!
//! All requests go through a shared `reqwest::Client` (connection pooling,
//! timeouts, user-agent) created lazily with OnceLock.

use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const BASE: &str = "https://www.googleapis.com/calendar/v3";
const USER_AGENT: &str = "google-calendar-widget/0.1.0";

static HTTP: OnceLock<reqwest::Client> = OnceLock::new();

/// Returns the shared HTTP client, initializing it on first call.
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

/// Returns the IANA name of the local timezone (e.g. "Europe/Rome"), or "UTC"
/// if it cannot be determined. Sent as `timeZone` for timed events so Google
/// stores the correct wall-clock time regardless of DST transitions.
fn local_timezone_name() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

/// Wire format for the start/end of an event. Exactly one of the two variants
/// is populated depending on whether the event is all-day or timed.
#[derive(Debug, Deserialize, Clone)]
struct EventDateTime {
    #[serde(rename = "dateTime")]
    date_time: Option<String>,
    date: Option<String>,
}

/// Raw event as returned by the API. Fields not needed by the widget are
/// dropped; serde ignores unknown fields by default.
#[derive(Debug, Deserialize, Clone)]
struct GoogleEvent {
    id: String,
    summary: Option<String>,
    start: EventDateTime,
    end: Option<EventDateTime>,
    #[serde(rename = "colorId")]
    color_id: Option<String>,
    /// Present on instances of a recurring series; identifies the master.
    #[serde(rename = "recurringEventId")]
    recurring_event_id: Option<String>,
    /// Original scheduled start of an instance (may differ from `start` if the
    /// instance was moved).
    #[serde(rename = "originalStartTime")]
    original_start_time: Option<EventDateTime>,
    /// RRULE lines (only present on the master of a series).
    recurrence: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct EventsResponse {
    items: Option<Vec<GoogleEvent>>,
    /// Pagination cursor; absent on the last page.
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

/// Normalized event used by the widget. All-day events still carry a UTC
/// DateTime (midnight local), plus `all_day = true`.
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
    /// Propagated from the master so that Undo can recreate a recurring event.
    pub recurrence: Option<Vec<String>>,
}

/// Data for creating or updating an event.
///
/// Grouping these fields into a struct keeps the API functions under clippy's
/// argument-count threshold and, more importantly, prevents call sites from
/// accidentally mixing up positional arguments (e.g. `start`/`end` or
/// `color_id`/`all_day`).
///
/// `recurrence` is only meaningful for `create_event` (and internally for the
/// tail of a "this and following" split); update functions ignore it.
pub struct EventInput<'a> {
    pub summary: &'a str,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub color_id: Option<&'a str>,
    pub all_day: bool,
    pub recurrence: Option<Vec<String>>,
}

impl<'a> EventInput<'a> {
    /// Convenience constructor for the common case where there is no RRULE.
    pub fn new(
        summary: &'a str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        color_id: Option<&'a str>,
        all_day: bool,
    ) -> Self {
        Self {
            summary,
            start,
            end,
            color_id,
            all_day,
            recurrence: None,
        }
    }
}

/// Serialized start/end for write operations. Untagged so the JSON shape
/// matches Google's expected schema (either {dateTime, timeZone} or {date}).
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
    /// Only sent on create of a recurring event; updates use PATCH with the
    /// dedicated RecurrencePatchBody.
    #[serde(skip_serializing_if = "Option::is_none")]
    recurrence: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct RecurrencePatchBody {
    recurrence: Vec<String>,
}

/// Converts a wire EventDateTime to (UTC DateTime, all_day).
/// `earliest()` handles fall-back DST by picking the first of two identical
/// local times. For the rare DST transition that skips midnight entirely
/// (spring-forward at 00:00 local) we fall back to 01:00 local rather than
/// discarding the event.
fn parse_datetime(dt: &EventDateTime) -> Option<(DateTime<Utc>, bool)> {
    if let Some(s) = &dt.date_time {
        let d = DateTime::parse_from_rfc3339(s).ok()?.with_timezone(&Utc);
        return Some((d, false));
    }
    if let Some(s) = &dt.date {
        let nd = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
        let naive = nd.and_hms_opt(0, 0, 0)?;
        // Midnight may not exist during a DST transition in some timezones.
        // Try midnight first, then 01:00, then 02:00 local.
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

/// Serializes a start/end pair for the API, choosing the DATE variant for
/// all-day events and DATE-TIME otherwise.
fn datetime_body(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    all_day: bool,
) -> (EventDateTimeBody, EventDateTimeBody) {
    if all_day {
        // Convert back to the local calendar date; the API expects YYYY-MM-DD.
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

/// Extracts the UNTIL component of an RRULE, if present, as a full
/// "UNTIL=..." string. Returning the raw string preserves the original
/// formatting (DATE vs DATE-TIME), which is required by Google: all-day
/// events need a DATE, timed events a UTC DATE-TIME.
fn extract_until(rrule: &str) -> Option<String> {
    rrule
        .split(';')
        .find(|p| p.starts_with("UNTIL="))
        .map(|s| s.to_string())
}

/// Rewrites an RRULE adding/replacing UNTIL.
///
/// For timed events UNTIL must be a UTC DATE-TIME (`YYYYMMDDTHHMMSSZ`).
/// For all-day events UNTIL must be a DATE (`YYYYMMDD`). Getting this wrong
/// makes Google reject the request with HTTP 400.
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

/// Removes UNTIL from an RRULE. Used when splitting a series to create the
/// "new" tail. Note that the caller must re-attach the *original* UNTIL (if
/// any) so that a bounded series does not silently become infinite.
fn rrule_without_until(rrule: &str) -> String {
    let parts: Vec<&str> = rrule.split(';').collect();
    let filtered: Vec<&str> = parts
        .into_iter()
        .filter(|p| !p.starts_with("UNTIL="))
        .collect();
    filtered.join(";")
}

/// Fetches a raw event by id (used to read the master of a recurring series).
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

/// Fetches all events in [time_min, time_max), following pagination.
///
/// `singleEvents=true` makes the API expand recurring events into individual
/// instances within the window, which is what the widget needs for rendering.
/// `orderBy=startTime` requires singleEvents=true.
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
        // Safety net: 20 pages * 2500 = 50k events. Enough for any real use.
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

        let resp = http()
            .get(&url)
            .bearer_auth(access_token)
            .query(&query)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error {}: {}", status, body);
        }

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
    input: EventInput<'_>,
) -> anyhow::Result<CalendarEvent> {
    let url = format!(
        "{}/calendars/{}/events",
        BASE,
        urlencoding::encode(calendar_id)
    );
    let (start_body, end_body) = datetime_body(input.start, input.end, input.all_day);
    let body = EventBody {
        summary: input.summary.to_string(),
        start: start_body,
        end: end_body,
        color_id: input.color_id.map(|s| s.to_string()),
        recurrence: input.recurrence,
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

/// PATCH a single instance (or a non-recurring event). `recurrence` is
/// intentionally omitted: patching RRULE requires a dedicated body and is not
/// part of single-instance edits.
pub async fn update_event(
    access_token: &str,
    calendar_id: &str,
    event_id: &str,
    input: EventInput<'_>,
) -> anyhow::Result<CalendarEvent> {
    let url = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(event_id)
    );
    let (start_body, end_body) = datetime_body(input.start, input.end, input.all_day);
    let body = EventBody {
        summary: input.summary.to_string(),
        start: start_body,
        end: end_body,
        color_id: input.color_id.map(|s| s.to_string()),
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

/// Updates the whole recurring series ("All events" scope).
///
/// The master's start is shifted by the same delta the user applied to the
/// instance, so relative timing across the series is preserved. The master's
/// duration is set to the duration the user currently has on the instance, so
/// changes to the end time (or to the length) are applied to the whole series
/// rather than being silently dropped.
pub async fn update_event_all_occurrences(
    access_token: &str,
    calendar_id: &str,
    base_event_id: &str,
    original_start: DateTime<Utc>,
    input: EventInput<'_>,
) -> anyhow::Result<CalendarEvent> {
    let base = get_event_raw(access_token, calendar_id, base_event_id).await?;

    let (base_start, _) = parse_datetime(&base.start)
        .ok_or_else(|| anyhow::anyhow!("Base event has no valid start"))?;

    // delta = "how much later/earlier the user moved this instance"
    let delta = input.start - original_start;
    let new_master_start = base_start + delta;
    // Use the *edited instance's* duration so end-time changes propagate.
    let new_duration = input.end - input.start;
    let new_master_end = new_master_start + new_duration;

    let url = format!(
        "{}/calendars/{}/events/{}",
        BASE,
        urlencoding::encode(calendar_id),
        urlencoding::encode(base_event_id)
    );
    let (sb, eb) = datetime_body(new_master_start, new_master_end, input.all_day);
    let body = EventBody {
        summary: input.summary.to_string(),
        start: sb,
        end: eb,
        color_id: input.color_id.map(|s| s.to_string()),
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

/// Splits a recurring series at `original_start` ("This and following"):
///   1. PATCH the master with UNTIL = original_start - 1s.
///   2. POST a new master with the original RRULE (minus UNTIL) and the new
///      start/end/summary.
///
/// If the original series was bounded by an UNTIL, the new tail must keep
/// that same UNTIL so it remains bounded exactly like the original. Otherwise
/// (e.g. `FREQ=WEEKLY;UNTIL=20261231T235959Z` split in June), the new series
/// would silently become infinite.
///
/// If the instance is the first occurrence (base_start >= original_start),
/// this degrades to a simple PATCH of the master.
pub async fn update_event_this_and_following(
    access_token: &str,
    calendar_id: &str,
    base_event_id: &str,
    original_start: DateTime<Utc>,
    input: EventInput<'_>,
) -> anyhow::Result<CalendarEvent> {
    let base = get_event_raw(access_token, calendar_id, base_event_id).await?;

    let rrule = base
        .recurrence
        .as_ref()
        .and_then(|v| v.first().cloned())
        .ok_or_else(|| anyhow::anyhow!("Event is not recurring"))?;

    let (base_start, _) = parse_datetime(&base.start)
        .ok_or_else(|| anyhow::anyhow!("Base event has no valid start"))?;

    // Splitting at the very first occurrence means the whole series moves.
    if base_start >= original_start {
        return update_event(
            access_token,
            calendar_id,
            base_event_id,
            EventInput {
                summary: input.summary,
                start: input.start,
                end: input.end,
                color_id: input.color_id,
                all_day: input.all_day,
                recurrence: None,
            },
        )
        .await;
    }

    // Cut just before the edited instance.
    let split_until = original_start - Duration::seconds(1);
    let truncated = rrule_with_until(&rrule, split_until, input.all_day);

    // Preserve the original UNTIL (if any) on the new series: a bounded
    // series must remain bounded after a split, otherwise the tail would
    // extend forever.
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
    let (sb2, eb2) = datetime_body(input.start, input.end, input.all_day);
    let create_body = EventBody {
        summary: input.summary.to_string(),
        start: sb2,
        end: eb2,
        color_id: input.color_id.map(|s| s.to_string()),
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

/// Deletes "this and following". Only PATCHes the master with a truncated
/// RRULE; if the split point is the first occurrence, deletes the whole series.
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