use crate::api::colors::ColorPalette;
use crate::app::{EventForm, FormMode};
use crate::messages::Message;
use crate::ui::common::parse_hex_color;
use crate::ui::AppTheme;
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{button, column, container, mouse_area, row, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};

pub fn view_form<'a>(
    form: &'a EventForm,
    palette: &'a ColorPalette,
    theme: &'a AppTheme,
    saving: bool,
) -> Element<'a, Message> {
    let t = theme.text;

    let title_label: Element<Message> = text("Titolo:").style(t).into();
    let title_input: Element<Message> = text_input("Titolo evento", &form.title)
        .on_input(Message::FormTitleChanged)
        .width(Length::Fill)
        .into();
    let row1: Element<Message> = row(vec![title_label, title_input]).spacing(8).into();

    let start_date_label: Element<Message> = text("Data:").style(t).into();
    let start_date_input: Element<Message> = text_input("YYYY-MM-DD", &form.date)
        .on_input(Message::FormDateChanged)
        .width(Length::Fixed(140.0))
        .into();
    let end_date_label: Element<Message> = text("Fine data:").style(t).into();
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

    let start_label: Element<Message> = text("Inizio:").style(t).into();
    let start_input: Element<Message> = text_input("HH:MM", &form.start_time)
        .on_input(Message::FormStartChanged)
        .width(Length::Fixed(90.0))
        .into();
    let end_label: Element<Message> = text("Fine:").style(t).into();
    let end_input: Element<Message> = text_input("HH:MM", &form.end_time)
        .on_input(Message::FormEndChanged)
        .width(Length::Fixed(90.0))
        .into();
    let row3: Element<Message> = row(vec![start_label, start_input, end_label, end_input])
        .spacing(8)
        .into();

    let color_label: Element<Message> = text("Colore:").style(t).into();

    let mut sorted_ids: Vec<String> = palette.event_colors.keys().cloned().collect();
    sorted_ids.sort_by_key(|s| s.parse::<u32>().unwrap_or(0));

    let mut color_row_items: Vec<Element<Message>> = vec![color_label];

    for id in sorted_ids.iter() {
        let bg = palette
            .background_for(id)
            .and_then(parse_hex_color)
            .unwrap_or(Color::from_rgb(0.8, 0.8, 0.8));
        let is_selected = form.color_id == *id;
        let border_color = if is_selected { theme.text } else { Color::TRANSPARENT };
        let border_width = if is_selected { 2.0 } else { 1.0 };

        let swatch_inner: Element<Message> = container(text(""))
            .width(Length::Fixed(22.0))
            .height(Length::Fixed(22.0))
            .style(move |_theme: &Theme| ContainerAppearance {
                text_color: None,
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border_color,
                    width: border_width,
                    radius: 4.0.into(),
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

    let clear_btn: Element<Message> = button(text("X").size(11))
        .on_press(Message::FormColorChanged(String::new()))
        .padding([2, 6])
        .into();
    color_row_items.push(clear_btn);

    let row4: Element<Message> = row(color_row_items).spacing(6).into();

    let save_btn: Element<Message> = if saving {
        button(text("Salvataggio...").size(13)).into()
    } else {
        button("Salva").on_press(Message::SaveEvent).into()
    };

    let cancel_btn: Element<Message> = if saving {
        button(text("Attendi...").size(13)).into()
    } else {
        button("Annulla").on_press(Message::CloseForm).into()
    };

    let mut actions: Vec<Element<Message>> = vec![save_btn, cancel_btn];

    if matches!(form.mode, FormMode::Edit(_)) {
        if saving {
            actions.push(text("Operazione in corso...").size(13).style(t).into());
        } else if form.confirm_delete {
            let warning: Element<Message> = text("Sei sicuro?").size(13).style(t).into();
            let yes_btn: Element<Message> = button("Sì, elimina")
                .on_press(Message::DeleteEvent)
                .into();
            let no_btn: Element<Message> =
                button("No").on_press(Message::CancelDeleteEvent).into();
            actions.push(warning);
            actions.push(yes_btn);
            actions.push(no_btn);
        } else {
            let del_btn: Element<Message> = button("Elimina")
                .on_press(Message::RequestDeleteEvent)
                .into();
            actions.push(del_btn);
        }
    }

    let row5: Element<Message> = row(actions).spacing(8).into();

    let bg = theme.form_bg;
    let border = theme.form_border;
    let text_color = theme.text;

    container(column(vec![row1, row2, row3, row4, row5]).spacing(8))
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