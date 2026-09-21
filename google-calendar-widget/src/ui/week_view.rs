use super::common::{render_event_box, EventBoxStyle};
use super::layout::event_font;
use crate::api::client::CalendarEvent;
use crate::api::colors::ColorPalette;
use crate::app::EventIndex;
use crate::ui::AppTheme;
use chrono::{Datelike, Duration, NaiveDate};
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{column, container, mouse_area, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

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
    let today = chrono::Local::now().date_naive();
    let start = selected;
    let ef = event_font(width);

    let q: Option<&str> = if search_query.trim().is_empty() {
        None
    } else {
        Some(search_query)
    };

    let mut cols: Vec<Element<'a, crate::Message>> = Vec::new();
    for i in 0..7 {
        let date = start + Duration::days(i);
        let day_events = index.for_date_filtered(date, q);
        let is_today = date == today;
        let is_drop_target = drop_target == Some(date);
        cols.push(render_day_column(
            date,
            &day_events,
            palette,
            theme,
            is_today,
            ef,
            height,
            drag_source_id,
            is_drop_target,
        ));
    }

    row(cols).spacing(4).height(Length::Fill).into()
}

fn render_day_column<'a>(
    date: NaiveDate,
    events: &[&'a CalendarEvent],
    palette: &'a ColorPalette,
    theme: &'a AppTheme,
    is_today: bool,
    ef: u16,
    height: f32,
    drag_source_id: Option<&'a str>,
    is_drop_target: bool,
) -> Element<'a, crate::Message> {
    let weekday_name = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        [date.weekday().num_days_from_monday() as usize];

    let header_color = if is_today { Color::WHITE } else { theme.text };
    let accent = theme.accent;

    let header_bg = if is_today {
        Some(Background::Color(accent))
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
        let dragging = drag_source_id == Some(event.id.as_str());
        evs.push(render_event_box(
            event,
            palette,
            EventBoxStyle::week(ef),
            date,
            dragging,
        ));
    }

    let ev_col: Element<'a, crate::Message> = column(evs).spacing(4).into();

    let available = (height - 150.0).max(0.0);
    let event_h = (ef as f32) * 2.5 + 22.0;
    let max_events = (available / event_h).floor() as usize;

    let body: Element<'a, crate::Message> = if events.len() <= max_events {
        container(ev_col)
            .width(Length::Fill)
            .padding(4)
            .into()
    } else {
        scrollable(
            container(ev_col)
                .width(Length::Fill)
                .padding(4),
        )
        .height(Length::Fill)
        .into()
    };

    let inner_text = theme.text;
    let inner_bg = if is_drop_target {
        Color::from_rgba(0.35, 0.55, 0.90, 0.20)
    } else {
        theme.surface_alt
    };
    let inner_border = if is_drop_target {
        Color::from_rgba(0.35, 0.55, 0.90, 0.95)
    } else {
        theme.border_light
    };
    let inner_border_w = if is_drop_target { 2.0 } else { 1.0 };

    let inner: Element<'a, crate::Message> = container(column(vec![header, body]).spacing(4))
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .padding(3)
        .style(move |_theme: &Theme| ContainerAppearance {
            text_color: Some(inner_text),
            background: Some(Background::Color(inner_bg)),
            border: Border {
                color: inner_border,
                width: inner_border_w,
                radius: 8.0.into(),
            },
            shadow: Default::default(),
        })
        .into();

    mouse_area(inner)
        .on_press(crate::Message::OpenCreateFormForDate(date))
        .on_move(move |_| crate::Message::CellHover(date))
        .into()
}