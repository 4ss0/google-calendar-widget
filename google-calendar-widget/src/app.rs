use crate::api::client::CalendarEvent;
use crate::api::colors::ColorPalette;
use crate::config::{AppConfig, StoredToken};
use crate::ui::AppTheme;
use chrono::NaiveDate;
use iced::{Point, Size};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum FormMode {
    Create,
    Edit(String),
}

#[derive(Debug, Clone)]
pub struct EventForm {
    pub mode: FormMode,
    pub title: String,
    pub date: String,
    pub end_date: String,
    pub start_time: String,
    pub end_time: String,
    pub color_id: String,
    pub confirm_delete: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SetupForm {
    pub client_id: String,
    pub client_secret: String,
    pub calendar_id: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ApiResult<T> {
    pub new_token: Option<StoredToken>,
    pub result: Result<T, String>,
}

#[derive(Debug)]
pub struct EventIndex {
    pub events: Vec<CalendarEvent>,
    pub by_date: HashMap<NaiveDate, Vec<usize>>,
}

impl EventIndex {
    pub fn new(events: Vec<CalendarEvent>) -> Self {
        let mut by_date: HashMap<NaiveDate, Vec<usize>> = HashMap::new();
        for (i, e) in events.iter().enumerate() {
            let start_local = e.start.with_timezone(&chrono::Local);
            let start_date = start_local.date_naive();

            let end_date = match e.end {
                Some(end) => {
                    let end_local = end.with_timezone(&chrono::Local);
                    let d = end_local.date_naive();
                    if e.all_day && d > start_date {
                        d.pred_opt().unwrap_or(d)
                    } else {
                        d
                    }
                }
                None => start_date,
            };

            let mut d = start_date;
            loop {
                by_date.entry(d).or_default().push(i);
                if d >= end_date {
                    break;
                }
                match d.succ_opt() {
                    Some(next) => d = next,
                    None => break,
                }
            }
        }
        Self { events, by_date }
    }

    pub fn for_date(&self, date: NaiveDate) -> Vec<&CalendarEvent> {
        self.by_date
            .get(&date)
            .map(|idxs| idxs.iter().map(|i| &self.events[*i]).collect())
            .unwrap_or_default()
    }
}

pub enum AppState {
    Setup,
    WaitingAuth,
    Loading,
    Ready {
        index: EventIndex,
    },
    Error(String),
}

pub struct App {
    pub config: AppConfig,
    pub palette: ColorPalette,
    pub theme: AppTheme,
    pub state: AppState,
    pub state_before_setup: Option<AppState>,
    pub setup_form: SetupForm,
    pub selected: NaiveDate,
    pub window_size: Size,
    pub window_position: Point,
    pub last_cursor: Point,
    pub resize_start: Option<(f32, f32, Size)>,
    pub pending_window_save: bool,
    pub bg_alpha: f32,
    pub window_alpha: f32,
    pub autostart_enabled: bool,
    pub started_minimized: bool,
    pub menu_open: bool,
    pub access_token: Option<String>,
    pub expires_at: i64,
    pub last_focus_fetch: Option<std::time::Instant>,
    pub form: Option<EventForm>,
    pub saving: bool,
    pub last_error: Option<String>,
}