//! Event create/edit form.
//!
//! Renders different fields depending on the mode:
//!   - Create: title, date range, optional time, color, all-day toggle,
//!     recurrence toggle (frequency / interval / optional until).
//!   - Edit: same but recurrence is disabled; instead, when editing an
//!     instance of a recurring event, a scope selector (Only this / All /
//!     This and following) is shown. Delete goes through a two-step
//!     confirmation which, for recurring instances, also lets the user pick
//!     the delete scope.

use crate::api::colors::ColorPalette;
use crate::app::{EditScope, EventForm, FormMode, RecurFreq};
use crate::messages::Message;
use crate::ui::common::parse_hex_color;
use crate::ui::AppTheme;
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{button, checkbox, column, container, mouse_area, row, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};

/// Small selectable pill used for recurrence frequency and edit/delete scope.
fn pill<'a>(
    label: &'a str,
    selected: bool,
    on_press: Message,
    theme: &'a AppTheme,
) -> Element<'a, Message> {
    let bg_color = if selected { theme.accent } else { Color::TRANSPARENT };
    let fg_color = if selected { Color::WHITE } else { theme.text };
    let border_color = if selected { theme.accent } else { theme.text_muted };
    let inner: Element<Message> = container(text(label).size(12).style(fg_color))
        .padding([3, 8])
        .style(move |_theme: &Theme| ContainerAppearance {
            text_color: Some(fg_color),
            background: Some(Background::Color(bg_color)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow: Default::default(),
        })
        .into();
    mouse_area(inner).on_press(on_press).into()
}

pub fn view_form<'a>(
    form: &'a EventForm,
    palette: &'a ColorPalette,
    theme: &'a AppTheme,
    saving: bool,
) -> Element<'a, Message> {
    let t = theme.text;
    let is_create = matches!(form.mode, FormMode::Create);
    let is_recurring_instance = form.recurring_event_id.is_some();

    let title_label: Element<Message> = text("Title:").style(t).into();
    let title_input: Element<Message> = text_input("Event title", &form.title)
        .on_input(Message::FormTitleChanged)
        .width(Length::Fill)
        .into();
    let row1: Element<Message> = row(vec![title_label, title_input]).spacing(8).into();

    let start_date_label: Element<Message> = text("Date:").style(t).into();
    let start_date_input: Element<Message> = text_input("YYYY-MM-DD", &form.date)
        .on_input(Message::FormDateChanged)
        .width(Length::Fixed(140.0))
        .into();
    let end_date_label: Element<Message> = text("End date:").style(t).into();
    let end_date_input: Element<Message> = text_input("YYYY-MM-DD", &form.end_date)
        .on_input(Message::FormEndDateChanged)
        .width(Length::Fixed(140.0))
        .into();
    let row2: Element<Message> = row(vec![
        start_date_label,
        start_date_input,
        end_date_label,
        end_date_input,
    ])
    .spacing(8)
    .into();

    let all_day_box: Element<Message> = checkbox("All day", form.all_day)
        .on_toggle(Message::FormAllDayToggled)
        .into();
    let row2b: Element<Message> = row(vec![all_day_box]).spacing(8).into();

    // Time inputs are hidden for all-day events.
    let row3: Element<Message> = if form.all_day {
        row(vec![]).spacing(0).into()
    } else {
        let start_label: Element<Message> = text("Start:").style(t).into();
        let start_input: Element<Message> = text_input("HH:MM", &form.start_time)
            .on_input(Message::FormStartChanged)
            .width(Length::Fixed(90.0))
            .into();
        let end_label: Element<Message> = text("End:").style(t).into();
        let end_input: Element<Message> = text_input("HH:MM", &form.end_time)
            .on_input(Message::FormEndChanged)
            .width(Length::Fixed(90.0))
            .into();
        row(vec![start_label, start_input, end_label, end_input])
            .spacing(8)
            .into()
    };

    let color_label: Element<Message> = text("Color:").style(t).into();

    // Color swatches, sorted numerically by id ("1".."11"). Sizes and
    // spacing are kept small so the whole row fits in the minimum window
    // width (360px) without overflow.
    let mut sorted_ids: Vec<String> = palette.event_colors.keys().cloned().collect();
    sorted_ids.sort_by_key(|s| s.parse::<u32>().unwrap_or(0));

    let mut color_row_items: Vec<Element<Message>> = vec![color_label];

    for id in sorted_ids.iter() {
        let bg = palette
            .background_for(id)
            .and_then(parse_hex_color)
            .unwrap_or(Color::from_rgb(0.8, 0.8, 0.8));
        let is_selected = form.color_id == *id;
        // Selected swatch gets a thicker, dark border.
        let border_color = if is_selected { theme.text } else { Color::TRANSPARENT };
        let border_width = if is_selected { 2.0 } else { 1.0 };

        let swatch_inner: Element<Message> = container(text(""))
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(14.0))
            .style(move |_theme: &Theme| ContainerAppearance {
                text_color: None,
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border_color,
                    width: border_width,
                    radius: 3.0.into(),
                },
                shadow: Default::default(),
            })
            .into();

        let id_clone = id.clone();
        let swatch: Element<Message> = mouse_area(swatch_inner)
            .on_press(Message::FormColorChanged(id_clone))
            .into();

        color_row_items.push(swatch);
    }

    // "X" clears the color (falls back to default gray in the renderer).
    let clear_btn: Element<Message> = button(text("X").size(11))
        .on_press(Message::FormColorChanged(String::new()))
        .padding([2, 5])
        .into();
    color_row_items.push(clear_btn);

    let row4: Element<Message> = row(color_row_items).spacing(4).into();

    let save_btn: Element<Message> = if saving {
        button(text("Saving...").size(13)).into()
    } else {
        button("Save").on_press(Message::SaveEvent).into()
    };

    let cancel_btn: Element<Message> = if saving {
        button(text("Please wait...").size(13)).into()
    } else {
        button("Cancel").on_press(Message::CloseForm).into()
    };

    let mut actions: Vec<Element<Message>> = vec![save_btn, cancel_btn];

    // Delete is only available in edit mode and until the user confirms it.
    if matches!(form.mode, FormMode::Edit(_)) && !form.confirm_delete {
        if saving {
            actions.push(text("Operation in progress...").size(13).style(t).into());
        } else {
            let del_btn: Element<Message> = button("Delete")
                .on_press(Message::RequestDeleteEvent)
                .into();
            actions.push(del_btn);
        }
    }

    let row5: Element<Message> = row(actions).spacing(8).into();

    let mut rows: Vec<Element<Message>> = Vec::new();

    // Scope selector: shown for instances of a recurring series while editing
    // (not while confirming a deletion, which has its own scope row below).
    if is_recurring_instance && !form.confirm_delete {
        let surface_alt = theme.surface_alt;
        let border_light = theme.border_light;
        let scope_label: Element<Message> =
            text("Apply changes to:").size(12).style(t).into();
        let mut items: Vec<Element<Message>> = vec![scope_label];
        for scope in EditScope::all() {
            let selected = form.edit_scope == scope;
            items.push(pill(
                scope.label(),
                selected,
                Message::FormEditScopeChanged(scope),
                theme,
            ));
        }
        let scope_row: Element<Message> = container(row(items).spacing(6))
            .padding(6)
            .width(Length::Fill)
            .style(move |_theme: &Theme| ContainerAppearance {
                text_color: None,
                background: Some(Background::Color(surface_alt)),
                border: Border {
                    color: border_light,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                shadow: Default::default(),
            })
            .into();
        rows.push(scope_row);
    }

    rows.push(row1);
    rows.push(row2);
    rows.push(row2b);
    if !form.all_day {
        rows.push(row3);
    }
    rows.push(row4);

    // Recurrence controls are only available when creating a new event.
    if is_create {
        let recur_box: Element<Message> = checkbox("Recurring", form.recurring)
            .on_toggle(Message::FormRecurringToggled)
            .into();
        rows.push(row(vec![recur_box]).spacing(8).into());

        if form.recurring {
            let every_label: Element<Message> = text("Every:").size(12).style(t).into();
            let mut freq_items: Vec<Element<Message>> = vec![every_label];
            for freq in RecurFreq::all() {
                let selected = form.recur_freq == freq;
                freq_items.push(pill(
                    freq.label(),
                    selected,
                    Message::FormRecurFreqChanged(freq),
                    theme,
                ));
            }
            rows.push(row(freq_items).spacing(6).into());

            let interval_label: Element<Message> =
                text("Interval:").size(12).style(t).into();
            let interval_input: Element<Message> =
                text_input("1", &form.recur_interval)
                    .on_input(Message::FormRecurIntervalChanged)
                    .width(Length::Fixed(60.0))
                    .into();
            let until_label: Element<Message> =
                text("Until (optional):").size(12).style(t).into();
            let until_input: Element<Message> =
                text_input("YYYY-MM-DD", &form.recur_until)
                    .on_input(Message::FormRecurUntilChanged)
                    .width(Length::Fixed(140.0))
                    .into();
            rows.push(
                row(vec![interval_label, interval_input, until_label, until_input])
                    .spacing(8)
                    .into(),
            );
        }
    }

    // Confirmation prompt when the user tries to save an event with no title.
    if form.confirm_empty_title && !saving {
        let warn: Element<Message> =
            text("Title is empty. Save anyway?").size(13).style(t).into();
        let yes: Element<Message> = button("Yes, save")
            .on_press(Message::ConfirmEmptyTitle)
            .into();
        let no: Element<Message> = button("No")
            .on_press(Message::CancelEmptyTitle)
            .into();
        rows.push(row(vec![warn, yes, no]).spacing(8).into());
    }

    // Delete confirmation: for recurring instances, offers the scope selector.
    if form.confirm_delete && !saving {
        let surface_alt = theme.surface_alt;
        let border_light = theme.border_light;
        let warn: Element<Message> =
            text("Are you sure?").size(13).style(t).into();

        let mut confirm_row_items: Vec<Element<Message>> = Vec::new();

        if is_recurring_instance {
            let scope_label: Element<Message> =
                text("Delete:").size(12).style(t).into();
            confirm_row_items.push(scope_label);
            for scope in EditScope::all() {
                let selected = form.edit_scope == scope;
                confirm_row_items.push(pill(
                    scope.label(),
                    selected,
                    Message::FormEditScopeChanged(scope),
                    theme,
                ));
            }
        }

        let yes_btn: Element<Message> = button("Yes, delete")
            .on_press(Message::DeleteEvent)
            .into();
        let no_btn: Element<Message> = button("No")
            .on_press(Message::CancelDeleteEvent)
            .into();

        let confirm_inner: Element<Message> = column(vec![
            row(vec![warn]).spacing(0).into(),
            row(confirm_row_items).spacing(6).into(),
            row(vec![yes_btn, no_btn]).spacing(8).into(),
        ])
        .spacing(6)
        .into();

        let confirm_block: Element<Message> = container(confirm_inner)
            .padding(6)
            .width(Length::Fill)
            .style(move |_theme: &Theme| ContainerAppearance {
                text_color: None,
                background: Some(Background::Color(surface_alt)),
                border: Border {
                    color: border_light,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                shadow: Default::default(),
            })
            .into();

        rows.push(confirm_block);
    }

    rows.push(row5);

    let bg = theme.form_bg;
    let border = theme.form_border;
    let text_color = theme.text;

    // Whole form sits in a rounded card.
    container(column(rows).spacing(8))
        .padding(10)
        .style(move |_theme: &Theme| ContainerAppearance {
            text_color: Some(text_color),
            background: Some(Background::Color(bg)),
            border: Border {
                color: border,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: Default::default(),
        })
        .into()
}