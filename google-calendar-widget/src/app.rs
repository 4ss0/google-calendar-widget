//! Application state and pure data types shared across the update/view layers.
//!
//! The `App` struct holds everything: config, theme, current state machine
//! variant, loaded events (`EventIndex`), form state, drag state, etc.
//! `EventIndex` precomputes a date -> event index for O(1) lookups in the
//! views, which would otherwise be O(n) per rendered day.

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
    /// Edit an existing event by its instance id.
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

    /// iCalendar FREQ token used in the RRULE sent to Google.
    pub fn rrule_name(&self) -> &'static str {
        match self {
            RecurFreq::Daily => "DAILY",
            RecurFreq::Weekly => "WEEKLY",
            RecurFreq::Monthly => "MONTHLY",
            RecurFreq::Yearly => "YEARLY",
        }
    }
}

/// Which occurrences an edit/delete applies to.
/// - OnlyThis: PATCH/DELETE the single instance.
/// - All: PATCH/DELETE the master (RRULE applies to every occurrence).
/// - ThisAndFollowing: truncate the master with UNTIL and create a new series.
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

/// In-memory state of the create/edit form. Dates/times are kept as strings
/// so the user can type freely; parsing happens in handlers::handle_save.
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
    /// Set for instances of a recurring series.
    pub recurring_event_id: Option<String>,
    /// Original start of the instance being edited (needed for split logic).
    pub original_start: Option<DateTime<Utc>>,
    pub edit_scope: EditScope,
    pub confirm_delete: bool,
    /// True while the "empty title" confirmation prompt is shown to the user.
    pub confirm_empty_title: bool,
    /// True only after the user explicitly answered "Yes, save" to the empty
    /// title prompt. Reset whenever the title is edited. Distinct from
    /// `confirm_empty_title` so that clicking the main Save button twice
    /// cannot bypass the confirmation.
    pub empty_title_confirmed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SetupForm {
    pub client_id: String,
    pub client_secret: String,
    pub calendar_id: String,
    pub error: Option<String>,
}

/// Result wrapper for async tasks that may have rotated the token.
#[derive(Debug, Clone)]
pub struct ApiResult<T> {
    pub new_token: Option<StoredToken>,
    pub result: Result<T, String>,
}

/// Snapshot of a deleted event, used to restore it via the Undo banner.
#[derive(Debug, Clone)]
pub struct PendingUndo {
    pub event: CalendarEvent,
    /// Monotonic id so a stale UndoExpired timer doesn't dismiss a newer undo.
    pub nonce: u64,
}

#[derive(Debug, Clone)]
pub struct DragState {
    pub event: CalendarEvent,
    pub source_date: NaiveDate,
    /// Cursor position at the moment of the press, used to detect a real drag.
    pub press_pos: Point,
    /// Becomes true once the cursor moves past a threshold.
    pub moved: bool,
}

/// Precomputed index: maps a date to the list of events occurring that day.
/// Timed events are indexed only on the local date of their start; all-day
/// events span from start_date to end_date-1 (Google's all-day end is
/// exclusive).
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
                    // Google's all-day end is exclusive: subtract one day if the
                    // event spans multiple days, otherwise keep the same day.
                    if e.all_day && d > start_date {
                        d.pred_opt().unwrap_or(d)
                    } else {
                        d
                    }
                }
                None => start_date,
            };

            // Iterate inclusively from start_date to end_date.
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

        // Sort each day's events: all-day first, then timed by start time.
        for idxs in by_date.values_mut() {
            idxs.sort_by(|&a, &b| {
                let ea = &events[a];
                let eb = &events[b];
                let ka = (if ea.all_day { 0u8 } else { 1u8 }, ea.start);
                let kb = (if eb.all_day { 0u8 } else { 1u8 }, eb.start);
                ka.cmp(&kb)
            });
        }

        Self { events, by_date }
    }

    #[cfg(test)]
    pub fn for_date(&self, date: NaiveDate) -> Vec<&CalendarEvent> {
        self.for_date_filtered(date, None)
    }

    /// Returns events occurring on `date`, optionally filtered by a case-
    /// insensitive summary query. The query is trimmed; empty queries are
    /// treated as "no filter".
    pub fn for_date_filtered(
        &self,
        date: NaiveDate,
        query: Option<&str>,
    ) -> Vec<&CalendarEvent> {
        let idxs = match self.by_date.get(&date) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let needle = query
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());
        match needle {
            None => idxs.iter().map(|i| &self.events[*i]).collect(),
            Some(q) => idxs
                .iter()
                .map(|i| &self.events[*i])
                .filter(|e| e.summary.to_lowercase().contains(&q))
                .collect(),
        }
    }
}

pub enum AppState {
    /// First run: user must enter OAuth credentials.
    Setup,
    /// Credentials known but no valid refresh token: full OAuth in progress.
    WaitingAuth,
    /// Token available: fetching events.
    Loading,
    /// Events loaded: main calendar view.
    Ready {
        index: EventIndex,
    },
    /// Any fatal error; UI shows Retry / Re-authenticate / Configure.
    Error(String),
}

pub struct App {
    pub config: AppConfig,
    pub palette: ColorPalette,
    pub theme: AppTheme,
    pub state: AppState,
    /// Saved when the user opens the setup wizard from an existing session,
    /// so Cancel can restore the previous screen.
    pub state_before_setup: Option<AppState>,
    pub setup_form: SetupForm,
    pub selected: NaiveDate,
    pub window_size: Size,
    pub window_position: Point,
    /// True once the real on-screen position of the window is known (loaded
    /// from disk or received via a WindowMoved event). Until then we must not
    /// persist the placeholder (0,0) into the saved state, otherwise the next
    /// launch would pin the window to the top-left corner of the screen.
    pub window_position_known: bool,
    pub last_cursor: Point,
    pub resize_start: Option<(f32, f32, Size)>,
    /// True while a debounced window-state save is pending.
    pub pending_window_save: bool,
    /// Alpha for the app background (container fill).
    pub bg_alpha: f32,
    /// Alpha for the whole window (SetLayeredWindowAttributes).
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
    pub pending_undo: Option<PendingUndo>,
    pub next_undo_nonce: u64,
    /// Snapshot of the event being edited, used to populate the undo banner
    /// after deletion.
    pub form_source_event: Option<CalendarEvent>,
    pub search_query: String,
    pub drag: Option<DragState>,
    pub hover_date: Option<NaiveDate>,
}

impl App {
    pub fn layout(&self) -> Layout {
        Layout::from_width(self.window_size.width)
    }
}

// Tests: exercise the EventIndex date bucketing, sorting and search filter.
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
            recurrence: None,
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
    fn all_day_first_then_by_start() {
        let day = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();

        let ad_start = chrono::Local
            .with_ymd_and_hms(2026, 9, 21, 0, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&chrono::Utc);
        let ad_end = chrono::Local
            .with_ymd_and_hms(2026, 9, 22, 0, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&chrono::Utc);

        let t1_start = chrono::Local
            .with_ymd_and_hms(2026, 9, 21, 15, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&chrono::Utc);
        let t2_start = chrono::Local
            .with_ymd_and_hms(2026, 9, 21, 9, 0, 0)
            .single()
            .unwrap()
            .with_timezone(&chrono::Utc);

        let events = vec![
            make("pm", t1_start, t1_start + Duration::hours(1), false),
            make("allday", ad_start, ad_end, true),
            make("am", t2_start, t2_start + Duration::hours(1), false),
        ];

        let idx = EventIndex::new(events);
        let ordered = idx.for_date(day);
        let ids: Vec<&str> = ordered.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["allday", "am", "pm"]);
    }

    #[test]
    fn search_filters_by_summary_case_insensitive() {
        let day = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
        let mk = |id: &str, summary: &str| {
            let start = chrono::Local
                .with_ymd_and_hms(2026, 9, 21, 10, 0, 0)
                .single()
                .unwrap()
                .with_timezone(&chrono::Utc);
            CalendarEvent {
                id: id.into(),
                summary: summary.into(),
                start,
                end: Some(start + Duration::hours(1)),
                color_id: None,
                all_day: false,
                recurring_event_id: None,
                original_start_time: None,
                recurrence: None,
            }
        };
        let idx = EventIndex::new(vec![
            mk("1", "Team Meeting"),
            mk("2", "Lunch with Bob"),
            mk("3", "meeting follow-up"),
        ]);

        let all = idx.for_date(day);
        assert_eq!(all.len(), 3);

        let q = idx.for_date_filtered(day, Some("meeting"));
        assert_eq!(q.len(), 2);
        assert!(q.iter().all(|e| e.summary.to_lowercase().contains("meeting")));

        let q2 = idx.for_date_filtered(day, Some("BOB"));
        assert_eq!(q2.len(), 1);
        assert_eq!(q2[0].id, "2");

        let q3 = idx.for_date_filtered(day, Some("  "));
        assert_eq!(q3.len(), 3);

        let q4 = idx.for_date_filtered(day, Some("zzz"));
        assert_eq!(q4.len(), 0);
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