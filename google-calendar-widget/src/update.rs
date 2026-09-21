use crate::app::{App, AppState, EventIndex, FormMode};
use crate::messages::Message;
use crate::persistence;
use crate::tray;
use crate::ui::{self, AppTheme};
use chrono::Duration;
use iced::{Command, Size};
use tray_icon::menu::MenuEvent;
use tray_icon::TrayIconEvent;

const MIN_WINDOW_ALPHA: f32 = 0.55;

impl App {
    pub fn show_window_with_effects() -> Command<Message> {
        let show = iced::window::change_mode(
            iced::window::Id::MAIN,
            iced::window::Mode::Windowed,
        );
        let effects1 = Command::perform(
            async {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            },
            |_| Message::ApplyWindowEffectsDeferred,
        );
        let effects2 = Command::perform(
            async {
                tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            },
            |_| Message::ApplyWindowEffectsDeferred,
        );
        let effects3 = Command::perform(
            async {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            },
            |_| Message::ApplyWindowEffectsDeferred,
        );
        Command::batch(vec![show, effects1, effects2, effects3])
    }

    pub fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::WindowResized(size) => {
                if size.width == 0.0 && size.height == 0.0 {
                    #[cfg(target_os = "windows")]
                    if let Some(hwnd) =
                        crate::platform::windows::find_hwnd("Google Calendar Widget")
                    {
                        crate::platform::windows::restore_window(hwnd);
                    }
                }
                self.window_size = size;
                if size.width >= 800.0 {
                    self.menu_open = false;
                }
                self.schedule_window_save()
            }
            Message::WindowMoved(pos) => {
                self.window_position = pos;
                self.schedule_window_save()
            }
            Message::FlushWindowState => {
                if self.pending_window_save {
                    self.pending_window_save = false;
                    persistence::save_window_state(
                        self.window_position,
                        self.window_size,
                        self.theme.is_dark,
                    );
                }
                Command::none()
            }
            Message::WindowFocused => {
                if !matches!(self.state, AppState::Ready { .. }) {
                    return Command::none();
                }
                if self.access_token.is_none() {
                    return Command::none();
                }
                if let Some(last) = self.last_focus_fetch {
                    if last.elapsed().as_secs() < 3 {
                        return Command::none();
                    }
                }
                self.last_focus_fetch = Some(std::time::Instant::now());
                self.refetch()
            }
            Message::CursorMoved(pos) => {
                self.last_cursor = pos;
                if let Some((sx, sy, start_size)) = self.resize_start {
                    let dw = pos.x - sx;
                    let dh = pos.y - sy;
                    let new_w = (start_size.width + dw).max(360.0);
                    let new_h = (start_size.height + dh).max(420.0);
                    self.window_size = Size::new(new_w, new_h);
                    return iced::window::resize(iced::window::Id::MAIN, Size::new(new_w, new_h));
                }
                Command::none()
            }
            Message::StartResize => {
                self.resize_start =
                    Some((self.last_cursor.x, self.last_cursor.y, self.window_size));
                Command::none()
            }
            Message::ResizeEnded => {
                self.resize_start = None;
                self.pending_window_save = false;
                persistence::save_window_state(
                    self.window_position,
                    self.window_size,
                    self.theme.is_dark,
                );
                Command::none()
            }
            Message::ToggleMenu => {
                self.menu_open = !self.menu_open;
                Command::none()
            }
            Message::ToggleTheme => {
                self.theme = if self.theme.is_dark {
                    AppTheme::light()
                } else {
                    AppTheme::dark()
                };
                persistence::save_window_state(
                    self.window_position,
                    self.window_size,
                    self.theme.is_dark,
                );
                Command::none()
            }
            Message::IncreaseTransparency => {
                self.window_alpha = (self.window_alpha - 0.08).max(MIN_WINDOW_ALPHA);
                #[cfg(target_os = "windows")]
                if let Some(hwnd) = crate::platform::windows::find_hwnd("Google Calendar Widget") {
                    crate::platform::windows::set_window_alpha(hwnd, self.window_alpha);
                }
                Command::none()
            }
            Message::DecreaseTransparency => {
                self.window_alpha = (self.window_alpha + 0.08).min(1.0);
                #[cfg(target_os = "windows")]
                if let Some(hwnd) = crate::platform::windows::find_hwnd("Google Calendar Widget") {
                    crate::platform::windows::set_window_alpha(hwnd, self.window_alpha);
                }
                Command::none()
            }
            Message::ToggleAutostart => {
                let enable = !self.autostart_enabled;
                Command::perform(
                    async move {
                        if enable {
                            crate::autostart::enable_autostart()
                                .map(|_| true)
                                .map_err(|e| e.to_string())
                        } else {
                            crate::autostart::disable_autostart()
                                .map(|_| false)
                                .map_err(|e| e.to_string())
                        }
                    },
                    Message::AutostartToggled,
                )
            }
            Message::AutostartToggled(Ok(enabled)) => {
                self.autostart_enabled = enabled;
                Command::none()
            }
            Message::AutostartToggled(Err(e)) => {
                self.last_error = Some(format!("Autostart error: {}", e));
                Command::none()
            }
            Message::ApplyWindowEffects => {
                if !self.started_minimized {
                    self.started_minimized = true;
                    Self::show_window_with_effects()
                } else {
                    Command::batch(vec![
                        Command::perform(
                            async {
                                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                            },
                            |_| Message::ApplyWindowEffectsDeferred,
                        ),
                        Command::perform(
                            async {
                                tokio::time::sleep(std::time::Duration::from_millis(700)).await;
                            },
                            |_| Message::ApplyWindowEffectsDeferred,
                        ),
                        Command::perform(
                            async {
                                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                            },
                            |_| Message::ApplyWindowEffectsDeferred,
                        ),
                    ])
                }
            }
            Message::ApplyWindowEffectsDeferred => {
                #[cfg(target_os = "windows")]
                if let Some(hwnd) = crate::platform::windows::find_hwnd("Google Calendar Widget") {
                    crate::platform::windows::hide_from_taskbar(hwnd);
                    crate::platform::windows::remove_minimize_box(hwnd);
                    crate::platform::windows::set_bottom(hwnd);
                    crate::platform::windows::set_window_alpha(hwnd, self.window_alpha);
                    crate::platform::windows::delete_taskbar_tab(hwnd);
                }
                Command::none()
            }
            Message::KeepAtBottom => {
                #[cfg(target_os = "windows")]
                if let Some(hwnd) = crate::platform::windows::find_hwnd("Google Calendar Widget") {
                    crate::platform::windows::set_bottom(hwnd);
                }
                Command::none()
            }
            Message::TokenPolled(Ok(token)) => {
                if let Some(rt) = &token.refresh_token {
                    let _ = crate::auth::oauth::save_refresh_token(rt);
                }
                self.access_token = Some(token.access_token.clone());
                self.expires_at = token.expires_at;
                self.state = AppState::Loading;
                let calendar_id = self.config.calendar_id.clone();
                self.fetch_command_now(calendar_id, token.access_token, token.expires_at)
            }
            Message::TokenPolled(Err(e)) => {
                self.state = AppState::Error(format!("Authentication error: {}", e));
                self.reveal_window()
            }
            Message::DataFetched(api_result) => {
                self.apply_new_token(api_result.new_token);
                match api_result.result {
                    Ok(events) => {
                        self.state = AppState::Ready {
                            index: EventIndex::new(events),
                        };
                        self.last_focus_fetch = Some(std::time::Instant::now());
                        self.reveal_window()
                    }
                    Err(e) => {
                        self.state = AppState::Error(format!("Data error: {}", e));
                        self.reveal_window()
                    }
                }
            }
            Message::Prev => {
                let layout = ui::Layout::from_width(self.window_size.width);
                self.selected = match layout {
                    ui::Layout::Month => crate::date_utils::shift_month(self.selected, -1),
                    ui::Layout::Week => self.selected - Duration::days(7),
                    ui::Layout::Day => self.selected - Duration::days(1),
                };
                self.refetch()
            }
            Message::Next => {
                let layout = ui::Layout::from_width(self.window_size.width);
                self.selected = match layout {
                    ui::Layout::Month => crate::date_utils::shift_month(self.selected, 1),
                    ui::Layout::Week => self.selected + Duration::days(7),
                    ui::Layout::Day => self.selected + Duration::days(1),
                };
                self.refetch()
            }
            Message::Today => {
                self.selected = chrono::Local::now().date_naive();
                self.refetch()
            }
            Message::OpenCreateForm => {
                let now = chrono::Local::now();
                let date = self.selected.format("%Y-%m-%d").to_string();
                let start = now.format("%H:%M").to_string();
                let end = (now + chrono::Duration::hours(1)).format("%H:%M").to_string();
                self.form = Some(crate::app::EventForm {
                    mode: FormMode::Create,
                    title: String::new(),
                    date: date.clone(),
                    end_date: date,
                    start_time: start,
                    end_time: end,
                    color_id: String::new(),
                    confirm_delete: false,
                });
                self.saving = false;
                self.last_error = None;
                Command::none()
            }
            Message::OpenCreateFormForDate(date) => {
                let today = chrono::Local::now().date_naive();
                let (start, end) = if date == today {
                    let now = chrono::Local::now();
                    (
                        now.format("%H:%M").to_string(),
                        (now + chrono::Duration::hours(1)).format("%H:%M").to_string(),
                    )
                } else {
                    ("09:00".to_string(), "10:00".to_string())
                };
                let date_str = date.format("%Y-%m-%d").to_string();
                self.form = Some(crate::app::EventForm {
                    mode: FormMode::Create,
                    title: String::new(),
                    date: date_str.clone(),
                    end_date: date_str,
                    start_time: start,
                    end_time: end,
                    color_id: String::new(),
                    confirm_delete: false,
                });
                self.saving = false;
                self.last_error = None;
                Command::none()
            }
            Message::OpenEditForm(event) => {
                let start_local = event.start.with_timezone(&chrono::Local);
                let end_local = event
                    .end
                    .unwrap_or(event.start + chrono::Duration::hours(1))
                    .with_timezone(&chrono::Local);

                let (date_s, end_date_s, time_s, time_e) = if event.all_day {
                    let sd = start_local.date_naive();
                    let ed = end_local.date_naive();
                    let effective_end = if ed > sd {
                        ed.pred_opt().unwrap_or(ed)
                    } else {
                        ed
                    };
                    (
                        sd.format("%Y-%m-%d").to_string(),
                        effective_end.format("%Y-%m-%d").to_string(),
                        "00:00".to_string(),
                        "23:59".to_string(),
                    )
                } else {
                    (
                        start_local.format("%Y-%m-%d").to_string(),
                        end_local.format("%Y-%m-%d").to_string(),
                        start_local.format("%H:%M").to_string(),
                        end_local.format("%H:%M").to_string(),
                    )
                };

                self.form = Some(crate::app::EventForm {
                    mode: FormMode::Edit(event.id.clone()),
                    title: event.summary.clone(),
                    date: date_s,
                    end_date: end_date_s,
                    start_time: time_s,
                    end_time: time_e,
                    color_id: event.color_id.clone().unwrap_or_default(),
                    confirm_delete: false,
                });
                self.saving = false;
                self.last_error = None;
                Command::none()
            }
            Message::CloseForm => {
                if self.saving {
                    return Command::none();
                }
                self.form = None;
                self.last_error = None;
                Command::none()
            }
            Message::FormTitleChanged(s) => {
                if let Some(f) = &mut self.form {
                    f.title = s;
                }
                Command::none()
            }
            Message::FormDateChanged(s) => {
                if let Some(f) = &mut self.form {
                    f.date = s.clone();
                    if f.end_date.is_empty() || f.end_date < s {
                        f.end_date = s;
                    }
                }
                Command::none()
            }
            Message::FormEndDateChanged(s) => {
                if let Some(f) = &mut self.form {
                    f.end_date = s;
                }
                Command::none()
            }
            Message::FormStartChanged(s) => {
                if let Some(f) = &mut self.form {
                    f.start_time = s;
                }
                Command::none()
            }
            Message::FormEndChanged(s) => {
                if let Some(f) = &mut self.form {
                    f.end_time = s;
                }
                Command::none()
            }
            Message::FormColorChanged(s) => {
                if let Some(f) = &mut self.form {
                    f.color_id = s;
                }
                Command::none()
            }
            Message::SaveEvent => {
                if self.saving {
                    return Command::none();
                }
                match self.handle_save() {
                    Some(cmd) => {
                        self.saving = true;
                        cmd
                    }
                    None => Command::none(),
                }
            }
            Message::RequestDeleteEvent => {
                if self.saving {
                    return Command::none();
                }
                if let Some(f) = &mut self.form {
                    f.confirm_delete = true;
                }
                Command::none()
            }
            Message::CancelDeleteEvent => {
                if let Some(f) = &mut self.form {
                    f.confirm_delete = false;
                }
                Command::none()
            }
            Message::DeleteEvent => {
                if self.saving {
                    return Command::none();
                }
                match self.handle_delete() {
                    Some(cmd) => {
                        self.saving = true;
                        cmd
                    }
                    None => Command::none(),
                }
            }
            Message::EventSaved(api_result) => {
                self.saving = false;
                self.apply_new_token(api_result.new_token);
                match api_result.result {
                    Ok(()) => {
                        self.form = None;
                        self.last_error = None;
                        self.refetch()
                    }
                    Err(e) => {
                        self.last_error = Some(e);
                        Command::none()
                    }
                }
            }
            Message::EventDeleted(api_result) => {
                self.saving = false;
                self.apply_new_token(api_result.new_token);
                match api_result.result {
                    Ok(()) => {
                        self.form = None;
                        self.last_error = None;
                        self.refetch()
                    }
                    Err(e) => {
                        self.last_error = Some(e);
                        Command::none()
                    }
                }
            }
            Message::StartDrag => iced::window::drag(iced::window::Id::MAIN),
            Message::CloseWindow => {
                self.pending_window_save = false;
                persistence::save_window_state(
                    self.window_position,
                    self.window_size,
                    self.theme.is_dark,
                );
                iced::window::close(iced::window::Id::MAIN)
            }
            Message::Reauthenticate => {
                crate::auth::oauth::delete_refresh_token();
                self.access_token = None;
                self.expires_at = 0;
                let client_id = self.config.client_id.clone();
                let client_secret = self.config.client_secret.clone();
                self.state = AppState::WaitingAuth;
                self.saving = false;
                Command::perform(
                    async move {
                        crate::auth::oauth::run_full_auth_flow(&client_id, &client_secret)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::TokenPolled,
                )
            }
            Message::RetryAuth => {
                let client_id = self.config.client_id.clone();
                let client_secret = self.config.client_secret.clone();
                match crate::auth::oauth::load_refresh_token() {
                    Some(rt) => {
                        self.state = AppState::Loading;
                        Command::perform(
                            async move {
                                crate::auth::oauth::refresh_access_token(
                                    &client_id,
                                    &client_secret,
                                    &rt,
                                )
                                .await
                                .map_err(|e| e.to_string())
                            },
                            Message::TokenPolled,
                        )
                    }
                    None => {
                        self.state = AppState::WaitingAuth;
                        Command::perform(
                            async move {
                                crate::auth::oauth::run_full_auth_flow(
                                    &client_id,
                                    &client_secret,
                                )
                                .await
                                .map_err(|e| e.to_string())
                            },
                            Message::TokenPolled,
                        )
                    }
                }
            }
            Message::AutoRetryTick => {
                if !matches!(self.state, AppState::Error(_)) {
                    return Command::none();
                }
                let client_id = self.config.client_id.clone();
                let client_secret = self.config.client_secret.clone();
                match crate::auth::oauth::load_refresh_token() {
                    Some(rt) => Command::perform(
                        async move {
                            crate::auth::oauth::refresh_access_token(
                                &client_id,
                                &client_secret,
                                &rt,
                            )
                            .await
                            .map_err(|e| e.to_string())
                        },
                        Message::TokenPolled,
                    ),
                    None => Command::none(),
                }
            }
            Message::CancelAuth => {
                self.state = AppState::Error(
                    "Authorization cancelled. Click Retry when you are ready.".to_string(),
                );
                Command::none()
            }
            Message::PollTray => {
                if TrayIconEvent::receiver().try_recv().is_ok() {
                    return Self::show_window_with_effects();
                }
                if let Ok(event) = MenuEvent::receiver().try_recv() {
                    if let Some(msg) = tray::handle_menu_event(event) {
                        return self.update(msg);
                    }
                }
                Command::none()
            }
            Message::TrayEvent(msg) => match msg {
                crate::tray::TrayMessage::Show => Self::show_window_with_effects(),
                crate::tray::TrayMessage::Hide => iced::window::change_mode(
                    iced::window::Id::MAIN,
                    iced::window::Mode::Hidden,
                ),
                crate::tray::TrayMessage::Quit => {
                    self.pending_window_save = false;
                    persistence::save_window_state(
                        self.window_position,
                        self.window_size,
                        self.theme.is_dark,
                    );
                    iced::window::close(iced::window::Id::MAIN)
                }
            },
            Message::SetupClientIdChanged(s) => {
                self.setup_form.client_id = s;
                Command::none()
            }
            Message::SetupClientSecretChanged(s) => {
                self.setup_form.client_secret = s;
                Command::none()
            }
            Message::SetupCalendarIdChanged(s) => {
                self.setup_form.calendar_id = s;
                Command::none()
            }
            Message::SetupSubmit => {
                let client_id = self.setup_form.client_id.trim().to_string();
                let client_secret = self.setup_form.client_secret.trim().to_string();
                let calendar_id = if self.setup_form.calendar_id.trim().is_empty() {
                    "primary".to_string()
                } else {
                    self.setup_form.calendar_id.trim().to_string()
                };

                if client_id.is_empty() || client_secret.is_empty() {
                    self.setup_form.error =
                        Some("Client ID and Client Secret are required.".to_string());
                    return Command::none();
                }

                let cfg = crate::config::AppConfig {
                    client_id,
                    client_secret,
                    calendar_id,
                };

                match cfg.save() {
                    Ok(()) => {
                        self.config = cfg;
                        self.state = AppState::WaitingAuth;
                        self.state_before_setup = None;
                        self.setup_form.error = None;

                        crate::auth::oauth::delete_refresh_token();

                        let client_id = self.config.client_id.clone();
                        let client_secret = self.config.client_secret.clone();

                        let auth_cmd = Command::perform(
                            async move {
                                crate::auth::oauth::run_full_auth_flow(
                                    &client_id,
                                    &client_secret,
                                )
                                .await
                                .map_err(|e| e.to_string())
                            },
                            Message::TokenPolled,
                        );

                        let platform_cmd = Command::perform(
                            async {
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            },
                            |_| Message::ApplyWindowEffects,
                        );

                        return Command::batch(vec![auth_cmd, platform_cmd]);
                    }
                    Err(e) => {
                        self.setup_form.error =
                            Some(format!("Error saving: {}", e));
                        Command::none()
                    }
                }
            }
            Message::SetupReconfigure => {
                self.setup_form = crate::app::SetupForm {
                    client_id: self.config.client_id.clone(),
                    client_secret: self.config.client_secret.clone(),
                    calendar_id: self.config.calendar_id.clone(),
                    error: None,
                };
                let prev = std::mem::replace(&mut self.state, AppState::Setup);
                self.state_before_setup = Some(prev);
                Command::none()
            }
            Message::SetupCancel => {
                if let Some(prev) = self.state_before_setup.take() {
                    self.state = prev;
                    self.setup_form.error = None;
                }
                Command::none()
            }
        }
    }

    fn schedule_window_save(&mut self) -> Command<Message> {
        if self.pending_window_save {
            return Command::none();
        }
        self.pending_window_save = true;
        Command::perform(
            async {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            },
            |_| Message::FlushWindowState,
        )
    }
}