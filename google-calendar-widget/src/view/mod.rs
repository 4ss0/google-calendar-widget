//! Top-level view composition: wraps the current state's content in the app
//! shell (outer container, optional resize handle) and dispatches to the
//! specific screen based on AppState.

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

/// Outer container style: rounded, semi-transparent, subtle border.
/// `alpha` is applied to the theme background color so the desktop shows
/// through the widget.
fn outer_style(
    theme: &ui::AppTheme,
    alpha: f32,
) -> impl Fn(&Theme) -> ContainerAppearance + 'static {
    let base = theme.bg;
    let bg_color = Color::from_rgba(base.r, base.g, base.b, alpha);
    // Border derives from alpha so it fades in/out with the background.
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

/// Wraps `content` in the outer rounded container and adds the bottom-right
/// resize handle below it.
fn wrap_with_handle<'a>(
    content: Element<'a, Message>,
    theme: &'a ui::AppTheme,
    alpha: f32,
) -> Element<'a, Message> {
    let inner: Element<Message> = container(content)
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

/// Minimal top bar used only in Setup mode: title + optional Cancel (only
/// when the wizard was opened from a running session) + Close.
fn setup_top_bar<'a>(app: &'a App) -> Element<'a, Message> {
    let theme = &app.theme;

    let title: Element<Message> = container(
        text("Google Calendar Widget")
            .size(14)
            .style(theme.text_muted),
    )
    .padding([4, 6])
    .into();

    // A full-width invisible area that initiates an OS-level window drag.
    let drag_area: Element<Message> = mouse_area(
        container(text(""))
            .width(Length::Fill)
            .height(Length::Fixed(26.0)),
    )
    .on_press(Message::StartDrag)
    .into();

    // Cancel is only shown when there's a previous state to restore.
    let cancel_btn: Element<Message> = if app.state_before_setup.is_some() {
        button(text("Cancel").size(12))
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

/// Undo banner shown after deleting an event (until UNDO_WINDOW_SECS expires
/// or the user dismisses it). Returns None when there's no pending undo.
fn undo_banner<'a>(app: &'a App) -> Option<Element<'a, Message>> {
    let pending = app.pending_undo.as_ref()?;
    let theme = &app.theme;

    let label: Element<Message> = text(format!("Event deleted: {}", pending.event.summary))
        .size(13)
        .style(theme.text)
        .into();
    let undo_btn: Element<Message> = button(text("Undo").size(12))
        .on_press(Message::UndoDelete)
        .padding([4, 10])
        .into();
    let dismiss_btn: Element<Message> = button(text("X").size(12))
        .on_press(Message::DismissUndo)
        .padding([4, 8])
        .into();

    let inner: Element<Message> = row(vec![label, undo_btn, dismiss_btn])
        .spacing(8)
        .align_items(iced::Alignment::Center)
        .into();

    let bg = theme.surface_alt;
    let border = theme.border_light;

    Some(
        container(inner)
            .padding([6, 10])
            .width(Length::Fill)
            .style(move |_theme: &Theme| ContainerAppearance {
                text_color: None,
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                shadow: Default::default(),
            })
            .into(),
    )
}

/// Main view entry point. Dispatches on AppState:
///   - Setup: setup wizard with its own minimal top bar.
///   - WaitingAuth: "authorization in progress" screen.
///   - Loading: spinner placeholder.
///   - Ready: top bar + undo banner + form + calendar grid.
///   - Error: error message + Retry/Re-authenticate/Configure buttons.
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
        return wrap_with_handle(body.into(), theme, alpha);
    }

    let layout = app.layout();
    let top_bar = top_bar::view_top_bar(app, layout, theme);

    let content: Element<Message> = match &app.state {
        AppState::Setup => unreachable!(),
        AppState::WaitingAuth => {
            let t = theme.text;
            let retry_btn: Element<Message> = button(text("Retry").size(13))
                .on_press(Message::RetryAuth)
                .padding([6, 14])
                .into();
            let cancel_btn: Element<Message> = button(text("Cancel").size(13))
                .on_press(Message::CancelAuth)
                .padding([6, 14])
                .into();
            let buttons: Element<Message> =
                row(vec![retry_btn, cancel_btn]).spacing(8).into();
            column(vec![
                text("Google Calendar Authorization").size(20).style(t).into(),
                text("The browser has been opened for authorization.").style(t).into(),
                text("Complete login and grant access.").style(t).into(),
                text("Waiting for confirmation...").style(t).into(),
                container(buttons).padding([12, 0]).into(),
            ])
            .spacing(12)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        }
        AppState::Loading => container(
            text("Loading events...").size(18).style(theme.text),
        )
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
        AppState::Ready { index } => {
            let mut items: Vec<Element<Message>> = Vec::new();

            if let Some(banner) = undo_banner(app) {
                items.push(banner);
            }

            // Inline error message (e.g. failed save) above the calendar.
            if let Some(err) = &app.last_error {
                items.push(
                    text(format!("Error: {}", err))
                        .size(13)
                        .style(theme.text)
                        .into(),
                );
            }

            // The event form (create/edit) overlays the calendar inline.
            if let Some(form) = &app.form {
                items.push(form::view_form(form, &app.palette, theme, app.saving));
            }

            let drag_source_id: Option<&str> = app
                .drag
                .as_ref()
                .map(|d| d.event.id.as_str());

            // Only show a drop target while the drag is "real" (past threshold).
            let drop_target = match &app.drag {
                Some(d) if d.moved => app.hover_date,
                _ => None,
            };

            let cal: Element<Message> = ui::build_view(
                layout,
                app.selected,
                index,
                &app.palette,
                theme,
                app.window_size.width,
                app.window_size.height,
                &app.search_query,
                drag_source_id,
                drop_target,
            );
            items.push(cal);

            column(items)
                .spacing(6)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
        AppState::Error(e) => {
            let msg: Element<Message> = text(format!("Error: {}", e))
                .size(16)
                .style(theme.text)
                .into();
            // Guidance to avoid unnecessary re-auth when it's just a network issue.
            let hint: Element<Message> = text(
                "If the problem is a missing internet connection, click Retry once it is back. \
                 Use \"Re-authenticate\" only if you want to sign in with a different account.",
            )
            .size(12)
            .style(theme.text_muted)
            .into();
            let retry_btn: Element<Message> = button(text("Retry").size(13))
                .on_press(Message::RetryAuth)
                .padding([6, 14])
                .into();
            let reauth_btn: Element<Message> = button(text("Re-authenticate").size(13))
                .on_press(Message::Reauthenticate)
                .padding([6, 14])
                .into();
            let settings_btn: Element<Message> = button(text("Configure credentials").size(13))
                .on_press(Message::SetupReconfigure)
                .padding([6, 14])
                .into();
            let buttons: Element<Message> =
                row(vec![retry_btn, reauth_btn, settings_btn])
                    .spacing(8)
                    .into();
            column(vec![msg, hint, buttons])
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

    wrap_with_handle(body.into(), theme, alpha)
}