use crate::app::{App, SetupForm};
use crate::messages::Message;
use iced::widget::{button, column, container, scrollable, text, text_input};
use iced::{Color, Element, Length};

pub fn view_setup<'a>(app: &'a App, form: &'a SetupForm) -> Element<'a, Message> {
    let theme = &app.theme;
    let t = theme.text;

    let title: Element<Message> = text("Benvenuto in Google Calendar Widget")
        .size(22)
        .style(t)
        .into();

    let subtitle: Element<Message> = text(
        "Per iniziare, inserisci le credenziali OAuth della tua applicazione Google Cloud.",
    )
    .size(13)
    .style(theme.text_muted)
    .into();

    let help: Element<Message> = text(
        "Se non le hai, crea un progetto su console.cloud.google.com, abilita l'API Calendar, \
         crea credenziali OAuth di tipo \"App desktop\" e copia qui Client ID e Client Secret.",
    )
    .size(11)
    .style(theme.text_dim)
    .into();

    let client_id_label: Element<Message> = text("Client ID").size(13).style(t).into();
    let client_id_input: Element<Message> = text_input(
        "xxxxx.apps.googleusercontent.com",
        &form.client_id,
    )
    .on_input(Message::SetupClientIdChanged)
    .width(Length::Fill)
    .into();

    let client_secret_label: Element<Message> = text("Client Secret").size(13).style(t).into();
    let client_secret_input: Element<Message> = text_input(
        "GOCSPX-xxxxxxxxxxxxxxxxxxxx",
        &form.client_secret,
    )
    .on_input(Message::SetupClientSecretChanged)
    .width(Length::Fill)
    .into();

    let calendar_label: Element<Message> = text("Calendar ID (opzionale)").size(13).style(t).into();
    let calendar_input: Element<Message> = text_input("primary", &form.calendar_id)
        .on_input(Message::SetupCalendarIdChanged)
        .width(Length::Fixed(220.0))
        .into();

    let error_el: Element<Message> = if let Some(err) = &form.error {
        text(err.clone())
            .size(12)
            .style(Color::from_rgb(0.85, 0.25, 0.25))
            .into()
    } else {
        text("").size(12).into()
    };

    let submit_btn: Element<Message> = button(text("Salva e continua").size(14))
        .on_press(Message::SetupSubmit)
        .padding([8, 18])
        .into();

    let body = column(vec![
        title,
        subtitle,
        container(help).padding([4, 0]).into(),
        container(client_id_label).padding([6, 0]).into(),
        client_id_input,
        container(client_secret_label).padding([6, 0]).into(),
        client_secret_input,
        container(calendar_label).padding([6, 0]).into(),
        calendar_input,
        container(error_el).padding([4, 0]).into(),
        container(submit_btn).padding([8, 0]).into(),
    ])
    .spacing(6)
    .width(Length::Fill)
    .max_width(560.0);

    scrollable(
        container(body)
            .padding(4)
            .width(Length::Fill)
            .center_x(),
    )
    .height(Length::Fill)
    .into()
}