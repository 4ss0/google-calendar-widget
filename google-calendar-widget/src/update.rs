use crate::app::{App, AppState, EventIndex, FormMode};
use crate::messages::Message;
use crate::persistence;
use crate::tray;
use crate::ui::{self, AppTheme};
use chrono::Duration;
use iced::{Command, Size};
use tray_icon::menu::MenuEvent;
use tray_icon::TrayIconEvent;

impl App {
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
                self.bg_alpha = (self.bg_alpha - 0.08).max(0.10);
                Command::none()
            }
            Message::DecreaseTransparency => {
                self.bg_alpha = (self.bg_alpha + 0.08).min(1.0);
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
                self.last_error = Some(format!("Errore autostart: {}", e));
                Command::none()
            }
            Message::ApplyWindowEffects => {
                #[cfg(target_os = "windows")]
                if let Some(hwnd) = crate::platform::windows::find_hwnd("Google Calendar Widget") {
                    crate::platform::windows::set_bottom(hwnd);
                    crate::platform::windows::hide_from_taskbar(hwnd);
                    crate::platform::windows::remove_minimize_box(hwnd);
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
                self.state = AppState::Error(format!("Errore autenticazione: {}", e));
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
                        self.state = AppState::Error(format!("Errore dati: {}", e));
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
                    date,
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
                self.form = Some(crate::app::EventForm {
                    mode: FormMode::Create,
                    title: String::new(),
                    date: date.format("%Y-%m-%d").to_string(),
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
                self.form = Some(crate::app::EventForm {
                    mode: FormMode::Edit(event.id.clone()),
                    title: event.summary.clone(),
                    date: start_local.format("%Y-%m-%d").to_string(),
                    start_time: start_local.format("%H:%M").to_string(),
                    end_time: end_local.format("%H:%M").to_string(),
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
                    f.date = s;
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
            Message::PollTray => {
                if TrayIconEvent::receiver().try_recv().is_ok() {
                    return iced::window::change_mode(
                        iced::window::Id::MAIN,
                        iced::window::Mode::Windowed,
                    );
                }
                if let Ok(event) = MenuEvent::receiver().try_recv() {
                    if let Some(msg) = tray::handle_menu_event(event) {
                        return self.update(msg);
                    }
                }
                Command::none()
            }
            Message::TrayEvent(msg) => match msg {
                crate::tray::TrayMessage::Show => iced::window::change_mode(
                    iced::window::Id::MAIN,
                    iced::window::Mode::Windowed,
                ),
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