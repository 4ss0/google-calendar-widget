use crate::api::client::CalendarEvent;
use crate::api::colors::ColorPalette;
use crate::config::{AppConfig, StoredToken};
use crate::ui::{AppTheme, Layout};
use chrono::{DateTime, NaiveDate, Utc};
use iced::{Point, Size};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum FormMode {
    Create,
    Edit(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurFreq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl RecurFreq {
    pub fn all() -> [RecurFreq; 4] {
        [
            RecurFreq::Daily,
            RecurFreq::Weekly,
            RecurFreq::Monthly,
            RecurFreq::Yearly,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            RecurFreq::Daily => "Daily",
            RecurFreq::Weekly => "Weekly",
            RecurFreq::Monthly => "Monthly",
            RecurFreq::Yearly => "Yearly",
        }
    }

    pub fn rrule_name(&self) -> &'static str {
        match self {
            RecurFreq::Daily => "DAILY",
            RecurFreq::Weekly => "WEEKLY",
            RecurFreq::Monthly => "MONTHLY",
            RecurFreq::Yearly => "YEARLY",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditScope {
    OnlyThis,
    All,
    ThisAndFollowing,
}

impl EditScope {
    pub fn all() -> [EditScope; 3] {
        [
            EditScope::OnlyThis,
            EditScope::All,
            EditScope::ThisAndFollowing,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            EditScope::OnlyThis => "Only this",
            EditScope::All => "All events",
            EditScope::ThisAndFollowing => "This and following",
        }
    }
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
    pub all_day: bool,
    pub recurring: bool,
    pub recur_freq: RecurFreq,
    pub recur_interval: String,
    pub recur_until: String,
    pub recurring_event_id: Option<String>,
    pub original_start: Option<DateTime<Utc>>,
    pub edit_scope: EditScope,
    pub confirm_delete: bool,
    pub confirm_empty_title: bool,
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
    pub refresh_token: Option<String>,
    pub expires_at: i64,
    pub auth_retry_in_flight: bool,
    pub last_focus_fetch: Option<std::time::Instant>,
    pub form: Option<EventForm>,
    pub saving: bool,
    pub last_error: Option<String>,
}

impl App {
    pub fn layout(&self) -> Layout {
        Layout::from_width(self.window_size.width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn make(
        id: &str,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
        all_day: bool,
    ) -> CalendarEvent {
        CalendarEvent {
            id: id.into(),
            summary: id.into(),
            start,
            end: Some(end),
            color_id: None,
            all_day,
            recurring_event_id: None,
            original_start_time: None,
        }
    }

    #[test]
    fn timed_event_only_on_start_day() {
        let start = chrono::Local
            .with_ymd_and_hms(2026, 9, 21, 14, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&chrono::Utc);
        let end = start + Duration::hours(2);
        let idx = EventIndex::new(vec![make("a", start, end, false)]);
        assert_eq!(idx.for_date(NaiveDate::from_ymd_opt(2026, 9, 21).unwrap()).len(), 1);
        assert_eq!(idx.for_date(NaiveDate::from_ymd_opt(2026, 9, 22).unwrap()).len(), 0);
    }

    #[test]
    fn all_day_event_spans_days() {
        let s_local = chrono::Local
            .with_ymd_and_hms(2026, 9, 21, 0, 0, 0)
            .single()
            .unwrap();
        let e_local = chrono::Local
            .with_ymd_and_hms(2026, 9, 24, 0, 0, 0)
            .single()
            .unwrap();
        let idx = EventIndex::new(vec![make(
            "a",
            s_local.with_timezone(&chrono::Utc),
            e_local.with_timezone(&chrono::Utc),
            true,
        )]);
        for d in 21..=23 {
            assert_eq!(
                idx.for_date(NaiveDate::from_ymd_opt(2026, 9, d).unwrap()).len(),
                1,
                "expected event on 2026-09-{}",
                d
            );
        }
        assert_eq!(
            idx.for_date(NaiveDate::from_ymd_opt(2026, 9, 24).unwrap()).len(),
            0
        );
    }

    #[test]
    fn late_night_timed_event_indexed_on_local_day() {
        let start = chrono::Local
            .with_ymd_and_hms(2026, 9, 21, 0, 30, 0)
            .single()
            .unwrap()
            .with_timezone(&chrono::Utc);
        let end = start + Duration::hours(1);
        let idx = EventIndex::new(vec![make("a", start, end, false)]);
        assert_eq!(idx.for_date(NaiveDate::from_ymd_opt(2026, 9, 21).unwrap()).len(), 1);
    }

    #[test]
    fn rrule_name_maps_to_ical_tokens() {
        assert_eq!(RecurFreq::Daily.rrule_name(), "DAILY");
        assert_eq!(RecurFreq::Weekly.rrule_name(), "WEEKLY");
        assert_eq!(RecurFreq::Monthly.rrule_name(), "MONTHLY");
        assert_eq!(RecurFreq::Yearly.rrule_name(), "YEARLY");
    }

    #[test]
    fn edit_scope_labels() {
        assert_eq!(EditScope::OnlyThis.label(), "Only this");
        assert_eq!(EditScope::All.label(), "All events");
        assert_eq!(EditScope::ThisAndFollowing.label(), "This and following");
    }
}