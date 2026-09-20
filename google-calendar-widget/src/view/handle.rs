use crate::messages::Message;
use crate::ui::AppTheme;
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{container, mouse_area, text};
use iced::{Background, Border, Color, Element, Length, Theme};

pub fn view_handle<'a>(theme: &'a AppTheme) -> Element<'a, Message> {
    let bg = if theme.is_dark {
        Color::from_rgba(0.7, 0.7, 0.8, 0.10)
    } else {
        Color::from_rgba(0.4, 0.4, 0.5, 0.15)
    };
    let border = if theme.is_dark {
        Color::from_rgba(0.7, 0.7, 0.8, 0.45)
    } else {
        Color::from_rgba(0.4, 0.4, 0.5, 0.55)
    };

    let handle: Element<Message> = mouse_area(
        container(text(""))
            .width(Length::Fixed(16.0))
            .height(Length::Fixed(16.0))
            .style(move |_theme: &Theme| ContainerAppearance {
                text_color: None,
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: 3.0.into(),
                },
                shadow: Default::default(),
            }),
    )
    .on_press(Message::StartResize)
    .into();

    container(handle)
        .width(Length::Fill)
        .height(Length::Fixed(20.0))
        .padding([2, 4])
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Bottom)
        .into()
}