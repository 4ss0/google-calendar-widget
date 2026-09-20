use crate::api::client::CalendarEvent;
use crate::api::colors::ColorPalette;
use chrono::NaiveDate;
use iced::Element;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Month,
    Week,
    Day,
}

impl Layout {
    pub fn from_width(w: f32) -> Self {
        if w < 520.0 {
            Layout::Day
        } else if w < 900.0 {
            Layout::Week
        } else {
            Layout::Month
        }
    }
}

pub fn day_font(width: f32) -> u16 {
    (10.0 + (width - 400.0) / 70.0).clamp(10.0, 22.0) as u16
}

pub fn event_font(width: f32) -> u16 {
    (8.0 + (width - 400.0) / 110.0).clamp(8.0, 14.0) as u16
}

pub fn header_font(width: f32) -> u16 {
    (12.0 + (width - 400.0) / 90.0).clamp(12.0, 18.0) as u16
}

pub fn build_view<'a>(
    layout: Layout,
    selected: NaiveDate,
    events: &'a [CalendarEvent],
    palette: &'a ColorPalette,
    width: f32,
) -> Element<'a, crate::Message> {
    match layout {
        Layout::Month => {
            super::month_view::build_view(selected, events, palette, width)
        }
        Layout::Week => super::week_view::build_view(selected, events, palette, width),
        Layout::Day => super::day_view::build_view(selected, events, palette, width),
    }
}