use super::layout::event_font;
use super::month_view::parse_hex_color;
use crate::api::client::CalendarEvent;
use crate::api::colors::ColorPalette;
use chrono::NaiveDate;
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{column, container, mouse_area, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

pub fn build_view<'a>(
    selected: NaiveDate,
    events: &'a [CalendarEvent],
    palette: &'a ColorPalette,
    width: f32,
) -> Element<'a, crate::Message> {
    let ef = event_font(width);
    let day_events: Vec<&CalendarEvent> = events
        .iter()
        .filter(|e| e.start.with_timezone(&chrono::Local).date_naive() == selected)
        .collect();

    let title = selected.format("%A %d %B %Y").to_string();

    let header: Element<'a, crate::Message> = container(
        text(title).size(ef + 6).style(Color::from_rgb(0.15, 0.15, 0.2)),
    )
    .padding(8)
    .width(Length::Fill)
    .center_x()
    .into();

    let mut evs: Vec<Element<'a, crate::Message>> = Vec::new();
    if day_events.is_empty() {
        evs.push(
            container(text("Nessun evento").size(ef).style(Color::from_rgb(0.5, 0.5, 0.55)))
                .padding(20)
                .width(Length::Fill)
                .center_x()
                .into(),
        );
    } else {
        for event in day_events.iter() {
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

            let start = event.start.with_timezone(&chrono::Local);
            let end = event.end.map(|e| e.with_timezone(&chrono::Local));
            let time_str = match end {
                Some(e) => format!("{} – {}", start.format("%H:%M"), e.format("%H:%M")),
                None => start.format("%H:%M").to_string(),
            };

            let ev_box: Element<'a, crate::Message> = container(
                column(vec![
                    text(time_str)
                        .size(ef.saturating_sub(1))
                        .style(fg_color)
                        .into(),
                    text(event.summary.clone())
                        .size(ef + 2)
                        .style(fg_color)
                        .into(),
                ])
                .spacing(2),
            )
            .padding(10)
            .width(Length::Fill)
            .style(move |_theme: &Theme| ContainerAppearance {
                text_color: Some(fg_color),
                background: Some(Background::Color(bg_color)),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 8.0.into(),
                },
                shadow: Default::default(),
            })
            .into();

            let ev_click: Element<'a, crate::Message> = mouse_area(ev_box)
                .on_press(crate::Message::OpenEditForm((*event).clone()))
                .into();
            evs.push(ev_click);
        }
    }

    let body: Element<'a, crate::Message> = scrollable(
        container(column(evs).spacing(6))
            .width(Length::Fill)
            .padding(6),
    )
    .height(Length::Fill)
    .into();

    column(vec![header, body])
        .spacing(6)
        .height(Length::Fill)
        .into()
}