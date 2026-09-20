#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod app;
mod auth;
mod autostart;
mod config;
mod crypto;
mod date_utils;
mod handlers;
mod log;
mod messages;
mod persistence;
mod platform;
mod tray;
mod ui;
mod update;
mod view;

use crate::api::colors::ColorPalette;
use crate::app::App;
use crate::messages::Message;
use crate::ui::AppTheme;
use config::{AppConfig, WindowState};
use iced::window::Position;
use iced::{Application, Command, Element, Point, Size, Subscription, Theme};

fn main() -> iced::Result {
    let started_minimized = std::env::args().any(|a| a == "--minimized");
    dotenvy::dotenv().ok();

    crate::log::line();
    crate::log::write("=== app starting ===");
    crate::log::write(&format!(
        "args: {:?}",
        std::env::args().collect::<Vec<String>>()
    ));
    crate::log::write(&format!("log file: {:?}", crate::log::path()));
    crate::log::write(&format!(
        "config dir: {:?}",
        AppConfig::config_dir()
    ));

    let saved = WindowState::load();
    let (size, position) = match &saved {
        Some(w) => (
            Size::new(w.width, w.height),
            Position::Specific(Point::new(w.x, w.y)),
        ),
        None => (Size::new(1150.0, 850.0), Position::Default),
    };

    let _ = started_minimized;

    App::run(iced::Settings {
        window: iced::window::Settings {
            transparent: true,
            decorations: false,
            level: iced::window::Level::Normal,
            resizable: true,
            visible: false,
            size,
            position,
            min_size: Some(iced::Size::new(360.0, 420.0)),
            ..Default::default()
        },
        ..Default::default()
    })
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        tray::init();
        let today = chrono::Local::now().date_naive();
        let autostart_enabled = autostart::is_autostart_enabled();
        autostart::heal_autostart();
        crate::log::write(&format!("autostart enabled: {}", autostart_enabled));
        let started_minimized = std::env::args().any(|a| a == "--minimized");
        let palette = ColorPalette::standard();

        let saved = WindowState::load();
        let (window_size, window_position, dark_mode) = match saved {
            Some(w) => (
                Size::new(w.width, w.height),
                Point::new(w.x, w.y),
                w.dark_mode,
            ),
            None => (Size::new(1150.0, 850.0), Point::new(0.0, 0.0), false),
        };
        let initial_theme = if dark_mode {
            AppTheme::dark()
        } else {
            AppTheme::light()
        };

        let loaded_config = AppConfig::load();
        crate::log::write(&format!("config loaded: {}", loaded_config.is_some()));

        let (config, initial_state, setup_form) = match loaded_config {
            Some(cfg) => (
                cfg,
                app::AppState::WaitingAuth,
                app::SetupForm::default(),
            ),
            None => (
                AppConfig {
                    client_id: String::new(),
                    client_secret: String::new(),
                    calendar_id: "primary".into(),
                },
                app::AppState::Setup,
                app::SetupForm {
                    calendar_id: "primary".into(),
                    ..Default::default()
                },
            ),
        };

        let platform_cmd = Command::perform(
            async {
                #[cfg(target_os = "windows")]
                {
                    for _ in 0..100 {
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        if crate::platform::windows::find_hwnd("Google Calendar Widget").is_some() {
                            break;
                        }
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            },
            |_| Message::ApplyWindowEffects,
        );

        let startup_cmd = if matches!(initial_state, app::AppState::Setup) {
            Command::batch(vec![platform_cmd])
        } else {
            let client_id = config.client_id.clone();
            let client_secret = config.client_secret.clone();
            let existing_refresh = auth::oauth::load_refresh_token();
            crate::log::write(&format!(
                "existing refresh token: {}",
                existing_refresh.is_some()
            ));

            let auth_cmd = if let Some(rt) = existing_refresh {
                Command::perform(
                    async move {
                        auth::oauth::refresh_access_token(&client_id, &client_secret, &rt)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::TokenPolled,
                )
            } else {
                Command::perform(
                    async move {
                        auth::oauth::run_full_auth_flow(&client_id, &client_secret)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::TokenPolled,
                )
            };

            Command::batch(vec![auth_cmd, platform_cmd])
        };

        (
            Self {
                config,
                palette,
                theme: initial_theme,
                state: initial_state,
                state_before_setup: None,
                setup_form,
                selected: today,
                window_size,
                window_position,
                last_cursor: Point::new(0.0, 0.0),
                resize_start: None,
                pending_window_save: false,
                bg_alpha: 0.65,
                window_alpha: 0.75,
                autostart_enabled,
                started_minimized,
                menu_open: false,
                access_token: None,
                expires_at: 0,
                last_focus_fetch: None,
                form: None,
                saving: false,
                last_error: None,
            },
            startup_cmd,
        )
    }

    fn title(&self) -> String {
        "Google Calendar Widget".into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let events = iced::event::listen_with(|event, _status| match event {
            iced::Event::Window(_id, iced::window::Event::Resized { width, height }) => {
                Some(Message::WindowResized(Size::new(width as f32, height as f32)))
            }
            iced::Event::Window(_id, iced::window::Event::Moved { x, y }) => {
                Some(Message::WindowMoved(Point::new(x as f32, y as f32)))
            }
            iced::Event::Window(_id, iced::window::Event::Focused) => Some(Message::WindowFocused),
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Some(Message::CursorMoved(position))
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                Some(Message::ResizeEnded)
            }
            _ => None,
        });

        let bottom_tick = iced::time::every(std::time::Duration::from_secs(2))
            .map(|_| Message::KeepAtBottom);

        let tray_tick = iced::time::every(std::time::Duration::from_millis(100))
            .map(|_| Message::PollTray);

        let keyboard = iced::keyboard::on_key_press(|key, _modifiers| match key {
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                Some(Message::CloseForm)
            }
            _ => None,
        });

        Subscription::batch(vec![events, bottom_tick, tray_tick, keyboard])
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        self.update(message)
    }

    fn view(&self) -> Element<'_, Message> {
        view::view(self)
    }

    fn theme(&self) -> Theme {
        if self.theme.is_dark {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}