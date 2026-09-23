//! Shared building blocks for rendering a single event box (used by all three
//! calendar views) and color resolution helpers.

use crate::api::client::CalendarEvent;
use crate::api::colors::ColorPalette;
use chrono::NaiveDate;
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{column, container, mouse_area, row, text};
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
    /// HH:MM (month and week views).
    StartOnly,
    /// HH:MM – HH:MM (day view).
    Range,
}

#[derive(Debug, Clone, Copy)]
pub struct EventBoxStyle {
    pub font_size: u16,
    pub time_font_size: u16,
    pub time_format: TimeFormat,
    /// When true, the time shares a line with the summary (compact layout).
    pub inline_time: bool,
    pub padding: iced::Padding,
    pub radius: f32,
    pub spacing: f32,
    /// When set, the box has exactly this height. The constructor picks a
    /// height that is guaranteed to fit the chosen layout, so the text is
    /// never clipped by the container.
    pub fixed_height: Option<f32>,
    /// When false, only a colored strip is rendered (no time, no summary).
    /// Used when the available space is too small for any text.
    pub show_text: bool,
}

impl EventBoxStyle {
    /// Picks the layout that fits in `avail_h` pixels without clipping:
    ///   - `avail_h` >= two lines worth -> time above summary
    ///   - `avail_h` >= one line worth  -> time inline with summary
    ///   - otherwise                    -> plain colored strip
    pub fn month(ef: u16, avail_h: f32) -> Self {
        let one_line_min = (ef as f32) * 1.3 + 6.0;
        let two_line_min = (ef as f32) * 2.5 + 8.0;

        if avail_h < one_line_min {
            Self {
                font_size: ef,
                time_font_size: 0,
                time_format: TimeFormat::StartOnly,
                inline_time: true,
                padding: 0.0.into(),
                radius: 3.0,
                spacing: 0.0,
                fixed_height: Some(avail_h.max(3.0)),
                show_text: false,
            }
        } else if avail_h < two_line_min {
            Self {
                font_size: ef,
                time_font_size: ef.saturating_sub(1),
                time_format: TimeFormat::StartOnly,
                inline_time: true,
                padding: [1, 5].into(),
                radius: 4.0,
                spacing: 0.0,
                fixed_height: Some(avail_h),
                show_text: true,
            }
        } else {
            Self {
                font_size: ef,
                time_font_size: ef.saturating_sub(1),
                time_format: TimeFormat::StartOnly,
                inline_time: false,
                padding: [2, 5].into(),
                radius: 4.0,
                spacing: 1.0,
                fixed_height: Some(avail_h),
                show_text: true,
            }
        }
    }

    pub fn week(ef: u16) -> Self {
        Self {
            font_size: ef,
            time_font_size: ef.saturating_sub(1),
            time_format: TimeFormat::StartOnly,
            inline_time: false,
            padding: 5.0.into(),
            radius: 5.0,
            spacing: 1.0,
            fixed_height: None,
            show_text: true,
        }
    }

    pub fn day(ef: u16) -> Self {
        Self {
            font_size: ef + 2,
            time_font_size: ef.saturating_sub(1),
            time_format: TimeFormat::Range,
            inline_time: false,
            padding: 10.0.into(),
            radius: 8.0,
            spacing: 2.0,
            fixed_height: None,
            show_text: true,
        }
    }
}

pub fn render_event_box<'a>(
    event: &'a CalendarEvent,
    palette: &'a ColorPalette,
    style: EventBoxStyle,
    source_date: NaiveDate,
    dragging: bool,
) -> Element<'a, crate::Message> {
    let (bg, fg) = event_colors(event, palette);
    let drag_border = Color::from_rgba(0.35, 0.55, 0.90, 0.95);
    let dim_bg = Color { a: 0.35, ..bg };

    // Strip-only fallback: the cell is too short for any text.
    if !style.show_text {
        let h = style.fixed_height.unwrap_or(4.0).max(3.0);
        let strip: Element<'a, crate::Message> = container(text(""))
            .width(Length::Fill)
            .height(Length::Fixed(h))
            .style(move |_theme: &Theme| {
                let (b, bc, bw) = if dragging {
                    (dim_bg, drag_border, 2.0)
                } else {
                    (bg, Color::TRANSPARENT, 0.0)
                };
                ContainerAppearance {
                    text_color: None,
                    background: Some(Background::Color(b)),
                    border: Border {
                        color: bc,
                        width: bw,
                        radius: style.radius.into(),
                    },
                    shadow: Default::default(),
                }
            })
            .into();
        return mouse_area(strip)
            .on_press(crate::Message::EventMouseDown {
                event: event.clone(),
                source_date,
            })
            .on_move(move |_| crate::Message::CellHover(source_date))
            .into();
    }

    // All-day events have no meaningful wall-clock time; suppress the label.
    let time_str: Option<String> = if event.all_day {
        None
    } else {
        match style.time_format {
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
        }
    };

    let content: Element<'a, crate::Message> = match (style.inline_time, time_str) {
        (true, Some(ts)) => row(vec![
            text(ts).size(style.time_font_size).style(fg).into(),
            text(event.summary.clone())
                .size(style.font_size)
                .style(fg)
                .into(),
        ])
        .spacing(4)
        .into(),
        (_, Some(ts)) => column(vec![
            text(ts).size(style.time_font_size).style(fg).into(),
            text(event.summary.clone())
                .size(style.font_size)
                .style(fg)
                .into(),
        ])
        .spacing(style.spacing)
        .into(),
        (_, None) => column(vec![
            text(event.summary.clone())
                .size(style.font_size)
                .style(fg)
                .into(),
        ])
        .into(),
    };

    let mut ev_container = container(content)
        .padding(style.padding)
        .width(Length::Fill);

    if let Some(h) = style.fixed_height {
        ev_container = ev_container.height(Length::Fixed(h));
    }

    let ev_box: Element<'a, crate::Message> = ev_container
        .style(move |_theme: &Theme| {
            if dragging {
                ContainerAppearance {
                    text_color: Some(fg),
                    background: Some(Background::Color(dim_bg)),
                    border: Border {
                        color: drag_border,
                        width: 2.0,
                        radius: style.radius.into(),
                    },
                    shadow: Default::default(),
                }
            } else {
                ContainerAppearance {
                    text_color: Some(fg),
                    background: Some(Background::Color(bg)),
                    border: Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: style.radius.into(),
                    },
                    shadow: Default::default(),
                }
            }
        })
        .into();

    mouse_area(ev_box)
        .on_press(crate::Message::EventMouseDown {
            event: event.clone(),
            source_date,
        })
        .on_move(move |_| crate::Message::CellHover(source_date))
        .into()
}