//! Central message enum for the Elm-style update loop.

use crate::app::{ApiResult, EditScope, RecurFreq};
use crate::api::client::CalendarEvent;
use crate::config::StoredToken;
use crate::tray::TrayMessage;
use chrono::NaiveDate;
use iced::{Point, Size};

#[derive(Debug, Clone)]
pub enum Message {
    // --- Auth & data -----------------------------------------------------
    TokenPolled(Result<StoredToken, String>),
    DataFetched(ApiResult<Vec<CalendarEvent>>),

    // --- Window plumbing -------------------------------------------------
    WindowResized(Size),
    WindowMoved(Point),
    FlushWindowState,
    WindowFocused,
    CursorMoved(Point),
    StartResize,
    GlobalLeftUp,

    // --- Calendar navigation --------------------------------------------
    Prev,
    Next,
    Today,

    // --- Event form ------------------------------------------------------
    OpenCreateForm,
    OpenCreateFormForDate(NaiveDate),
    OpenEditForm(CalendarEvent),
    CloseForm,
    FormTitleChanged(String),
    FormDateChanged(String),
    FormEndDateChanged(String),
    FormStartChanged(String),
    FormEndChanged(String),
    FormColorChanged(String),
    FormAllDayToggled(bool),
    FormRecurringToggled(bool),
    FormRecurFreqChanged(RecurFreq),
    FormRecurIntervalChanged(String),
    FormRecurUntilChanged(String),
    FormEditScopeChanged(EditScope),
    /// Set the start date (and end date if needed) to today.
    FormDateToday,
    /// Set the start time to now, end time to now + 1h.
    FormStartNow,
    ConfirmEmptyTitle,
    CancelEmptyTitle,
    SaveEvent,
    RequestDeleteEvent,
    CancelDeleteEvent,
    DeleteEvent,
    EventSaved(ApiResult<()>),
    EventDeleted(ApiResult<()>),

    // --- Undo banner -----------------------------------------------------
    UndoDelete,
    UndoExpired(u64),
    DismissUndo,
    UndoCompleted(ApiResult<()>),

    // --- Search & drag/drop ---------------------------------------------
    SearchQueryChanged(String),
    SearchClear,
    EventMouseDown {
        event: CalendarEvent,
        source_date: NaiveDate,
    },
    CellHover(NaiveDate),
    EventMoved(ApiResult<()>),
    StartDrag,

    // --- App lifecycle ---------------------------------------------------
    CloseWindow,
    Reauthenticate,
    RetryAuth,
    AutoRetryTick,
    CancelAuth,
    ApplyWindowEffects,
    ApplyWindowEffectsDeferred,
    HideOnStartup,

    // --- Appearance & options -------------------------------------------
    IncreaseTransparency,
    DecreaseTransparency,
    ToggleAutostart,
    AutostartToggled(Result<bool, String>),
    ToggleMenu,
    ToggleTheme,

    // --- Tray ------------------------------------------------------------
    PollTray,
    TrayEvent(TrayMessage),

    // --- Setup wizard ----------------------------------------------------
    SetupClientIdChanged(String),
    SetupClientSecretChanged(String),
    SetupCalendarIdChanged(String),
    SetupSubmit,
    SetupReconfigure,
    SetupCancel,
}