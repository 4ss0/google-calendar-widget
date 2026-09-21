//! Crate root. Sets up the iced application, loads persisted state (config,
//! refresh token, window geometry), and wires the platform-specific startup
//! sequence (auth refresh + Win32 window tweaks).
//!
//! On Windows the binary is built as a GUI app in release (no console window).
//! The `--minimized` flag is honored so the app can start hidden when launched
//! by the autostart registry entry.

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
use crate::app::{App, AppState};
use crate::messages::Message;
use crate::ui::AppTheme;
use config::{AppConfig, WindowState};
use iced::window::Position;
use iced::{Application, Command, Element, Point, Size, Subscription, Theme};

fn main() -> iced::Result {
    // Best-effort: load a .env file if present (useful for dev, ignored in prod).
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

    // Restore the last known window geometry if available. If the previous
    // session never observed a real position (position_is_default), ask the
    // OS to choose the position again.
    let saved = WindowState::load();
    let (size, position) = match &saved {
        Some(w) => (
            Size::new(w.width, w.height),
            if w.position_is_default {
                Position::Default
            } else {
                Position::Specific(Point::new(w.x, w.y))
            },
        ),
        None => (Size::new(1150.0, 850.0), Position::Default),
    };

    // `--minimized` is used by the autostart registry entry so the widget
    // doesn't pop up at login.
    let started_minimized = std::env::args().any(|a| a == "--minimized");

    App::run(iced::Settings {
        window: iced::window::Settings {
            // The window background is painted by our own containers, not by iced.
            transparent: true,
            // Custom title bar is drawn in ui/top_bar.rs and view/mod.rs.
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

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        tray::init();

        let today = chrono::Local::now().date_naive();
        let autostart_enabled = autostart::is_autostart_enabled();
        // Rewrites the registry entry to the current .exe path if it was moved.
        autostart::heal_autostart();
        crate::log::write(&format!("autostart enabled: {}", autostart_enabled));
        let started_minimized = std::env::args().any(|a| a == "--minimized");
        let palette = ColorPalette::standard();

        // Restore theme + geometry. On first run (no saved state) fall back to
        // the system dark-mode preference on Windows. `position_known` tells
        // us whether the saved position is meaningful or just a placeholder.
        let saved = WindowState::load();
        let (window_size, window_position, dark_mode, position_known) = match saved {
            Some(w) => {
                let known = !w.position_is_default;
                (
                    Size::new(w.width, w.height),
                    Point::new(w.x, w.y),
                    w.dark_mode,
                    known,
                )
            }
            None => {
                #[cfg(target_os = "windows")]
                let d = crate::platform::windows::system_is_dark();
                #[cfg(not(target_os = "windows"))]
                let d = false;
                (Size::new(1150.0, 850.0), Point::new(0.0, 0.0), d, false)
            }
        };

        let initial_theme = if dark_mode {
            AppTheme::dark()
        } else {
            AppTheme::light()
        };

        let loaded_config = AppConfig::load();
        crate::log::write(&format!("config loaded: {}", loaded_config.is_some()));

        // Refresh token is only meaningful if credentials exist.
        let existing_refresh = if loaded_config.is_some() {
            let rt = auth::oauth::load_refresh_token();
            crate::log::write(&format!(
                "existing refresh token: {}",
                rt.is_some()
            ));
            rt
        } else {
            None
        };

        // Decide the initial screen:
        //   - No config -> Setup wizard.
        //   - Config + refresh token -> go straight to Loading (silent refresh).
        //   - Config but no refresh token -> WaitingAuth (full OAuth flow).
        let (config, initial_state, setup_form) = match loaded_config {
            Some(cfg) => {
                let state = if existing_refresh.is_some() {
                    app::AppState::Loading
                } else {
                    app::AppState::WaitingAuth
                };
                (cfg, state, app::SetupForm::default())
            }
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

        // Wait for the OS to create the HWND before applying Win32 tweaks.
        // The window is found by title ("Google Calendar Widget").
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
                    // Extra delay to let iced finish its own window setup.
                    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                }
                #[cfg(not(target_os = "windows"))]
                {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            },
            |_| Message::ApplyWindowEffects,
        );

        let startup_cmd = if matches!(initial_state, app::AppState::Setup) {
            // In setup mode there is nothing to auth yet.
            Command::batch(vec![platform_cmd])
        } else {
            let client_id = config.client_id.clone();
            let client_secret = config.client_secret.clone();

            let auth_cmd = if let Some(rt) = existing_refresh.clone() {
                // Silent refresh: no browser interaction.
                Command::perform(
                    async move {
                        auth::oauth::refresh_access_token(&client_id, &client_secret, &rt)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::TokenPolled,
                )
            } else {
                // First run with credentials but no token: full OAuth flow.
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
                window_position_known: position_known,
                last_cursor: Point::new(0.0, 0.0),
                resize_start: None,
                pending_window_save: false,
                bg_alpha: 0.65,
                window_alpha: 0.75,
                autostart_enabled,
                started_minimized,
                menu_open: false,
                access_token: None,
                refresh_token: existing_refresh,
                expires_at: 0,
                auth_retry_in_flight: false,
                last_focus_fetch: None,
                form: None,
                saving: false,
                last_error: None,
                pending_undo: None,
                next_undo_nonce: 0,
                form_source_event: None,
                search_query: String::new(),
                drag: None,
                hover_date: None,
                last_desktop_foreground: false,
            },
            startup_cmd,
        )
    }

    fn title(&self) -> String {
        "Google Calendar Widget".into()
    }

    fn subscription(&self) -> Subscription<Message> {
        // Global event stream: window resize/move/focus, cursor position, and
        // left-button release (used to terminate resize and drag operations).
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
                Some(Message::GlobalLeftUp)
            }
            _ => None,
        });

        // Periodically re-pin the window to the bottom of the z-order so it
        // stays behind normal apps but above the desktop wallpaper.
        let bottom_tick = iced::time::every(std::time::Duration::from_millis(500))
            .map(|_| Message::KeepAtBottom);

        // The tray-icon crate exposes non-Send event receivers, so we poll them
        // from the iced update loop at a low frequency.
        let tray_tick = iced::time::every(std::time::Duration::from_millis(100))
            .map(|_| Message::PollTray);

        // Auto-retry auth every 10s while in Error state.
        let auto_retry = if matches!(self.state, AppState::Error(_)) {
            iced::time::every(std::time::Duration::from_secs(10))
                .map(|_| Message::AutoRetryTick)
        } else {
            Subscription::none()
        };

        // Escape closes the event form if one is open.
        let keyboard = iced::keyboard::on_key_press(|key, _modifiers| match key {
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                Some(Message::CloseForm)
            }
            _ => None,
        });

        Subscription::batch(vec![events, bottom_tick, tray_tick, auto_retry, keyboard])
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        // Delegates to update.rs::App::update.
        self.update(message)
    }

    fn view(&self) -> Element<'_, Message> {
        view::view(self)
    }

    fn theme(&self) -> Theme {
        // iced needs a built-in theme to color built-in widgets (buttons, text
        // inputs). Our own containers use AppTheme directly.
        if self.theme.is_dark {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}