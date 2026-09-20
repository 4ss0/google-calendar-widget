use std::sync::OnceLock;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIconBuilder, TrayIconEvent};

use crate::messages::Message;

#[derive(Debug, Clone)]
pub enum TrayMessage {
    Show,
    Hide,
    Quit,
}

struct TrayIds {
    show: MenuId,
    hide: MenuId,
    quit: MenuId,
}

static TRAY_IDS: OnceLock<TrayIds> = OnceLock::new();

pub fn init() {
    let menu = Menu::new();
    let show_item = MenuItem::new("Show", true, None);
    let hide_item = MenuItem::new("Hide", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let ids = TrayIds {
        show: show_item.id().clone(),
        hide: hide_item.id().clone(),
        quit: quit_item.id().clone(),
    };
    let _ = TRAY_IDS.set(ids);

    let _ = menu.append_items(&[
        &show_item,
        &hide_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ]);

    let icon = create_icon();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Google Calendar Widget")
        .with_icon(icon)
        .build();

    if let Ok(tray) = tray {
        std::mem::forget(tray);
    }

    let _ = TrayIconEvent::receiver();
    let _ = MenuEvent::receiver();
}

fn create_icon() -> tray_icon::Icon {
    let width: u32 = 32;
    let height: u32 = 32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..(width * height) {
        rgba.push(66);
        rgba.push(133);
        rgba.push(244);
        rgba.push(255);
    }
    tray_icon::Icon::from_rgba(rgba, width, height).unwrap()
}

pub fn handle_menu_event(event: MenuEvent) -> Option<Message> {
    let ids = TRAY_IDS.get()?;
    if event.id == ids.show {
        return Some(Message::TrayEvent(TrayMessage::Show));
    }
    if event.id == ids.hide {
        return Some(Message::TrayEvent(TrayMessage::Hide));
    }
    if event.id == ids.quit {
        return Some(Message::TrayEvent(TrayMessage::Quit));
    }
    None
}