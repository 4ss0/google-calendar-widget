use super::common::{render_event_box, EventBoxStyle};
use super::layout::event_font;
use crate::api::colors::ColorPalette;
use crate::app::EventIndex;
use crate::ui::AppTheme;
use chrono::NaiveDate;
use iced::widget::{column, container, mouse_area, scrollable, text};
use iced::{Element, Length};

pub fn build_view<'a>(
    selected: NaiveDate,
    index: &'a EventIndex,
    palette: &'a ColorPalette,
    theme: &'a AppTheme,
    width: f32,
    height: f32,
) -> Element<'a, crate::Message> {
    let ef = event_font(width);
    let day_events = index.for_date(selected);

    let title = selected.format("%A %d %B %Y").to_string();

    let header: Element<'a, crate::Message> = container(
        text(title).size(ef + 6).style(theme.text),
    )
    .padding(8)
    .width(Length::Fill)
    .center_x()
    .into();

    let mut evs: Vec<Element<'a, crate::Message>> = Vec::new();
    if day_events.is_empty() {
        evs.push(
            container(
                text("Nessun evento")
                    .size(ef)
                    .style(theme.text_muted),
            )
            .padding(20)
            .width(Length::Fill)
            .center_x()
            .into(),
        );
    } else {
        for event in day_events.iter() {
            evs.push(render_event_box(event, palette, EventBoxStyle::day(ef)));
        }
    }

    let ev_col: Element<'a, crate::Message> = column(evs).spacing(6).into();

    let available = (height - 140.0).max(0.0);
    let event_h = (ef as f32) * 3.0 + 40.0;
    let max_events = (available / event_h).floor() as usize;

    let needs_scroll = !day_events.is_empty() && day_events.len() > max_events;

    let body: Element<'a, crate::Message> = if needs_scroll {
        scrollable(
            container(ev_col)
                .width(Length::Fill)
                .padding(6),
        )
        .height(Length::Fill)
        .into()
    } else {
        container(ev_col)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(6)
            .into()
    };

    let content: Element<'a, crate::Message> = column(vec![header, body])
        .spacing(6)
        .height(Length::Fill)
        .into();

    mouse_area(content)
        .on_press(crate::Message::OpenCreateFormForDate(selected))
        .into()
}