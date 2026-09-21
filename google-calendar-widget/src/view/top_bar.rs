//! Application top bar. Two variants:
//!   - Wide (>= 800px): all controls on a single row.
//!   - Narrow (< 800px): compact row plus a collapsible menu (toggle button)
//!     that reveals search, transparency, autostart, etc.
//!
//! The bar includes an invisible full-width drag area so the user can move
//! the window by clicking empty space (there's no OS title bar).

use crate::app::App;
use crate::messages::Message;
use crate::ui::{self, AppTheme};
use chrono::Duration;
use iced::widget::{button, container, mouse_area, row, text, text_input};
use iced::{Element, Length};

pub fn view_top_bar<'a>(
    app: &'a App,
    layout: ui::Layout,
    theme: &'a AppTheme,
) -> Element<'a, Message> {
    let is_narrow = app.window_size.width < 800.0;

    // Two title formats: long for the wide bar, short for the narrow one.
    let title_long = match layout {
        ui::Layout::Month => app.selected.format("%B %Y").to_string(),
        ui::Layout::Week => {
            let start = app.selected;
            let end = start + Duration::days(6);
            format!("{} – {}", start.format("%d %b"), end.format("%d %b %Y"))
        }
        ui::Layout::Day => app.selected.format("%A %d %B %Y").to_string(),
    };

    let title_short = match layout {
        ui::Layout::Month => app.selected.format("%m/%y").to_string(),
        ui::Layout::Week => {
            let start = app.selected;
            let end = start + Duration::days(6);
            format!("{}–{}", start.format("%d/%m"), end.format("%d/%m"))
        }
        ui::Layout::Day => app.selected.format("%d/%m").to_string(),
    };

    // Label shows the theme you'll switch TO, not the current one.
    let theme_label = if app.theme.is_dark { "Light" } else { "Dark" };

    let prev_btn: Element<Message> = button(text("<").size(16))
        .on_press(Message::Prev)
        .padding([2, 10])
        .into();
    let next_btn: Element<Message> = button(text(">").size(16))
        .on_press(Message::Next)
        .padding([2, 10])
        .into();

    let today_btn: Element<Message> = button(text("Today").size(13))
        .on_press(Message::Today)
        .padding([4, 10])
        .into();
    let new_btn: Element<Message> = button(text("+").size(18))
        .on_press(Message::OpenCreateForm)
        .padding([2, 12])
        .into();

    // Transparency controls: window_alpha is decremented/incremented by 0.08
    // per click. "+" decreases transparency (more opaque), "-" increases it.
    let less_transp_btn: Element<Message> = button(text("+").size(14))
        .on_press(Message::DecreaseTransparency)
        .padding([2, 8])
        .into();
    let more_transp_btn: Element<Message> = button(text("-").size(14))
        .on_press(Message::IncreaseTransparency)
        .padding([2, 8])
        .into();
    let opacity_lbl: Element<Message> = container(
        text(format!("{:.0}%", app.window_alpha * 100.0))
            .size(12)
            .style(theme.text),
    )
    .padding([4, 4])
    .into();

    let autostart_label = if app.autostart_enabled {
        "Auto: ON"
    } else {
        "Auto: OFF"
    };
    let autostart_btn: Element<Message> = button(text(autostart_label).size(11))
        .on_press(Message::ToggleAutostart)
        .padding([4, 8])
        .into();

    let theme_btn: Element<Message> = button(text(theme_label).size(11))
        .on_press(Message::ToggleTheme)
        .padding([4, 8])
        .into();

    let settings_btn: Element<Message> = button(text("Cfg").size(11))
        .on_press(Message::SetupReconfigure)
        .padding([4, 8])
        .into();

    let close_btn: Element<Message> = button(text("X").size(14))
        .on_press(Message::CloseWindow)
        .padding([4, 10])
        .into();

    let search_input: Element<Message> = text_input("Search…", &app.search_query)
        .on_input(Message::SearchQueryChanged)
        .width(Length::Fixed(150.0))
        .padding([4, 6])
        .into();

    // The clear button only takes up space when there's something to clear.
    let search_clear_btn: Element<Message> = if app.search_query.is_empty() {
        container(text("")).width(Length::Fixed(0.0)).into()
    } else {
        button(text("×").size(14))
            .on_press(Message::SearchClear)
            .padding([2, 8])
            .into()
    };

    // Invisible full-width drag area. Windows handles the actual move
    // (Message::StartDrag -> iced::window::drag).
    let drag_area: Element<Message> = mouse_area(
        container(text(""))
            .width(Length::Fill)
            .height(Length::Fixed(26.0)),
    )
    .on_press(Message::StartDrag)
    .into();

    if !is_narrow {
        let title: Element<Message> = container(
            text(title_long).size(18).style(theme.text),
        )
        .padding([4, 6])
        .into();

        return row(vec![
            prev_btn,
            next_btn,
            title,
            today_btn,
            new_btn,
            search_input,
            search_clear_btn,
            more_transp_btn,
            opacity_lbl,
            less_transp_btn,
            autostart_btn,
            theme_btn,
            drag_area,
            settings_btn,
            close_btn,
        ])
        .spacing(6)
        .align_items(iced::Alignment::Center)
        .into();
    }

    // Narrow variant: compact single row + optional collapsible menu.
    let title: Element<Message> = container(
        text(title_short).size(15).style(theme.text),
    )
    .padding([4, 6])
    .into();

    let menu_btn: Element<Message> = if app.menu_open {
        button(text("X").size(14))
            .on_press(Message::ToggleMenu)
            .padding([4, 10])
            .into()
    } else {
        button(text("=").size(18))
            .on_press(Message::ToggleMenu)
            .padding([2, 10])
            .into()
    };

    let bar_row: Element<Message> = row(vec![
        prev_btn,
        next_btn,
        title,
        drag_area,
        theme_btn,
        settings_btn,
        menu_btn,
        close_btn,
    ])
    .spacing(6)
    .align_items(iced::Alignment::Center)
    .into();

    if !app.menu_open {
        return bar_row;
    }

    let menu_row0: Element<Message> = row(vec![search_input, search_clear_btn])
        .spacing(6)
        .padding([2, 2])
        .into();

    let menu_row1: Element<Message> = row(vec![today_btn, new_btn, autostart_btn])
        .spacing(6)
        .padding([2, 2])
        .into();

    let menu_row2: Element<Message> = row(vec![
        more_transp_btn,
        opacity_lbl,
        less_transp_btn,
    ])
    .spacing(6)
    .padding([2, 2])
    .into();

    iced::widget::column(vec![bar_row, menu_row0, menu_row1, menu_row2])
        .spacing(4)
        .into()
}