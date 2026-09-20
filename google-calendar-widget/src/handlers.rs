use crate::app::{ApiResult, App, FormMode};
use crate::messages::Message;
use crate::persistence;
use crate::ui;
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use iced::Command;

impl App {
    pub fn apply_new_token(&mut self, new_token: Option<crate::config::StoredToken>) {
        if let Some(t) = new_token {
            self.access_token = Some(t.access_token.clone());
            self.expires_at = t.expires_at;
            if let Some(rt) = &t.refresh_token {
                let _ = crate::auth::oauth::save_refresh_token(rt);
            }
        }
    }

    pub fn reveal_window(&mut self) -> Command<Message> {
        if self.started_minimized {
            self.started_minimized = false;
            iced::window::change_mode(iced::window::Id::MAIN, iced::window::Mode::Windowed)
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
        let layout = ui::Layout::from_width(self.window_size.width);
        let (time_min, time_max) = visible_range(self.selected, layout);
        Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
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
        let expires_at = self.expires_at;

        let start_date = match chrono::NaiveDate::parse_from_str(&form.date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                self.last_error = Some("Data inizio non valida (YYYY-MM-DD)".into());
                return None;
            }
        };
        let end_date = match chrono::NaiveDate::parse_from_str(&form.end_date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                self.last_error = Some("Data fine non valida (YYYY-MM-DD)".into());
                return None;
            }
        };
        let start_time = match chrono::NaiveTime::parse_from_str(&form.start_time, "%H:%M") {
            Ok(t) => t,
            Err(_) => {
                self.last_error = Some("Ora inizio non valida (HH:MM)".into());
                return None;
            }
        };
        let end_time = match chrono::NaiveTime::parse_from_str(&form.end_time, "%H:%M") {
            Ok(t) => t,
            Err(_) => {
                self.last_error = Some("Ora fine non valida (HH:MM)".into());
                return None;
            }
        };

        let start_naive = start_date.and_time(start_time);
        let end_naive = end_date.and_time(end_time);

        let start_utc =
            match chrono::TimeZone::from_local_datetime(&chrono::Local, &start_naive).single() {
                Some(dt) => dt.with_timezone(&chrono::Utc),
                None => {
                    self.last_error = Some("Ora inizio ambigua".into());
                    return None;
                }
            };
        let end_utc =
            match chrono::TimeZone::from_local_datetime(&chrono::Local, &end_naive).single() {
                Some(dt) => dt.with_timezone(&chrono::Utc),
                None => {
                    self.last_error = Some("Ora fine ambigua".into());
                    return None;
                }
            };

        if end_utc <= start_utc {
            self.last_error =
                Some("La data/ora di fine deve essere successiva all'inizio".into());
            return None;
        }

        let title = form.title.clone();
        let color_id = if form.color_id.is_empty() {
            None
        } else {
            Some(form.color_id.clone())
        };
        let mode = form.mode.clone();

        Some(Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
                move |access| async move {
                    match mode {
                        FormMode::Create => crate::api::client::create_event(
                            &access,
                            &calendar_id,
                            &title,
                            start_utc,
                            end_utc,
                            color_id.as_deref(),
                        )
                        .await
                        .map(|_| ()),
                        FormMode::Edit(id) => crate::api::client::update_event(
                            &access,
                            &calendar_id,
                            &id,
                            &title,
                            start_utc,
                            end_utc,
                            color_id.as_deref(),
                        )
                        .await
                        .map(|_| ()),
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
        let expires_at = self.expires_at;
        let id = match &form.mode {
            FormMode::Edit(id) => id.clone(),
            FormMode::Create => return None,
        };

        Some(Command::perform(
            run_with_token(
                client_id,
                client_secret,
                token,
                expires_at,
                move |access| async move {
                    crate::api::client::delete_event(&access, &calendar_id, &id)
                        .await
                        .map_err(|e| e.to_string())
                },
            ),
            Message::EventDeleted,
        ))
    }
}

async fn run_with_token<F, Fut, T>(
    client_id: String,
    client_secret: String,
    token: String,
    expires_at: i64,
    f: F,
) -> ApiResult<T>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let (access, _new_exp, new_token) =
        match persistence::ensure_token(&client_id, &client_secret, &token, expires_at).await {
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

fn visible_range(
    selected: chrono::NaiveDate,
    layout: ui::Layout,
) -> (DateTime<Utc>, DateTime<Utc>) {
    match layout {
        ui::Layout::Month => {
            let y = selected.year();
            let m = selected.month();
            let start = Utc.with_ymd_and_hms(y, m, 1, 0, 0, 0).unwrap();
            let (ey, em) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            let end = Utc.with_ymd_and_hms(ey, em, 1, 0, 0, 0).unwrap();
            (start, end)
        }
        ui::Layout::Week => {
            let start_naive = selected.and_hms_opt(0, 0, 0).unwrap();
            let end_naive = (selected + Duration::days(7))
                .and_hms_opt(0, 0, 0)
                .unwrap();
            (
                Utc.from_utc_datetime(&start_naive),
                Utc.from_utc_datetime(&end_naive),
            )
        }
        ui::Layout::Day => {
            let start_naive = selected.and_hms_opt(0, 0, 0).unwrap();
            let end_naive = (selected + Duration::days(1))
                .and_hms_opt(0, 0, 0)
                .unwrap();
            (
                Utc.from_utc_datetime(&start_naive),
                Utc.from_utc_datetime(&end_naive),
            )
        }
    }
}