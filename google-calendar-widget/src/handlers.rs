use crate::app::{ApiResult, App, DragState, EditScope, FormMode};
use crate::messages::Message;
use crate::persistence;
use crate::ui;
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use iced::Command;

impl App {
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

    pub fn reveal_window(&mut self) -> Command<Message> {
        if self.started_minimized {
            self.started_minimized = false;
            Self::show_window_with_effects()
        } else {
            Command::none()
        }
    }

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

    pub fn refetch(&self) -> Command<Message> {
        let token = match &self.access_token {
            Some(t) => t.clone(),
            None => return Command::none(),
        };
        let calendar_id = self.config.calendar_id.clone();
        self.fetch_command_now(calendar_id, token, self.expires_at)
    }

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

        let start_date = match chrono::NaiveDate::parse_from_str(&form.date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                self.last_error = Some("Invalid start date (YYYY-MM-DD)".into());
                return None;
            }
        };
        let end_date = match chrono::NaiveDate::parse_from_str(&form.end_date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                self.last_error = Some("Invalid end date (YYYY-MM-DD)".into());
                return None;
            }
        };

        let (start_utc, end_utc) = if form.all_day {
            let s_naive = start_date.and_hms_opt(0, 0, 0).unwrap();
            let e_naive = (end_date + Duration::days(1))
                .and_hms_opt(0, 0, 0)
                .unwrap();
            (local_to_utc(s_naive), local_to_utc(e_naive))
        } else {
            let start_time = match NaiveTime::parse_from_str(&form.start_time, "%H:%M") {
                Ok(t) => t,
                Err(_) => {
                    self.last_error = Some("Invalid start time (HH:MM)".into());
                    return None;
                }
            };
            let end_time = match NaiveTime::parse_from_str(&form.end_time, "%H:%M") {
                Ok(t) => t,
                Err(_) => {
                    self.last_error = Some("Invalid end time (HH:MM)".into());
                    return None;
                }
            };
            let s_naive = start_date.and_time(start_time);
            let e_naive = end_date.and_time(end_time);
            let s = match chrono::Local.from_local_datetime(&s_naive).single() {
                Some(dt) => dt.with_timezone(&Utc),
                None => {
                    self.last_error = Some("Ambiguous start time".into());
                    return None;
                }
            };
            let e = match chrono::Local.from_local_datetime(&e_naive).single() {
                Some(dt) => dt.with_timezone(&Utc),
                None => {
                    self.last_error = Some("Ambiguous end time".into());
                    return None;
                }
            };
            (s, e)
        };

        if end_utc <= start_utc {
            self.last_error = Some("End date/time must be after start date/time".into());
            return None;
        }

        let recurrence: Option<Vec<String>> =
            if matches!(form.mode, FormMode::Create) && form.recurring {
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
                    match chrono::NaiveDate::parse_from_str(until, "%Y-%m-%d") {
                        Ok(d) => {
                            rrule.push_str(&format!(";UNTIL={}T235959Z", d.format("%Y%m%d")));
                        }
                        Err(_) => {
                            self.last_error =
                                Some("Invalid recurrence end date (YYYY-MM-DD)".into());
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

        Some(Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
                refresh_token,
                move |access| async move {
                    match mode {
                        FormMode::Create => crate::api::client::create_event(
                            &access,
                            &calendar_id,
                            &title,
                            start_utc,
                            end_utc,
                            color_id.as_deref(),
                            all_day,
                            recurrence,
                        )
                        .await
                        .map(|_| ()),
                        FormMode::Edit(instance_id) => {
                            let is_recurring = recurring_event_id.is_some();
                            if is_recurring && scope == EditScope::ThisAndFollowing {
                                let base = recurring_event_id.unwrap();
                                let orig = original_start.ok_or_else(|| {
                                    "Missing original start time".to_string()
                                })?;
                                crate::api::client::update_event_this_and_following(
                                    &access,
                                    &calendar_id,
                                    &base,
                                    orig,
                                    &title,
                                    start_utc,
                                    end_utc,
                                    color_id.as_deref(),
                                    all_day,
                                )
                                .await
                                .map(|_| ())
                            } else if is_recurring && scope == EditScope::All {
                                let base = recurring_event_id.unwrap();
                                let orig = original_start.ok_or_else(|| {
                                    "Missing original start time".to_string()
                                })?;
                                crate::api::client::update_event_all_occurrences(
                                    &access,
                                    &calendar_id,
                                    &base,
                                    orig,
                                    &title,
                                    start_utc,
                                    end_utc,
                                    color_id.as_deref(),
                                    all_day,
                                )
                                .await
                                .map(|_| ())
                            } else {
                                crate::api::client::update_event(
                                    &access,
                                    &calendar_id,
                                    &instance_id,
                                    &title,
                                    start_utc,
                                    end_utc,
                                    color_id.as_deref(),
                                    all_day,
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

    pub fn handle_undo(&mut self) -> Option<Command<Message>> {
        let pending = self.pending_undo.take()?;
        let token = match &self.access_token {
            Some(t) => t.clone(),
            None => {
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

        crate::log::write(&format!(
            "undo: recreating deleted event summary={:?} start={}",
            summary, start
        ));

        Some(Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
                refresh_token,
                move |access| async move {
                    crate::api::client::create_event(
                        &access,
                        &calendar_id,
                        &summary,
                        start,
                        end,
                        color_id.as_deref(),
                        all_day,
                        None,
                    )
                    .await
                    .map(|_| ())
                    .map_err(|e| e.to_string())
                },
            ),
            Message::UndoCompleted,
        ))
    }

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
        let new_start = event.start + Duration::days(delta_days);
        let new_end = event
            .end
            .unwrap_or(event.start + Duration::hours(1))
            + Duration::days(delta_days);

        Some(Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
                refresh_token,
                move |access| async move {
                    crate::api::client::update_event(
                        &access,
                        &calendar_id,
                        &instance_id,
                        &summary,
                        new_start,
                        new_end,
                        color_id.as_deref(),
                        all_day,
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

fn local_to_utc(naive: chrono::NaiveDateTime) -> DateTime<Utc> {
    match chrono::Local.from_local_datetime(&naive).earliest() {
        Some(dt) => dt.with_timezone(&Utc),
        None => Utc.from_utc_datetime(&naive),
    }
}

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
}