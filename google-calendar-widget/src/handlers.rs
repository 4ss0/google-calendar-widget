//! Business logic for user actions that require network I/O.
//!
//! Each `handle_*` method:
//!   1. Validates the form state (early-returns with an error message on
//!      invalid input).
//!   2. Builds an async task that either refreshes the token (via
//!      `run_with_token`) or performs the API call.
//!   3. Returns a `Command` that will eventually produce a `*Completed`
//!      message consumed by update.rs.
//!
//! `visible_range` computes the API time window for the current layout;
//! `local_to_utc` handles the DST edge cases when converting naive local
//! datetimes to UTC.

use crate::api::client::EventInput;
use crate::app::{ApiResult, App, DragState, EditScope, FormMode};
use crate::messages::Message;
use crate::persistence;
use crate::ui;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use iced::Command;

impl App {
    /// Stores a refreshed token (and possibly a new refresh token) in memory
    /// and on disk.
    pub fn apply_new_token(&mut self, new_token: Option<crate::config::StoredToken>) {
        if let Some(t) = new_token {
            self.access_token = Some(t.access_token.clone());
            self.expires_at = t.expires_at;
            if let Some(rt) = t.refresh_token {
                let _ = crate::auth::oauth::save_refresh_token(&rt);
                self.refresh_token = Some(rt);
            }
        }
    }

    /// Persists the current window geometry and theme. If the real on-screen
    /// position is not yet known (first run, window still at the OS-chosen
    /// default position), passes None so the saved file preserves the
    /// "use OS default" flag instead of baking in a placeholder (0,0).
    pub fn persist_geometry(&self) {
        let pos = if self.window_position_known {
            Some(self.window_position)
        } else {
            None
        };
        persistence::save_window_state(pos, self.window_size, self.theme.is_dark);
    }

    /// If the app was started with --minimized, restores the window.
    pub fn reveal_window(&mut self) -> Command<Message> {
        if self.started_minimized {
            self.started_minimized = false;
            Self::show_window_with_effects()
        } else {
            Command::none()
        }
    }

    /// Fetches events for the currently visible range using an existing token.
    pub fn fetch_command_now(
        &self,
        calendar_id: String,
        token: String,
        expires_at: i64,
    ) -> Command<Message> {
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        let refresh_token = self.refresh_token.clone();
        let layout = self.layout();
        let (time_min, time_max) = visible_range(self.selected, layout);
        Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
                refresh_token,
                move |access| async move {
                    crate::api::client::fetch_events(
                        &access,
                        &calendar_id,
                        time_min,
                        time_max,
                    )
                    .await
                    .map_err(|e| e.to_string())
                },
            ),
            Message::DataFetched,
        )
    }

    /// Convenience wrapper: refetches with the current token.
    pub fn refetch(&self) -> Command<Message> {
        let token = match &self.access_token {
            Some(t) => t.clone(),
            None => return Command::none(),
        };
        let calendar_id = self.config.calendar_id.clone();
        self.fetch_command_now(calendar_id, token, self.expires_at)
    }

    /// Save handler for both create and edit. Returns None (and sets
    /// `self.last_error`) on invalid input.
    pub fn handle_save(&mut self) -> Option<Command<Message>> {
        let form = match &self.form {
            Some(f) => f.clone(),
            None => return None,
        };
        let token = match &self.access_token {
            Some(t) => t.clone(),
            None => return None,
        };
        let calendar_id = self.config.calendar_id.clone();
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        let refresh_token = self.refresh_token.clone();
        let expires_at = self.expires_at;

        // Dates and times accept both the extended (YYYY-MM-DD, HH:MM) and the
        // compact (YYYYMMDD, HHMM) formats, so users who touch-type digits
        // don't have to insert separators.
        let start_date = match parse_date_flexible(&form.date) {
            Some(d) => d,
            None => {
                self.last_error = Some("Invalid start date (YYYY-MM-DD or YYYYMMDD)".into());
                return None;
            }
        };
        let end_date = match parse_date_flexible(&form.end_date) {
            Some(d) => d,
            None => {
                self.last_error = Some("Invalid end date (YYYY-MM-DD or YYYYMMDD)".into());
                return None;
            }
        };

        // Convert the (possibly all-day) form fields to UTC instants.
        // All-day end is exclusive: Google expects the day AFTER the last day.
        let (start_utc, end_utc) = if form.all_day {
            let s_naive = start_date.and_hms_opt(0, 0, 0).unwrap();
            let e_naive = (end_date + Duration::days(1))
                .and_hms_opt(0, 0, 0)
                .unwrap();
            (local_to_utc(s_naive), local_to_utc(e_naive))
        } else {
            let start_time = match parse_time_flexible(&form.start_time) {
                Some(t) => t,
                None => {
                    self.last_error = Some("Invalid start time (HH:MM or HHMM)".into());
                    return None;
                }
            };
            let end_time = match parse_time_flexible(&form.end_time) {
                Some(t) => t,
                None => {
                    self.last_error = Some("Invalid end time (HH:MM or HHMM)".into());
                    return None;
                }
            };
            let s_naive = start_date.and_time(start_time);
            let e_naive = end_date.and_time(end_time);
            // `.earliest()` picks the first of two identical local times on
            // DST fall-back. Returns None only if the time doesn't exist at
            // all (spring-forward gap), which is a genuine user error.
            let s = match chrono::Local.from_local_datetime(&s_naive).earliest() {
                Some(dt) => dt.with_timezone(&Utc),
                None => {
                    self.last_error =
                        Some("Invalid start time (does not exist in local timezone)".into());
                    return None;
                }
            };
            let e = match chrono::Local.from_local_datetime(&e_naive).earliest() {
                Some(dt) => dt.with_timezone(&Utc),
                None => {
                    self.last_error =
                        Some("Invalid end time (does not exist in local timezone)".into());
                    return None;
                }
            };
            (s, e)
        };

        if end_utc <= start_utc {
            self.last_error = Some("End date/time must be after start date/time".into());
            return None;
        }

        // The recurrence rule is built whenever the user asked for recurrence
        // (checkbox on), regardless of Create vs Edit mode. When editing a
        // non-recurring event, this converts it to a recurring series. When
        // editing an instance of an existing series, the checkbox is hidden
        // by the UI, so `form.recurring` is false and this branch is skipped.
        let recurrence: Option<Vec<String>> = if form.recurring {
            let interval = form
                .recur_interval
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|&n| n >= 1)
                .unwrap_or(1);
            let mut rrule = format!(
                "RRULE:FREQ={};INTERVAL={}",
                form.recur_freq.rrule_name(),
                interval
            );
            let until = form.recur_until.trim();
            if !until.is_empty() {
                match parse_date_flexible(until) {
                    Some(d) => {
                        // The recurrence end must not be before the event's
                        // own start date, otherwise the RRULE is meaningless
                        // (Google rejects it with HTTP 400, or produces an
                        // empty series).
                        if d < start_date {
                            self.last_error = Some(
                                "Recurrence end date must be on or after the event start date"
                                    .into(),
                            );
                            return None;
                        }
                        // UNTIL format depends on whether DTSTART is a DATE
                        // (all-day) or DATE-TIME (timed). For timed events
                        // UNTIL must be a UTC instant: converting the local
                        // end-of-day avoids off-by-one inclusion/exclusion
                        // for non-UTC timezones.
                        if form.all_day {
                            rrule.push_str(&format!(";UNTIL={}", d.format("%Y%m%d")));
                        } else {
                            let local_end_naive = d.and_hms_opt(23, 59, 59).unwrap();
                            let until_utc = local_to_utc(local_end_naive);
                            rrule.push_str(&format!(
                                ";UNTIL={}",
                                until_utc.format("%Y%m%dT%H%M%SZ")
                            ));
                        }
                    }
                    None => {
                        self.last_error = Some(
                            "Invalid recurrence end date (YYYY-MM-DD or YYYYMMDD)".into(),
                        );
                        return None;
                    }
                }
            }
            Some(vec![rrule])
        } else {
            None
        };

        let title = form.title.clone();
        let color_id = if form.color_id.is_empty() {
            None
        } else {
            Some(form.color_id.clone())
        };
        let mode = form.mode.clone();
        let all_day = form.all_day;
        let scope = form.edit_scope;
        let recurring_event_id = form.recurring_event_id.clone();
        let original_start = form.original_start;

        crate::log::write(&format!(
            "save: mode={:?} title={:?} start={} end={} all_day={} recurring={}",
            mode, title, start_utc, end_utc, all_day, recurrence.is_some()
        ));

        Some(Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
                refresh_token,
                move |access| async move {
                    match mode {
                        FormMode::Create => {
                            let input = EventInput {
                                summary: &title,
                                start: start_utc,
                                end: end_utc,
                                color_id: color_id.as_deref(),
                                all_day,
                                recurrence,
                            };
                            crate::api::client::create_event(&access, &calendar_id, input)
                                .await
                                .map(|_| ())
                        }
                        FormMode::Edit(instance_id) => {
                            let is_recurring = recurring_event_id.is_some();
                            if is_recurring && scope == EditScope::ThisAndFollowing {
                                let base = recurring_event_id.unwrap();
                                let orig = original_start.ok_or_else(|| {
                                    "Missing original start time".to_string()
                                })?;
                                let input = EventInput::new(
                                    &title,
                                    start_utc,
                                    end_utc,
                                    color_id.as_deref(),
                                    all_day,
                                );
                                crate::api::client::update_event_this_and_following(
                                    &access,
                                    &calendar_id,
                                    &base,
                                    orig,
                                    input,
                                )
                                .await
                                .map(|_| ())
                            } else if is_recurring && scope == EditScope::All {
                                let base = recurring_event_id.unwrap();
                                let orig = original_start.ok_or_else(|| {
                                    "Missing original start time".to_string()
                                })?;
                                let input = EventInput::new(
                                    &title,
                                    start_utc,
                                    end_utc,
                                    color_id.as_deref(),
                                    all_day,
                                );
                                crate::api::client::update_event_all_occurrences(
                                    &access,
                                    &calendar_id,
                                    &base,
                                    orig,
                                    input,
                                )
                                .await
                                .map(|_| ())
                            } else {
                                // OnlyThis, or a non-recurring event. In this
                                // branch `recurrence` may be Some(...) when
                                // the user turned on the "Recurring" checkbox
                                // while editing a plain event, converting it
                                // into a series.
                                let input = EventInput {
                                    summary: &title,
                                    start: start_utc,
                                    end: end_utc,
                                    color_id: color_id.as_deref(),
                                    all_day,
                                    recurrence,
                                };
                                crate::api::client::update_event(
                                    &access,
                                    &calendar_id,
                                    &instance_id,
                                    input,
                                )
                                .await
                                .map(|_| ())
                            }
                        }
                    }
                    .map_err(|e| e.to_string())
                },
            ),
            Message::EventSaved,
        ))
    }

    /// Delete handler. Scope determines which API call is used.
    pub fn handle_delete(&mut self) -> Option<Command<Message>> {
        let form = match &self.form {
            Some(f) => f.clone(),
            None => return None,
        };
        let token = match &self.access_token {
            Some(t) => t.clone(),
            None => return None,
        };
        let calendar_id = self.config.calendar_id.clone();
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        let refresh_token = self.refresh_token.clone();
        let expires_at = self.expires_at;
        let instance_id = match &form.mode {
            FormMode::Edit(id) => id.clone(),
            FormMode::Create => return None,
        };
        let scope = form.edit_scope;
        let recurring_event_id = form.recurring_event_id.clone();
        let original_start = form.original_start;

        Some(Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
                refresh_token,
                move |access| async move {
                    let is_recurring = recurring_event_id.is_some();
                    if is_recurring && scope == EditScope::All {
                        let base = recurring_event_id.unwrap();
                        crate::api::client::delete_event(&access, &calendar_id, &base)
                            .await
                            .map_err(|e| e.to_string())
                    } else if is_recurring && scope == EditScope::ThisAndFollowing {
                        let base = recurring_event_id.unwrap();
                        let orig = original_start.ok_or_else(|| {
                            "Missing original start time".to_string()
                        })?;
                        crate::api::client::delete_event_this_and_following(
                            &access,
                            &calendar_id,
                            &base,
                            orig,
                        )
                        .await
                        .map_err(|e| e.to_string())
                    } else {
                        crate::api::client::delete_event(&access, &calendar_id, &instance_id)
                            .await
                            .map_err(|e| e.to_string())
                    }
                },
            ),
            Message::EventDeleted,
        ))
    }

    /// Recreates the last deleted event (Undo banner). Propagates the original
    /// recurrence so a deleted recurring series is restored as a series.
    pub fn handle_undo(&mut self) -> Option<Command<Message>> {
        let pending = self.pending_undo.take()?;
        let token = match &self.access_token {
            Some(t) => t.clone(),
            None => {
                // Put the undo back so the banner doesn't vanish silently.
                self.pending_undo = Some(pending);
                return None;
            }
        };
        let calendar_id = self.config.calendar_id.clone();
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        let refresh_token = self.refresh_token.clone();
        let expires_at = self.expires_at;

        let summary = pending.event.summary.clone();
        let start = pending.event.start;
        let end = pending
            .event
            .end
            .unwrap_or(pending.event.start + Duration::hours(1));
        let color_id = pending.event.color_id.clone();
        let all_day = pending.event.all_day;
        let recurrence = pending.event.recurrence.clone();

        crate::log::write(&format!(
            "undo: recreating deleted event summary={:?} start={} recurring={}",
            summary,
            start,
            recurrence.is_some()
        ));

        Some(Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
                refresh_token,
                move |access| async move {
                    let input = EventInput {
                        summary: &summary,
                        start,
                        end,
                        color_id: color_id.as_deref(),
                        all_day,
                        recurrence,
                    };
                    crate::api::client::create_event(&access, &calendar_id, input)
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                },
            ),
            Message::UndoCompleted,
        ))
    }

    /// Drag & drop: shift the event by whole days. Recurring instances are
    /// moved individually (they get a new id server-side but the series link
    /// is preserved by Google).
    ///
    /// The shift is performed in *local* time to preserve the wall-clock time
    /// across DST transitions: adding days to a UTC timestamp would otherwise
    /// shift the local time by one hour when crossing a DST boundary.
    pub fn handle_move_event(
        &mut self,
        drag: DragState,
        target: NaiveDate,
    ) -> Option<Command<Message>> {
        let token = match &self.access_token {
            Some(t) => t.clone(),
            None => return None,
        };
        let calendar_id = self.config.calendar_id.clone();
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        let refresh_token = self.refresh_token.clone();
        let expires_at = self.expires_at;

        let delta_days = (target - drag.source_date).num_days();
        if delta_days == 0 {
            return None;
        }

        crate::log::write(&format!(
            "dnd: move id={} from {} to {} (delta={}d)",
            drag.event.id, drag.source_date, target, delta_days
        ));

        let event = drag.event;
        let instance_id = event.id.clone();
        let summary = event.summary.clone();
        let color_id = event.color_id.clone();
        let all_day = event.all_day;

        // Shift start/end by calendar days while preserving local time-of-day.
        let new_start = shift_utc_by_local_days(event.start, delta_days);
        let new_end = shift_utc_by_local_days(
            event.end.unwrap_or(event.start + Duration::hours(1)),
            delta_days,
        );

        Some(Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
                refresh_token,
                move |access| async move {
                    // Moving a single occurrence never touches recurrence, so
                    // we pass None and Google leaves the rule alone.
                    let input =
                        EventInput::new(&summary, new_start, new_end, color_id.as_deref(), all_day);
                    crate::api::client::update_event(
                        &access,
                        &calendar_id,
                        &instance_id,
                        input,
                    )
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
                },
            ),
            Message::EventMoved,
        ))
    }
}

/// Ensures the token is fresh (via persistence::ensure_token), then runs `f`
/// with the access token. Any refresh that happened is reported back in the
/// ApiResult so the caller can persist the new refresh token.
async fn run_with_token<F, Fut, T>(
    client_id: String,
    client_secret: String,
    token: String,
    expires_at: i64,
    refresh_token: Option<String>,
    f: F,
) -> ApiResult<T>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let (access, _new_exp, new_token) = match persistence::ensure_token(
        &client_id,
        &client_secret,
        &token,
        expires_at,
        refresh_token.as_deref(),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return ApiResult {
                new_token: None,
                result: Err(e),
            }
        }
    };
    let result = f(access).await;
    ApiResult { new_token, result }
}

/// Parses a date string, accepting both the extended `YYYY-MM-DD` format and
/// the compact `YYYYMMDD` format. Whitespace is trimmed.
fn parse_date_flexible(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d);
    }
    if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(d) = NaiveDate::parse_from_str(s, "%Y%m%d") {
            return Some(d);
        }
    }
    None
}

/// Parses a time string, accepting both the extended `HH:MM` format and the
/// compact `HHMM` format. Whitespace is trimmed.
fn parse_time_flexible(s: &str) -> Option<NaiveTime> {
    let s = s.trim();
    if let Ok(t) = NaiveTime::parse_from_str(s, "%H:%M") {
        return Some(t);
    }
    if s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(t) = NaiveTime::parse_from_str(s, "%H%M") {
            return Some(t);
        }
    }
    None
}

/// Converts a naive local datetime to UTC.
///
/// `earliest()` resolves DST fall-back ambiguity (two identical local times
/// on the same night). For spring-forward gaps (the local time does not exist
/// at all) the function advances by hours until it finds a valid local time,
/// instead of falling back to UTC which would move the event by the full
/// timezone offset.
fn local_to_utc(naive: chrono::NaiveDateTime) -> DateTime<Utc> {
    if let Some(dt) = chrono::Local.from_local_datetime(&naive).earliest() {
        return dt.with_timezone(&Utc);
    }
    // DST spring-forward gap: try successive hours until a valid local time
    // is found. The gap is at most 1-2 hours in practice.
    for h in 1..=3 {
        let candidate = naive + Duration::hours(h);
        if let Some(dt) = chrono::Local.from_local_datetime(&candidate).earliest() {
            return dt.with_timezone(&Utc);
        }
    }
    // Last-resort fallback (should not happen in practice).
    Utc.from_utc_datetime(&naive)
}

/// Shifts a UTC datetime by a number of *calendar* days, preserving the local
/// time-of-day. This is essential for drag & drop across DST boundaries: adding
/// 24-hour days to a UTC instant would change the local wall-clock time.
///
/// For all-day events (which are stored at local midnight) this correctly
/// moves the event to the new local date without shifting the time.
fn shift_utc_by_local_days(dt: DateTime<Utc>, days: i64) -> DateTime<Utc> {
    let local = dt.with_timezone(&chrono::Local);
    let naive = local.naive_local();
    let new_date = naive.date() + Duration::days(days);
    let new_naive = new_date.and_time(naive.time());

    match chrono::Local.from_local_datetime(&new_naive).earliest() {
        Some(new_local) => new_local.with_timezone(&Utc),
        None => {
            // DST gap: the requested local time does not exist (e.g. 02:30 on
            // the spring-forward night). Fall back to adding the days in UTC,
            // which is the previous behavior. A more sophisticated approach
            // would shift to the next valid local time, but this edge case is
            // extremely uncommon.
            dt + Duration::days(days)
        }
    }
}

/// Computes the API fetch window for the current layout. Always starts/ends at
/// local midnight so DST transitions don't shift the boundaries.
fn visible_range(
    selected: chrono::NaiveDate,
    layout: ui::Layout,
) -> (DateTime<Utc>, DateTime<Utc>) {
    match layout {
        ui::Layout::Month => {
            let y = selected.year();
            let m = selected.month();
            let start_naive = chrono::NaiveDate::from_ymd_opt(y, m, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let (ey, em) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            let end_naive = chrono::NaiveDate::from_ymd_opt(ey, em, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            (local_to_utc(start_naive), local_to_utc(end_naive))
        }
        ui::Layout::Week => {
            let start_naive = selected.and_hms_opt(0, 0, 0).unwrap();
            let end_naive = (selected + Duration::days(7))
                .and_hms_opt(0, 0, 0)
                .unwrap();
            (local_to_utc(start_naive), local_to_utc(end_naive))
        }
        ui::Layout::Day => {
            let start_naive = selected.and_hms_opt(0, 0, 0).unwrap();
            let end_naive = (selected + Duration::days(1))
                .and_hms_opt(0, 0, 0)
                .unwrap();
            (local_to_utc(start_naive), local_to_utc(end_naive))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_range_starts_at_local_midnight() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
        let (start, end) = visible_range(d, ui::Layout::Day);
        let start_local = start.with_timezone(&chrono::Local);
        let end_local = end.with_timezone(&chrono::Local);
        assert_eq!(start_local.date_naive(), d);
        assert_eq!(start_local.time(), NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        assert_eq!(end_local.date_naive(), d.succ_opt().unwrap());
        assert_eq!(end_local.time(), NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    }

    #[test]
    fn week_range_is_seven_days_local() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
        let (start, end) = visible_range(d, ui::Layout::Week);
        let start_local = start.with_timezone(&chrono::Local);
        let end_local = end.with_timezone(&chrono::Local);
        assert_eq!(start_local.date_naive(), d);
        assert_eq!(end_local.date_naive(), d + Duration::days(7));
    }

    #[test]
    fn month_range_covers_full_local_month() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 9, 15).unwrap();
        let (start, end) = visible_range(d, ui::Layout::Month);
        let start_local = start.with_timezone(&chrono::Local);
        let end_local = end.with_timezone(&chrono::Local);
        assert_eq!(
            start_local.date_naive(),
            chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()
        );
        assert_eq!(
            end_local.date_naive(),
            chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
        );
    }

    #[test]
    fn parse_date_accepts_extended_and_compact() {
        assert_eq!(
            parse_date_flexible("2026-11-25"),
            NaiveDate::from_ymd_opt(2026, 11, 25)
        );
        assert_eq!(
            parse_date_flexible("20261125"),
            NaiveDate::from_ymd_opt(2026, 11, 25)
        );
        assert_eq!(
            parse_date_flexible("  20261125  "),
            NaiveDate::from_ymd_opt(2026, 11, 25)
        );
        assert_eq!(parse_date_flexible(""), None);
        assert_eq!(parse_date_flexible("abcd"), None);
        assert_eq!(parse_date_flexible("2026-13-01"), None);
    }

    #[test]
    fn parse_time_accepts_extended_and_compact() {
        assert_eq!(
            parse_time_flexible("17:30"),
            NaiveTime::from_hms_opt(17, 30, 0)
        );
        assert_eq!(
            parse_time_flexible("1730"),
            NaiveTime::from_hms_opt(17, 30, 0)
        );
        assert_eq!(
            parse_time_flexible("0000"),
            NaiveTime::from_hms_opt(0, 0, 0)
        );
        assert_eq!(parse_time_flexible(""), None);
        assert_eq!(parse_time_flexible("2500"), None);
        assert_eq!(parse_time_flexible("17:30:00"), None);
    }
}