mod form;
mod handle;
mod setup;
mod top_bar;

use crate::app::{App, AppState};
use crate::messages::Message;
use crate::ui;
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{button, column, container, mouse_area, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

fn outer_style(
    theme: &ui::AppTheme,
    alpha: f32,
) -> impl Fn(&Theme) -> ContainerAppearance + 'static {
    let base = theme.bg;
    let bg_color = Color::from_rgba(base.r, base.g, base.b, alpha);
    let border_color = if theme.is_dark {
        Color::from_rgba(0.25, 0.25, 0.32, (alpha * 0.9).min(0.9))
    } else {
        Color::from_rgba(0.55, 0.62, 0.72, (alpha * 0.8).min(0.8))
    };
    let text_color = theme.text;
    move |_theme: &Theme| ContainerAppearance {
        text_color: Some(text_color),
        background: Some(Background::Color(bg_color)),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: Default::default(),
    }
}

fn setup_top_bar<'a>(app: &'a App) -> Element<'a, Message> {
    let theme = &app.theme;

    let title: Element<Message> = container(
        text("Google Calendar Widget")
            .size(14)
            .style(theme.text_muted),
    )
    .padding([4, 6])
    .into();

    let drag_area: Element<Message> = mouse_area(
        container(text(""))
            .width(Length::Fill)
            .height(Length::Fixed(26.0)),
    )
    .on_press(Message::StartDrag)
    .into();

    let cancel_btn: Element<Message> = if app.state_before_setup.is_some() {
        button(text("Annulla").size(12))
            .on_press(Message::SetupCancel)
            .padding([4, 10])
            .into()
    } else {
        container(text("")).width(Length::Fixed(0.0)).into()
    };

    let close_btn: Element<Message> = button(text("X").size(14))
        .on_press(Message::CloseWindow)
        .padding([4, 10])
        .into();

    row(vec![title, drag_area, cancel_btn, close_btn])
        .spacing(6)
        .align_items(iced::Alignment::Center)
        .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let theme = &app.theme;
    let alpha = app.bg_alpha;

    if matches!(app.state, AppState::Setup) {
        let top_bar = setup_top_bar(app);
        let content: Element<Message> = setup::view_setup(app, &app.setup_form);
        let body = column(vec![top_bar, content])
            .spacing(6)
            .width(Length::Fill)
            .height(Length::Fill);

        let inner: Element<Message> = container(body)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(outer_style(theme, alpha))
            .into();

        let handle_el = handle::view_handle(theme);

        return column(vec![inner, handle_el])
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    }

    let layout = ui::Layout::from_width(app.window_size.width);
    let top_bar = top_bar::view_top_bar(app, layout, theme);

    let content: Element<Message> = match &app.state {
        AppState::Setup => unreachable!(),
        AppState::WaitingAuth => {
            let t = theme.text;
            container(
                column(vec![
                    text("Autorizzazione Google Calendar").size(20).style(t).into(),
                    text("Il browser è stato aperto per l'autorizzazione.").style(t).into(),
                    text("Completa il login e autorizza l'accesso.").style(t).into(),
                    text("In attesa di conferma...").style(t).into(),
                ])
                .spacing(12),
            )
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        }
        AppState::Loading => container(
            text("Caricamento eventi...").size(18).style(theme.text),
        )
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
        AppState::Ready { index } => {
            let mut items: Vec<Element<Message>> = Vec::new();

            if let Some(err) = &app.last_error {
                items.push(
                    text(format!("Errore: {}", err))
                        .size(13)
                        .style(theme.text)
                        .into(),
                );
            }

            if let Some(form) = &app.form {
                items.push(form::view_form(form, &app.palette, theme, app.saving));
            }

            let cal: Element<Message> = ui::build_view(
                layout,
                app.selected,
                index,
                &app.palette,
                theme,
                app.window_size.width,
                app.window_size.height,
            );
            items.push(cal);

            column(items)
                .spacing(6)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
        AppState::Error(e) => {
            let msg: Element<Message> = text(format!("Errore: {}", e))
                .size(16)
                .style(theme.text)
                .into();
            let retry_btn: Element<Message> = iced::widget::button("Riprova autenticazione")
                .on_press(Message::Reauthenticate)
                .into();
            let settings_btn: Element<Message> = iced::widget::button("Configura credenziali")
                .on_press(Message::SetupReconfigure)
                .into();
            let buttons: Element<Message> =
                row(vec![retry_btn, settings_btn]).spacing(8).into();
            column(vec![msg, buttons])
                .spacing(12)
                .padding(20)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    };

    let body = column(vec![top_bar, content])
        .spacing(6)
        .width(Length::Fill)
        .height(Length::Fill);

    let inner: Element<Message> = container(body)
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(outer_style(theme, alpha))
        .into();

    let handle_el = handle::view_handle(theme);

    column(vec![inner, handle_el])
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}