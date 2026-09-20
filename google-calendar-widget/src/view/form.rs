use crate::api::colors::ColorPalette;
use crate::app::{EventForm, FormMode};
use crate::messages::Message;
use crate::ui::AppTheme;
use iced::widget::container::Appearance as ContainerAppearance;
use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Background, Border, Element, Length, Theme};

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

    let date_label: Element<Message> = text("Data:").style(t).into();
    let date_input: Element<Message> = text_input("YYYY-MM-DD", &form.date)
        .on_input(Message::FormDateChanged)
        .width(Length::Fixed(150.0))
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
    let row2: Element<Message> = row(vec![
        date_label,
        date_input,
        start_label,
        start_input,
        end_label,
        end_input,
    ])
    .spacing(8)
    .into();

    let color_label: Element<Message> = text("Colore:").style(t).into();
    let mut options: Vec<String> = palette.event_colors.keys().cloned().collect();
    options.sort();
    let selected = if form.color_id.is_empty() {
        None
    } else {
        Some(form.color_id.clone())
    };
    let color_picker: Element<Message> =
        pick_list(options, selected, Message::FormColorChanged).into();
    let row3: Element<Message> = row(vec![color_label, color_picker]).spacing(8).into();

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

    let row4: Element<Message> = row(actions).spacing(8).into();

    let bg = theme.form_bg;
    let border = theme.form_border;
    let text_color = theme.text;

    container(column(vec![row1, row2, row3, row4]).spacing(8))
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