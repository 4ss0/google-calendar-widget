mod api;
mod auth;
mod config;
mod platform;
mod ui;

use api::client::{create_event, delete_event, fetch_month_events, update_event, CalendarEvent};
use api::colors::ColorPalette;
use auth::oauth::{
    load_refresh_token, refresh_access_token, run_full_auth_flow, save_refresh_token,
};
use auto_launch::AutoLaunch;
use config::{AppConfig, StoredToken, WindowState};
use chrono::{Datelike, Duration, NaiveDate};
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{button, column, container, mouse_area, pick_list, row, text, text_input};
use iced::window::Position;
use iced::{
    Application, Background, Border, Color, Command, Element, Length, Point, Size,
    Subscription, Theme,
};

const NARROW_THRESHOLD: f32 = 800.0;
const DEFAULT_WIDTH: f32 = 1150.0;
const DEFAULT_HEIGHT: f32 = 850.0;
const TOKEN_REFRESH_MARGIN: i64 = 60;
const FOCUS_DEBOUNCE_SECS: u64 = 3;

fn main() -> iced::Result {
    let started_minimized = std::env::args().any(|a| a == "--minimized");
    // uncomment for logs
    //tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let saved = WindowState::load();
    let (size, position) = match &saved {
        Some(w) => (
            Size::new(w.width, w.height),
            Position::Specific(Point::new(w.x, w.y)),
        ),
        None => (
            Size::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
            Position::Default,
        ),
    };

    App::run(iced::Settings {
        window: iced::window::Settings {
            transparent: true,
            decorations: false,
            level: iced::window::Level::Normal,
            resizable: true,
            visible: !started_minimized,
            size,
            position,
            min_size: Some(iced::Size::new(360.0, 420.0)),
            ..Default::default()
        },
        ..Default::default()
    })
}

#[derive(Debug, Clone)]
enum FormMode {
    Create,
    Edit(String),
}

#[derive(Debug, Clone)]
struct EventForm {
    mode: FormMode,
    title: String,
    date: String,
    start_time: String,
    end_time: String,
    color_id: String,
}
#[derive(Debug, Clone)]struct ApiResult<T> {
    new_token: Option<StoredToken>,
    result: Result<T, String>,
}

struct App {
    config: AppConfig,
    state: AppState,
    selected: NaiveDate,
    window_size: Size,
    window_position: Point,
    last_cursor: Point,
    resize_start: Option<(f32, f32, Size)>,
    bg_alpha: f32,
    autostart_enabled: bool,
    started_minimized: bool,
    menu_open: bool,
    access_token: Option<String>,
    expires_at: i64,
    last_focus_fetch: Option<std::time::Instant>,
    form: Option<EventForm>,
    last_error: Option<String>,
}

enum AppState {
    WaitingAuth,
    Loading,
    Ready {
        events: Vec<CalendarEvent>,
        palette: ColorPalette,
    },
    Error(String),
}

#[derive(Debug, Clone)]
enum Message {
    TokenPolled(Result<StoredToken, String>),
    DataFetched(ApiResult<Vec<CalendarEvent>>),
    WindowResized(Size),
    WindowMoved(Point),
    WindowFocused,
    CursorMoved(Point),
    StartResize,
    ResizeEnded,
    Prev,
    Next,
    Today,
    OpenCreateForm,
    OpenEditForm(CalendarEvent),
    CloseForm,
    FormTitleChanged(String),
    FormDateChanged(String),
    FormStartChanged(String),
    FormEndChanged(String),
    FormColorChanged(String),
    SaveEvent,
    DeleteEvent,
    EventSaved(ApiResult<()>),
    EventDeleted(ApiResult<()>),
    StartDrag,
    CloseWindow,
    Reauthenticate,
    ApplyWindowEffects,
    KeepAtBottom,
    IncreaseTransparency,
    DecreaseTransparency,
    ToggleAutostart,
    AutostartToggled(Result<bool, String>),
    ToggleMenu,
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let today = chrono::Local::now().date_naive();
        let autostart_enabled = is_autostart_enabled();
        let started_minimized = std::env::args().any(|a| a == "--minimized");

        let saved = WindowState::load();
        let (window_size, window_position) = match saved {
            Some(w) => (Size::new(w.width, w.height), Point::new(w.x, w.y)),
            None => (
                Size::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
                Point::new(0.0, 0.0),
            ),
        };

        match AppConfig::load() {
            Ok(config) => {
                let client_id = config.client_id.clone();
                let client_secret = config.client_secret.clone();
                let existing_refresh = load_refresh_token();

                let auth_cmd = if let Some(rt) = existing_refresh {
                    Command::perform(
                        async move {
                            refresh_access_token(&client_id, &client_secret, &rt)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        Message::TokenPolled,
                    )
                } else {
                    Command::perform(
                        async move {
                            run_full_auth_flow(&client_id, &client_secret)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        Message::TokenPolled,
                    )
                };

                let platform_cmd = Command::perform(
                    async {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    },
                    |_| Message::ApplyWindowEffects,
                );

                (
                    Self {
                        config,
                        state: AppState::WaitingAuth,
                        selected: today,
                        window_size,
                        window_position,
                        last_cursor: Point::new(0.0, 0.0),
                        resize_start: None,
                        bg_alpha: 0.65,
                        autostart_enabled,
                        started_minimized,
                        menu_open: false,
                        access_token: None,
                        expires_at: 0,
                        last_focus_fetch: None,
                        form: None,
                        last_error: None,
                    },
                    Command::batch(vec![auth_cmd, platform_cmd]),
                )
            }
            Err(e) => (
                Self {
                    config: AppConfig {
                        client_id: String::new(),
                        client_secret: String::new(),
                        calendar_id: "primary".into(),
                    },
                    state: AppState::Error(format!(
                        "Configurazione non trovata (crea il file .env): {}",
                        e
                    )),
                    selected: today,
                    window_size,
                    window_position,
                    last_cursor: Point::new(0.0, 0.0),
                    resize_start: None,
                    bg_alpha: 0.65,
                    autostart_enabled,
                    started_minimized,
                    menu_open: false,
                    access_token: None,
                    expires_at: 0,
                    last_focus_fetch: None,
                    form: None,
                    last_error: None,
                },
                Command::none(),
            ),
        }
    }

    fn title(&self) -> String {
        "Google Calendar Widget".into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let events = iced::event::listen_with(|event, _status| match event {
            iced::Event::Window(
                _id,
                iced::window::Event::Resized { width, height },
            ) => Some(Message::WindowResized(Size::new(width as f32, height as f32))),
            iced::Event::Window(_id, iced::window::Event::Moved { x, y }) => {
                Some(Message::WindowMoved(Point::new(x as f32, y as f32)))
            }
            iced::Event::Window(_id, iced::window::Event::Focused) => {
                Some(Message::WindowFocused)
            }
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Some(Message::CursorMoved(position))
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                iced::mouse::Button::Left,
            )) => Some(Message::ResizeEnded),
            _ => None,
        });

        let bottom_tick = iced::time::every(std::time::Duration::from_secs(2))
            .map(|_| Message::KeepAtBottom);

        Subscription::batch(vec![events, bottom_tick])
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::WindowResized(size) => {
                self.window_size = size;
                if size.width >= NARROW_THRESHOLD {
                    self.menu_open = false;
                }
                save_window_state(self.window_position, self.window_size);
                Command::none()
            }
            Message::WindowMoved(pos) => {
                self.window_position = pos;
                save_window_state(self.window_position, self.window_size);
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
                    if last.elapsed().as_secs() < FOCUS_DEBOUNCE_SECS {
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
                    return iced::window::resize(
                        iced::window::Id::MAIN,
                        Size::new(new_w, new_h),
                    );
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
                save_window_state(self.window_position, self.window_size);
                Command::none()
            }
            Message::ToggleMenu => {
                self.menu_open = !self.menu_open;
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
                            enable_autostart()
                                .map(|_| true)
                                .map_err(|e| e.to_string())
                        } else {
                            disable_autostart()
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
                {
                    if let Some(hwnd) = platform::windows::find_hwnd("Google Calendar Widget") {
                        platform::windows::set_bottom(hwnd);
                    }
                }
                Command::none()
            }
            Message::KeepAtBottom => {
                #[cfg(target_os = "windows")]
                {
                    if let Some(hwnd) = platform::windows::find_hwnd("Google Calendar Widget") {
                        platform::windows::set_bottom(hwnd);
                    }
                }
                Command::none()
            }
            Message::TokenPolled(Ok(token)) => {
                if let Some(rt) = &token.refresh_token {
                    let _ = save_refresh_token(rt);
                }
                self.access_token = Some(token.access_token.clone());
                self.expires_at = token.expires_at;
                self.state = AppState::Loading;
                let calendar_id = self.config.calendar_id.clone();
                self.fetch_command_now(calendar_id, token.access_token, token.expires_at)
            }
            Message::TokenPolled(Err(e)) => {
                self.state = AppState::Error(format!(
                    "Errore autenticazione: {}. Premi Riprova per rifare il login.",
                    e
                ));
                self.reveal_window()
            }
            Message::DataFetched(api_result) => {
                self.apply_new_token(api_result.new_token);
                match api_result.result {
                    Ok(events) => {
                        self.state = AppState::Ready {
                            events,
                            palette: ColorPalette::standard(),
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
                    ui::Layout::Month => shift_month(self.selected, -1),
                    ui::Layout::Week => self.selected - Duration::days(7),
                    ui::Layout::Day => self.selected - Duration::days(1),
                };
                self.refetch()
            }
            Message::Next => {
                let layout = ui::Layout::from_width(self.window_size.width);
                self.selected = match layout {
                    ui::Layout::Month => shift_month(self.selected, 1),
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
                let end = (now + chrono::Duration::hours(1))
                    .format("%H:%M")
                    .to_string();
                self.form = Some(EventForm {
                    mode: FormMode::Create,
                    title: String::new(),
                    date,
                    start_time: start,
                    end_time: end,
                    color_id: String::new(),
                });
                self.last_error = None;
                Command::none()
            }
            Message::OpenEditForm(event) => {
                let start_local = event.start.with_timezone(&chrono::Local);
                let end_local = event
                    .end
                    .unwrap_or(event.start + chrono::Duration::hours(1))
                    .with_timezone(&chrono::Local);
                self.form = Some(EventForm {
                    mode: FormMode::Edit(event.id.clone()),
                    title: event.summary.clone(),
                    date: start_local.format("%Y-%m-%d").to_string(),
                    start_time: start_local.format("%H:%M").to_string(),
                    end_time: end_local.format("%H:%M").to_string(),
                    color_id: event.color_id.clone().unwrap_or_default(),
                });
                self.last_error = None;
                Command::none()
            }
            Message::CloseForm => {
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
            Message::SaveEvent => self.handle_save(),
            Message::DeleteEvent => self.handle_delete(),
            Message::EventSaved(api_result) => {
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
                save_window_state(self.window_position, self.window_size);
                iced::window::close(iced::window::Id::MAIN)
            }
            Message::Reauthenticate => {
                let client_id = self.config.client_id.clone();
                let client_secret = self.config.client_secret.clone();
                self.state = AppState::WaitingAuth;
                Command::perform(
                    async move {
                        run_full_auth_flow(&client_id, &client_secret)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::TokenPolled,
                )
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let layout = ui::Layout::from_width(self.window_size.width);
        let top_bar = self.view_top_bar(layout);

        let content: Element<Message> = match &self.state {
            AppState::WaitingAuth => container(
                column(vec![
                    text("Autorizzazione Google Calendar").size(20).into(),
                    text("Il browser è stato aperto per l'autorizzazione.").into(),
                    text("Completa il login e autorizza l'accesso.").into(),
                    text("In attesa di conferma...").into(),
                ])
                .spacing(12),
            )
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
            AppState::Loading => container(text("Caricamento eventi...").size(18))
                .padding(20)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            AppState::Ready { events, palette } => {
                let mut items: Vec<Element<Message>> = Vec::new();

                if let Some(err) = &self.last_error {
                    items.push(text(format!("Errore: {}", err)).size(13).into());
                }

                if let Some(form) = &self.form {
                    items.push(self.view_form(form, palette));
                }

                let cal: Element<Message> = ui::build_view(
                    layout,
                    self.selected,
                    events,
                    palette,
                    self.window_size.width,
                );
                items.push(cal);

                column(items)
                    .spacing(6)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
            AppState::Error(e) => {
                let msg: Element<Message> = text(format!("Errore: {}", e)).size(16).into();
                let retry_btn: Element<Message> = button("Riprova autenticazione")
                    .on_press(Message::Reauthenticate)
                    .into();
                column(vec![msg, retry_btn])
                    .spacing(12)
                    .padding(20)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
        };

        let body = column(vec![top_bar, content])
            .spacing(6)
            .width(Length::Fill)
            .height(Length::Fill);

        let alpha = self.bg_alpha;
        let inner: Element<Message> = container(body)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme: &Theme| ContainerAppearance {
                text_color: Some(Color::BLACK),
                background: Some(Background::Color(Color::from_rgba(0.98, 0.98, 1.0, alpha))),
                border: Border {
                    color: Color::from_rgba(0.55, 0.55, 0.65, (alpha * 0.8).min(0.8)),
                    width: 1.0,
                    radius: 12.0.into(),
                },
                shadow: Default::default(),
            })
            .into();

        let handle: Element<Message> = mouse_area(
            container(text(""))
                .width(Length::Fixed(16.0))
                .height(Length::Fixed(16.0))
                .style(|_theme: &Theme| ContainerAppearance {
                    text_color: None,
                    background: Some(Background::Color(Color::from_rgba(0.4, 0.4, 0.5, 0.15))),
                    border: Border {
                        color: Color::from_rgba(0.4, 0.4, 0.5, 0.55),
                        width: 1.0,
                        radius: 3.0.into(),
                    },
                    shadow: Default::default(),
                }),
        )
        .on_press(Message::StartResize)
        .into();

        let handle_row: Element<Message> = container(handle)
            .width(Length::Fill)
            .height(Length::Fixed(20.0))
            .padding([2, 4])
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Bottom)
            .into();

        column(vec![inner, handle_row])
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Light
    }
}

impl App {
    fn apply_new_token(&mut self, new_token: Option<StoredToken>) {
        if let Some(t) = new_token {
            self.access_token = Some(t.access_token.clone());
            self.expires_at = t.expires_at;
            if let Some(rt) = &t.refresh_token {
                let _ = save_refresh_token(rt);
            }
        }
    }

    fn reveal_window(&mut self) -> Command<Message> {
        if self.started_minimized {
            self.started_minimized = false;
            iced::window::change_mode(
                iced::window::Id::MAIN,
                iced::window::Mode::Windowed,
            )
        } else {
            Command::none()
        }
    }

    fn fetch_command_now(
        &self,
        calendar_id: String,
        token: String,
        expires_at: i64,
    ) -> Command<Message> {
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        let selected = self.selected;
        Command::perform(
            async move {
                let (access, _new_exp, new_token) =
                    match ensure_token(&client_id, &client_secret, &token, expires_at).await {
                        Ok(v) => v,
                        Err(e) => {
                            return ApiResult {
                                new_token: None,
                                result: Err(e),
                            }
                        }
                    };
                let result = fetch_month_events(
                    &access,
                    &calendar_id,
                    selected.year(),
                    selected.month(),
                )
                .await
                .map_err(|e| e.to_string());
                ApiResult { new_token, result }
            },
            Message::DataFetched,
        )
    }

    fn refetch(&self) -> Command<Message> {
        let token = match &self.access_token {
            Some(t) => t.clone(),
            None => return Command::none(),
        };
        let calendar_id = self.config.calendar_id.clone();
        self.fetch_command_now(calendar_id, token, self.expires_at)
    }

    fn view_top_bar(&self, layout: ui::Layout) -> Element<'_, Message> {
        let is_narrow = self.window_size.width < NARROW_THRESHOLD;

        let title_long = match layout {
            ui::Layout::Month => self.selected.format("%B %Y").to_string(),
            ui::Layout::Week => {
                let start = self.selected;
                let end = start + Duration::days(6);
                format!(
                    "{} – {}",
                    start.format("%d %b"),
                    end.format("%d %b %Y")
                )
            }
            ui::Layout::Day => self.selected.format("%A %d %B %Y").to_string(),
        };

        let title_short = match layout {
            ui::Layout::Month => self.selected.format("%m/%y").to_string(),
            ui::Layout::Week => {
                let start = self.selected;
                let end = start + Duration::days(6);
                format!("{}–{}", start.format("%d/%m"), end.format("%d/%m"))
            }
            ui::Layout::Day => self.selected.format("%d/%m").to_string(),
        };

        let prev_btn: Element<Message> = button(text("<").size(16))
            .on_press(Message::Prev)
            .padding([2, 10])
            .into();
        let next_btn: Element<Message> = button(text(">").size(16))
            .on_press(Message::Next)
            .padding([2, 10])
            .into();

        let today_btn: Element<Message> = button(text("Oggi").size(13))
            .on_press(Message::Today)
            .padding([4, 10])
            .into();
        let new_btn: Element<Message> = button(text("+").size(18))
            .on_press(Message::OpenCreateForm)
            .padding([2, 12])
            .into();
        let less_transp_btn: Element<Message> = button(text("+").size(14))
            .on_press(Message::DecreaseTransparency)
            .padding([2, 8])
            .into();
        let more_transp_btn: Element<Message> = button(text("-").size(14))
            .on_press(Message::IncreaseTransparency)
            .padding([2, 8])
            .into();
        let opacity_lbl: Element<Message> = container(
            text(format!("{:.0}%", self.bg_alpha * 100.0)).size(12),
        )
        .padding([4, 4])
        .into();

        let autostart_label = if self.autostart_enabled {
            "Auto: ON"
        } else {
            "Auto: OFF"
        };
        let autostart_btn: Element<Message> = button(text(autostart_label).size(11))
            .on_press(Message::ToggleAutostart)
            .padding([4, 8])
            .into();

        let close_btn: Element<Message> = button(text("X").size(14))
            .on_press(Message::CloseWindow)
            .padding([4, 10])
            .into();

        let drag_area: Element<Message> = mouse_area(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(26.0)),
        )
        .on_press(Message::StartDrag)
        .into();

        if !is_narrow {
            let title: Element<Message> = container(text(title_long).size(18))
                .padding([4, 6])
                .into();

            return row(vec![
                prev_btn,
                next_btn,
                title,
                today_btn,
                new_btn,
                more_transp_btn,
                opacity_lbl,
                less_transp_btn,
                autostart_btn,
                drag_area,
                close_btn,
            ])
            .spacing(6)
            .align_items(iced::Alignment::Center)
            .into();
        }

        let title: Element<Message> = container(text(title_short).size(15))
            .padding([4, 6])
            .into();

        let menu_btn: Element<Message> = if self.menu_open {
            button(text("X").size(14))
                .on_press(Message::ToggleMenu)
                .padding([4, 10])
                .into()
        } else {
            button(text("=").size(18))
                .on_press(Message::ToggleMenu)
                .padding([2, 10])
                .into()
        };

        let bar_row: Element<Message> = row(vec![
            prev_btn,
            next_btn,
            title,
            drag_area,
            menu_btn,
            close_btn,
        ])
        .spacing(6)
        .align_items(iced::Alignment::Center)
        .into();

        if !self.menu_open {
            return bar_row;
        }

        let menu_row1: Element<Message> = row(vec![today_btn, new_btn, autostart_btn])
            .spacing(6)
            .padding([2, 2])
            .into();

        let menu_row2: Element<Message> = row(vec![
            more_transp_btn,
            opacity_lbl,
            less_transp_btn,
        ])
        .spacing(6)
        .padding([2, 2])
        .into();

        column(vec![bar_row, menu_row1, menu_row2])
            .spacing(4)
            .into()
    }

    fn handle_save(&mut self) -> Command<Message> {
        let form = match &self.form {
            Some(f) => f.clone(),
            None => return Command::none(),
        };
        let token = match &self.access_token {
            Some(t) => t.clone(),
            None => return Command::none(),
        };
        let calendar_id = self.config.calendar_id.clone();
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        let expires_at = self.expires_at;

        let date = match chrono::NaiveDate::parse_from_str(&form.date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                self.last_error = Some("Data non valida (YYYY-MM-DD)".into());
                return Command::none();
            }
        };
        let start_time = match chrono::NaiveTime::parse_from_str(&form.start_time, "%H:%M") {
            Ok(t) => t,
            Err(_) => {
                self.last_error = Some("Ora inizio non valida (HH:MM)".into());
                return Command::none();
            }
        };
        let end_time = match chrono::NaiveTime::parse_from_str(&form.end_time, "%H:%M") {
            Ok(t) => t,
            Err(_) => {
                self.last_error = Some("Ora fine non valida (HH:MM)".into());
                return Command::none();
            }
        };

        let start_naive = date.and_time(start_time);
        let end_naive = date.and_time(end_time);

        let start_utc =
            match chrono::TimeZone::from_local_datetime(&chrono::Local, &start_naive).single() {
                Some(dt) => dt.with_timezone(&chrono::Utc),
                None => {
                    self.last_error = Some("Ora inizio ambigua".into());
                    return Command::none();
                }
            };
        let end_utc =
            match chrono::TimeZone::from_local_datetime(&chrono::Local, &end_naive).single() {
                Some(dt) => dt.with_timezone(&chrono::Utc),
                None => {
                    self.last_error = Some("Ora fine ambigua".into());
                    return Command::none();
                }
            };

        let title = form.title.clone();
        let color_id = if form.color_id.is_empty() {
            None
        } else {
            Some(form.color_id.clone())
        };
        let mode = form.mode.clone();

        Command::perform(
            async move {
                let (access, _new_exp, new_token) =
                    match ensure_token(&client_id, &client_secret, &token, expires_at).await {
                        Ok(v) => v,
                        Err(e) => {
                            return ApiResult {
                                new_token: None,
                                result: Err(e),
                            }
                        }
                    };
                let result = match mode {
                    FormMode::Create => create_event(
                        &access,
                        &calendar_id,
                        &title,
                        start_utc,
                        end_utc,
                        color_id.as_deref(),
                    )
                    .await
                    .map(|_| ()),
                    FormMode::Edit(id) => update_event(
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
                };
                ApiResult {
                    new_token,
                    result: result.map_err(|e| e.to_string()),
                }
            },
            Message::EventSaved,
        )
    }

    fn handle_delete(&mut self) -> Command<Message> {
        let form = match &self.form {
            Some(f) => f.clone(),
            None => return Command::none(),
        };
        let token = match &self.access_token {
            Some(t) => t.clone(),
            None => return Command::none(),
        };
        let calendar_id = self.config.calendar_id.clone();
        let client_id = self.config.client_id.clone();
        let client_secret = self.config.client_secret.clone();
        let expires_at = self.expires_at;
        let id = match &form.mode {
            FormMode::Edit(id) => id.clone(),
            FormMode::Create => return Command::none(),
        };

        Command::perform(
            async move {
                let (access, _new_exp, new_token) =
                    match ensure_token(&client_id, &client_secret, &token, expires_at).await {
                        Ok(v) => v,
                        Err(e) => {
                            return ApiResult {
                                new_token: None,
                                result: Err(e),
                            }
                        }
                    };
                let result = delete_event(&access, &calendar_id, &id)
                    .await
                    .map_err(|e| e.to_string());
                ApiResult {
                    new_token,
                    result,
                }
            },
            Message::EventDeleted,
        )
    }

    fn view_form<'a>(
        &self,
        form: &'a EventForm,
        palette: &'a ColorPalette,
    ) -> Element<'a, Message> {
        let title_label: Element<Message> = text("Titolo:").into();
        let title_input: Element<Message> = text_input("Titolo evento", &form.title)
            .on_input(Message::FormTitleChanged)
            .width(Length::Fill)
            .into();
        let row1: Element<Message> = row(vec![title_label, title_input]).spacing(8).into();

        let date_label: Element<Message> = text("Data:").into();
        let date_input: Element<Message> = text_input("YYYY-MM-DD", &form.date)
            .on_input(Message::FormDateChanged)
            .width(Length::Fixed(150.0))
            .into();
        let start_label: Element<Message> = text("Inizio:").into();
        let start_input: Element<Message> = text_input("HH:MM", &form.start_time)
            .on_input(Message::FormStartChanged)
            .width(Length::Fixed(90.0))
            .into();
        let end_label: Element<Message> = text("Fine:").into();
        let end_input: Element<Message> = text_input("HH:MM", &form.end_time)
            .on_input(Message::FormEndChanged)
            .width(Length::Fixed(90.0))
            .into();
        let row2: Element<Message> = row(vec![
            date_label,
            date_input,
            start_label,
            start_input,
            end_label,
            end_input,
        ])
        .spacing(8)
        .into();

        let color_label: Element<Message> = text("Colore:").into();
        let mut options: Vec<String> = palette.event_colors.keys().cloned().collect();
        options.sort();
        let selected = if form.color_id.is_empty() {
            None
        } else {
            Some(form.color_id.clone())
        };
        let color_picker: Element<Message> =
            pick_list(options, selected, Message::FormColorChanged).into();
        let row3: Element<Message> = row(vec![color_label, color_picker]).spacing(8).into();

        let save_btn: Element<Message> = button("Salva").on_press(Message::SaveEvent).into();
        let cancel_btn: Element<Message> = button("Annulla").on_press(Message::CloseForm).into();
        let mut actions: Vec<Element<Message>> = vec![save_btn, cancel_btn];
        if matches!(form.mode, FormMode::Edit(_)) {
            let del_btn: Element<Message> =
                button("Elimina").on_press(Message::DeleteEvent).into();
            actions.push(del_btn);
        }
        let row4: Element<Message> = row(actions).spacing(8).into();

        container(column(vec![row1, row2, row3, row4]).spacing(8))
            .padding(10)
            .style(|_theme: &Theme| ContainerAppearance {
                text_color: Some(Color::BLACK),
                background: Some(Background::Color(Color::from_rgba(0.9, 0.9, 1.0, 0.9))),
                border: Border {
                    color: Color::from_rgb(0.7, 0.7, 0.9),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                shadow: Default::default(),
            })
            .into()
    }
}

async fn ensure_token(
    client_id: &str,
    client_secret: &str,
    token: &str,
    expires_at: i64,
) -> Result<(String, i64, Option<StoredToken>), String> {
    let now = chrono::Utc::now().timestamp();
    if expires_at - now > TOKEN_REFRESH_MARGIN {
        return Ok((token.to_string(), expires_at, None));
    }
    let rt = load_refresh_token().ok_or_else(|| "Refresh token non trovato".to_string())?;
    let new_token = refresh_access_token(client_id, client_secret, &rt)
        .await
        .map_err(|e| e.to_string())?;
    Ok((
        new_token.access_token.clone(),
        new_token.expires_at,
        Some(new_token),
    ))
}

fn shift_month(d: NaiveDate, delta: i32) -> NaiveDate {
    let mut y = d.year();
    let mut m = d.month() as i32 + delta;
    while m < 1 {
        m += 12;
        y -= 1;
    }
    while m > 12 {
        m -= 12;
        y += 1;
    }
    NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap_or(d)
}

fn save_window_state(position: Point, size: Size) {
    let state = WindowState {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    };
    let _ = state.save();
}

fn autostart_instance() -> AutoLaunch {
    let exe_path = std::env::current_exe().unwrap_or_default();
    let exe_str = exe_path.to_string_lossy().to_string();
    AutoLaunch::new("GoogleCalendarWidget", &exe_str, &["--minimized"])
}

fn is_autostart_enabled() -> bool {
    autostart_instance().is_enabled().unwrap_or(false)
}

fn enable_autostart() -> Result<(), String> {
    autostart_instance().enable().map_err(|e| e.to_string())
}

fn disable_autostart() -> Result<(), String> {
    autostart_instance()
        .disable()
        .map_err(|e| e.to_string())
}