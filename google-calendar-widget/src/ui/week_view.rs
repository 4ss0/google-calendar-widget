use super::layout::event_font;
use super::month_view::parse_hex_color;
use crate::api::client::CalendarEvent;
use crate::api::colors::ColorPalette;
use chrono::{Datelike, Duration, NaiveDate};
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{column, container, mouse_area, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

pub fn build_view<'a>(
    selected: NaiveDate,
    events: &'a [CalendarEvent],
    palette: &'a ColorPalette,
    width: f32,
) -> Element<'a, crate::Message> {
    let today = chrono::Local::now().date_naive();
    let start = selected;
    let ef = event_font(width);

    let mut cols: Vec<Element<'a, crate::Message>> = Vec::new();
    for i in 0..7 {
        let date = start + Duration::days(i);
        let day_events: Vec<&CalendarEvent> = events
            .iter()
            .filter(|e| e.start.with_timezone(&chrono::Local).date_naive() == date)
            .collect();
        let is_today = date == today;
        cols.push(render_day_column(date, &day_events, palette, is_today, ef));
    }

    row(cols).spacing(4).height(Length::Fill).into()
}

fn render_day_column<'a>(
    date: NaiveDate,
    events: &[&CalendarEvent],
    palette: &'a ColorPalette,
    is_today: bool,
    ef: u16,
) -> Element<'a, crate::Message> {
    let weekday_name = ["Lun", "Mar", "Mer", "Gio", "Ven", "Sab", "Dom"]
        [date.weekday().num_days_from_monday() as usize];

    let header_color = if is_today {
        Color::WHITE
    } else {
        Color::from_rgb(0.25, 0.25, 0.3)
    };

    let header_bg = if is_today {
        Some(Background::Color(Color::from_rgb(0.26, 0.52, 0.96)))
    } else {
        None
    };

    let header: Element<'a, crate::Message> = container(
        column(vec![
            text(weekday_name).size(ef + 1).style(header_color).into(),
            text(date.day().to_string())
                .size(ef + 5)
                .style(header_color)
                .into(),
        ])
        .spacing(0)
        .align_items(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding(6)
    .center_x()
    .style(move |_theme: &Theme| ContainerAppearance {
        text_color: Some(header_color),
        background: header_bg,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 6.0.into(),
        },
        shadow: Default::default(),
    })
    .into();

    let mut evs: Vec<Element<'a, crate::Message>> = Vec::new();
    for event in events.iter() {
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

        let time_str = event
            .start
            .with_timezone(&chrono::Local)
            .format("%H:%M")
            .to_string();

        let ev_box: Element<'a, crate::Message> = container(
            column(vec![
                text(time_str)
                    .size(ef.saturating_sub(1))
                    .style(fg_color)
                    .into(),
                text(event.summary.clone()).size(ef).style(fg_color).into(),
            ])
            .spacing(1),
        )
        .padding(5)
        .width(Length::Fill)
        .style(move |_theme: &Theme| ContainerAppearance {
            text_color: Some(fg_color),
            background: Some(Background::Color(bg_color)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 5.0.into(),
            },
            shadow: Default::default(),
        })
        .into();

        let ev_click: Element<'a, crate::Message> = mouse_area(ev_box)
            .on_press(crate::Message::OpenEditForm((*event).clone()))
            .into();
        evs.push(ev_click);
    }

    let body: Element<'a, crate::Message> = scrollable(
        container(column(evs).spacing(4))
            .width(Length::Fill)
            .padding(4),
    )
    .height(Length::Fill)
    .into();

    container(column(vec![header, body]).spacing(4))
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .padding(3)
        .style(|_theme: &Theme| ContainerAppearance {
            text_color: Some(Color::BLACK),
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.5))),
            border: Border {
                color: Color::from_rgba(0.75, 0.75, 0.8, 0.4),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: Default::default(),
        })
        .into()
}