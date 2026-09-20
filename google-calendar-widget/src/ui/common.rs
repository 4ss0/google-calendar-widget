use crate::api::client::CalendarEvent;
use crate::api::colors::ColorPalette;
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{column, container, mouse_area, text};
use iced::{Background, Border, Color, Element, Length, Theme};

pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    Some(Color::from_rgb(r, g, b))
}

pub fn event_colors(event: &CalendarEvent, palette: &ColorPalette) -> (Color, Color) {
    let bg = event
        .color_id
        .as_ref()
        .and_then(|id| palette.background_for(id))
        .and_then(parse_hex_color)
        .unwrap_or(Color::from_rgb(0.55, 0.55, 0.6));
    let fg = event
        .color_id
        .as_ref()
        .and_then(|id| palette.foreground_for(id))
        .and_then(parse_hex_color)
        .unwrap_or(Color::BLACK);
    (bg, fg)
}

#[derive(Debug, Clone, Copy)]
pub enum TimeFormat {
    None,
    StartOnly,
    Range,
}

#[derive(Debug, Clone, Copy)]
pub struct EventBoxStyle {
    pub font_size: u16,
    pub time_font_size: u16,
    pub time_format: TimeFormat,
    pub padding: iced::Padding,
    pub radius: f32,
    pub spacing: f32,
    pub min_height: Option<f32>,
}

impl EventBoxStyle {
    pub fn month(ef: u16) -> Self {
        Self {
            font_size: ef,
            time_font_size: 0,
            time_format: TimeFormat::None,
            padding: [2, 5].into(),
            radius: 4.0,
            spacing: 0.0,
            min_height: Some(ef as f32 + 12.0),
        }
    }

    pub fn week(ef: u16) -> Self {
        Self {
            font_size: ef,
            time_font_size: ef.saturating_sub(1),
            time_format: TimeFormat::StartOnly,
            padding: 5.0.into(),
            radius: 5.0,
            spacing: 1.0,
            min_height: None,
        }
    }

    pub fn day(ef: u16) -> Self {
        Self {
            font_size: ef + 2,
            time_font_size: ef.saturating_sub(1),
            time_format: TimeFormat::Range,
            padding: 10.0.into(),
            radius: 8.0,
            spacing: 2.0,
            min_height: None,
        }
    }
}

pub fn render_event_box<'a>(
    event: &'a CalendarEvent,
    palette: &'a ColorPalette,
    style: EventBoxStyle,
) -> Element<'a, crate::Message> {
    let (bg, fg) = event_colors(event, palette);

    let time_str: Option<String> = match style.time_format {
        TimeFormat::None => None,
        TimeFormat::StartOnly => {
            let start = event.start.with_timezone(&chrono::Local);
            Some(start.format("%H:%M").to_string())
        }
        TimeFormat::Range => {
            let start = event.start.with_timezone(&chrono::Local);
            match event.end {
                Some(e) => {
                    let end = e.with_timezone(&chrono::Local);
                    Some(format!(
                        "{} – {}",
                        start.format("%H:%M"),
                        end.format("%H:%M")
                    ))
                }
                None => Some(start.format("%H:%M").to_string()),
            }
        }
    };

    let mut content: Vec<Element<'a, crate::Message>> = Vec::new();
    if let Some(ts) = time_str {
        content.push(text(ts).size(style.time_font_size).style(fg).into());
    }
    content.push(
        text(event.summary.clone())
            .size(style.font_size)
            .style(fg)
            .into(),
    );

    let mut ev_container = container(column(content).spacing(style.spacing))
        .padding(style.padding)
        .width(Length::Fill);

    if let Some(h) = style.min_height {
        ev_container = ev_container.height(Length::Fixed(h));
    }

    let ev_box: Element<'a, crate::Message> = ev_container
        .style(move |_theme: &Theme| ContainerAppearance {
            text_color: Some(fg),
            background: Some(Background::Color(bg)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: style.radius.into(),
            },
            shadow: Default::default(),
        })
        .into();

    mouse_area(ev_box)
        .on_press(crate::Message::OpenEditForm(event.clone()))
        .into()
}