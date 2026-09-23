//! Month grid: 7 columns, up to 6 rows.

use super::common::{render_event_box, EventBoxStyle};
use super::layout::{day_font, event_font, header_font};
use crate::api::client::CalendarEvent;
use crate::api::colors::ColorPalette;
use crate::app::EventIndex;
use crate::ui::AppTheme;
use chrono::{Datelike, NaiveDate};
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{column, container, mouse_area, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

#[allow(clippy::too_many_arguments)]
pub fn build_view<'a>(
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
    let year = selected.year();
    let month = selected.month();
    let today = chrono::Local::now().date_naive();
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let total_days = days_in_month(year, month);
    let start_weekday = first_day.weekday().num_days_from_monday();
    let weeks_needed = ((start_weekday + total_days + 6) / 7) as u32;

    let q: Option<&str> = if search_query.trim().is_empty() {
        None
    } else {
        Some(search_query)
    };

    let df = day_font(width);
    let ef = event_font(width);
    let hf = header_font(width);

    let chrome = 130.0;
    let weeks_f = weeks_needed as f32;
    let cell_h = ((height - chrome) / weeks_f).max(40.0);

    let day_label_h = df as f32 + 8.0;
    let spacing = 3.0;

    // Space available for event boxes inside the cell.
    let avail_for_events = (cell_h - day_label_h - 8.0).max(0.0);

    // One-line event height (matches the minimum EventBoxStyle::month will use).
    let one_line_min = (ef as f32) * 1.3 + 6.0;

    // How many one-line boxes fit. Clamped so the layout stays readable.
    let max_events = ((avail_for_events + spacing) / (one_line_min + spacing))
        .floor()
        .clamp(1.0, 6.0) as usize;

    // Per-box height the style will use to pick its layout.
    let avail_per_event = if max_events > 0 {
        (avail_for_events - spacing * (max_events as f32 - 1.0)) / max_events as f32
    } else {
        avail_for_events
    };

    let weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let header: Vec<Element<'a, crate::Message>> = weekdays
        .iter()
        .enumerate()
        .map(|(i, w)| -> Element<'a, crate::Message> {
            let c = if i >= 5 { theme.text_dim } else { theme.text_muted };
            container(text(*w).size(hf).style(c))
                .width(Length::FillPortion(1))
                .padding(4)
                .center_x()
                .into()
        })
        .collect();

    let mut current_day = 1u32;
    let mut weeks: Vec<Element<'a, crate::Message>> = Vec::new();

    for week in 0..weeks_needed {
        let mut days_row: Vec<Element<'a, crate::Message>> = Vec::new();
        for weekday in 0..7 {
            if (week == 0 && weekday < start_weekday) || current_day > total_days {
                let empty: Element<'a, crate::Message> = container(text(""))
                    .width(Length::FillPortion(1))
                    .height(Length::Fill)
                    .into();
                days_row.push(empty);
            } else {
                let date = NaiveDate::from_ymd_opt(year, month, current_day).unwrap();
                let day_events = index.for_date_filtered(date, q);
                let is_today = date == today;
                let is_weekend = weekday >= 5;
                let is_drop_target = drop_target == Some(date);
                days_row.push(render_day(
                    date,
                    current_day,
                    &day_events,
                    palette,
                    theme,
                    is_today,
                    is_weekend,
                    df,
                    ef,
                    max_events,
                    avail_per_event,
                    drag_source_id,
                    is_drop_target,
                ));
                current_day += 1;
            }
        }
        let week_row: Element<'a, crate::Message> =
            row(days_row).spacing(3).height(Length::FillPortion(1)).into();
        weeks.push(week_row);
    }

    let header_row: Element<'a, crate::Message> = row(header).spacing(3).into();
    let weeks_col: Element<'a, crate::Message> =
        column(weeks).spacing(3).height(Length::Fill).into();
    column(vec![header_row, weeks_col])
        .spacing(6)
        .height(Length::Fill)
        .into()
}

#[allow(clippy::too_many_arguments)]
fn render_day<'a>(
    date: NaiveDate,
    day: u32,
    events: &[&'a CalendarEvent],
    palette: &'a ColorPalette,
    theme: &'a AppTheme,
    is_today: bool,
    is_weekend: bool,
    df: u16,
    ef: u16,
    max_events: usize,
    avail_per_event: f32,
    drag_source_id: Option<&'a str>,
    is_drop_target: bool,
) -> Element<'a, crate::Message> {
    let day_num_color = if is_today {
        Color::WHITE
    } else if is_weekend {
        theme.text_dim
    } else {
        theme.text
    };

    let today_bg = theme.accent;
    let day_label: Element<'a, crate::Message> = if is_today {
        container(text(day.to_string()).size(df).style(Color::WHITE))
            .padding([1, 6])
            .style(move |_theme: &Theme| ContainerAppearance {
                text_color: Some(Color::WHITE),
                background: Some(Background::Color(today_bg)),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 10.0.into(),
                },
                shadow: Default::default(),
            })
            .into()
    } else {
        container(text(day.to_string()).size(df).style(day_num_color))
            .padding([1, 4])
            .into()
    };

    let mut day_children: Vec<Element<'a, crate::Message>> = vec![day_label];

    // Reserve a row for the "+N" badge only when we're actually showing 2+
    // events; with max_events == 1 we prefer to show the event itself.
    let show_more = events.len() > max_events && max_events >= 2;
    let shown = if show_more {
        max_events - 1
    } else {
        events.len().min(max_events)
    };

    for event in events.iter().take(shown) {
        let dragging = drag_source_id == Some(event.id.as_str());
        day_children.push(render_event_box(
            event,
            palette,
            EventBoxStyle::month(ef, avail_per_event),
            date,
            dragging,
        ));
    }

    if show_more {
        let more: Element<'a, crate::Message> = text(format!("+{}", events.len() - shown))
            .size(ef.saturating_sub(1))
            .style(theme.text_dim)
            .into();
        day_children.push(more);
    }

    let day_col: Element<'a, crate::Message> = column(day_children).spacing(3).into();

    let cell_bg = if is_drop_target {
        Color::from_rgba(0.35, 0.55, 0.90, 0.20)
    } else if is_today {
        theme.today_bg
    } else if is_weekend {
        theme.cell_bg_weekend
    } else {
        theme.cell_bg
    };

    let cell_border = if is_drop_target {
        Color::from_rgba(0.35, 0.55, 0.90, 0.95)
    } else if is_today {
        theme.today_border
    } else {
        theme.border_light
    };

    let cell_border_w = if is_drop_target { 2.0 } else { 1.0 };
    let cell_text = theme.text;

    let cell: Element<'a, crate::Message> = container(day_col)
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .padding(4)
        .style(move |_theme: &Theme| ContainerAppearance {
            text_color: Some(cell_text),
            background: Some(Background::Color(cell_bg)),
            border: Border {
                color: cell_border,
                width: cell_border_w,
                radius: 6.0.into(),
            },
            shadow: Default::default(),
        })
        .into();

    mouse_area(cell)
        .on_press(crate::Message::OpenCreateFormForDate(date))
        .on_move(move |_| crate::Message::CellHover(date))
        .into()
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}