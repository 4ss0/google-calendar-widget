use super::layout::{day_font, event_font, header_font};
use crate::api::client::CalendarEvent;
use crate::api::colors::ColorPalette;
use chrono::{Datelike, NaiveDate};
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{column, container, mouse_area, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

pub fn build_view<'a>(
    selected: NaiveDate,
    events: &'a [CalendarEvent],
    palette: &'a ColorPalette,
    width: f32,
) -> Element<'a, crate::Message> {
    let year = selected.year();
    let month = selected.month();
    let today = chrono::Local::now().date_naive();
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let total_days = days_in_month(year, month);
    let start_weekday = first_day.weekday().num_days_from_monday();
    let weeks_needed = ((start_weekday + total_days + 6) / 7) as u32;

    let df = day_font(width);
    let ef = event_font(width);
    let hf = header_font(width);

    let weekdays = ["Lun", "Mar", "Mer", "Gio", "Ven", "Sab", "Dom"];
    let header: Vec<Element<'a, crate::Message>> = weekdays
        .iter()
        .enumerate()
        .map(|(i, w)| -> Element<'a, crate::Message> {
            let c = if i >= 5 {
                Color::from_rgb(0.55, 0.55, 0.6)
            } else {
                Color::from_rgb(0.3, 0.3, 0.35)
            };
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
                let day_events: Vec<&CalendarEvent> = events
                    .iter()
                    .filter(|e| e.start.with_timezone(&chrono::Local).date_naive() == date)
                    .collect();
                let is_today = date == today;
                let is_weekend = weekday >= 5;
                days_row.push(render_day(
                    current_day,
                    &day_events,
                    palette,
                    is_today,
                    is_weekend,
                    df,
                    ef,
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

fn render_day<'a>(
    day: u32,
    events: &[&CalendarEvent],
    palette: &'a ColorPalette,
    is_today: bool,
    is_weekend: bool,
    df: u16,
    ef: u16,
) -> Element<'a, crate::Message> {
    let day_num_color = if is_today {
        Color::WHITE
    } else if is_weekend {
        Color::from_rgb(0.45, 0.45, 0.5)
    } else {
        Color::from_rgb(0.2, 0.2, 0.25)
    };

    let day_label: Element<'a, crate::Message> = if is_today {
        container(text(day.to_string()).size(df).style(Color::WHITE))
            .padding([1, 6])
            .style(|_theme: &Theme| ContainerAppearance {
                text_color: Some(Color::WHITE),
                background: Some(Background::Color(Color::from_rgb(0.26, 0.52, 0.96))),
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

    for event in events.iter().take(3) {
        let bg_color = event
            .color_id
            .as_ref()
            .and_then(|id| palette.background_for(id))
            .and_then(parse_hex_color)
            .unwrap_or(Color::from_rgb(0.55, 0.55, 0.6));

        let fg_color = event
            .color_id
            .as_ref()
            .and_then(|id| palette.foreground_for(id))
            .and_then(parse_hex_color)
            .unwrap_or(Color::BLACK);

        let ev_text: Element<'a, crate::Message> = text(event.summary.clone())
            .size(ef)
            .style(fg_color)
            .into();

        let ev_container: Element<'a, crate::Message> = container(ev_text)
            .style(move |_theme: &Theme| ContainerAppearance {
                text_color: Some(fg_color),
                background: Some(Background::Color(bg_color)),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 4.0.into(),
                },
                shadow: Default::default(),
            })
            .padding([2, 5])
            .width(Length::Fill)
            .into();

        let ev_clickable: Element<'a, crate::Message> = mouse_area(ev_container)
            .on_press(crate::Message::OpenEditForm((*event).clone()))
            .into();

        day_children.push(ev_clickable);
    }

    if events.len() > 3 {
        let more: Element<'a, crate::Message> = text(format!("+{}", events.len() - 3))
            .size(ef.saturating_sub(2))
            .style(Color::from_rgb(0.4, 0.4, 0.45))
            .into();
        day_children.push(more);
    }

    let day_col: Element<'a, crate::Message> = column(day_children).spacing(3).into();

    let bg = if is_today {
        Color::from_rgba(0.85, 0.92, 1.0, 0.55)
    } else if is_weekend {
        Color::from_rgba(0.94, 0.94, 0.97, 0.4)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.5)
    };

    let border_color = if is_today {
        Color::from_rgba(0.26, 0.52, 0.96, 0.7)
    } else {
        Color::from_rgba(0.75, 0.75, 0.8, 0.4)
    };

    container(day_col)
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .padding(4)
        .style(move |_theme: &Theme| ContainerAppearance {
            text_color: Some(Color::BLACK),
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 6.0.into(),
            },
            shadow: Default::default(),
        })
        .into()
}

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