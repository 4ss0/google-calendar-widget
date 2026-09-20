// src/ui/widget.rs
use iced::window;
use iced::Settings;

pub fn widget_settings() -> Settings {
    Settings {
        window: window::Settings {
            transparent: true,
            decorations: false,
            always_on_top: true,
            resizable: false,
            size: iced::Size::new(400.0, 500.0),
            ..Default::default()
        },
        ..Default::default()
    }
}