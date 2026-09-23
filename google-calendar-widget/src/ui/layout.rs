//! Responsive layout selection and font-size heuristics.
//!
//! The layout is chosen purely from the window width so it can be recomputed
//! cheaply on every resize without storing extra state:
//!   - < 520px: Day view
//!   - < 900px: Week view
//!   - >= 900px: Month view
//!
//! Font sizes scale linearly with width between hard clamps.

use crate::api::colors::ColorPalette;
use crate::app::EventIndex;
use crate::ui::AppTheme;
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

/// Font size for the day number in month cells. Grows with window width.
pub fn day_font(width: f32) -> u16 {
    (12.0 + (width - 400.0) / 70.0).clamp(12.0, 24.0) as u16
}

/// Font size for event summaries and times inside the calendar grid.
pub fn event_font(width: f32) -> u16 {
    (10.0 + (width - 400.0) / 110.0).clamp(10.0, 16.0) as u16
}

/// Font size for weekday headers.
pub fn header_font(width: f32) -> u16 {
    (13.0 + (width - 400.0) / 90.0).clamp(13.0, 19.0) as u16
}

#[allow(clippy::too_many_arguments)]
pub fn build_view<'a>(
    layout: Layout,
    selected: NaiveDate,
    index: &'a EventIndex,
    palette: &'a ColorPalette,
    theme: &'a AppTheme,
    width: f32,
    height: f32,
    search_query: &str,
    drag_source_id: Option<&'a str>,
    drop_target: Option<NaiveDate>,
) -> Element<'a, crate::Message> {
    match layout {
        Layout::Month => super::month_view::build_view(
            selected,
            index,
            palette,
            theme,
            width,
            height,
            search_query,
            drag_source_id,
            drop_target,
        ),
        Layout::Week => super::week_view::build_view(
            selected,
            index,
            palette,
            theme,
            width,
            height,
            search_query,
            drag_source_id,
            drop_target,
        ),
        Layout::Day => super::day_view::build_view(
            selected,
            index,
            palette,
            theme,
            width,
            height,
            search_query,
        ),
    }
}