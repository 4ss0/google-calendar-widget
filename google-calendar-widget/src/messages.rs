//! Central message enum for the Elm-style update loop.
//!
//! Every user interaction, async task completion, or timer tick is
//! represented here. The variants are grouped by concern:
//!   - Auth / data: TokenPolled, DataFetched
//!   - Window plumbing: WindowResized, WindowMoved, KeepAtBottom, ...
//!   - Calendar navigation: Prev, Next, Today
//!   - Event form: Open*Form, Form*Changed, SaveEvent, DeleteEvent, ...
//!   - Drag & drop: EventMouseDown, CellHover, EventMoved
//!   - Tray / menu / theme / transparency
//!   - Setup wizard

use crate::app::{ApiResult, EditScope, RecurFreq};
use crate::api::client::CalendarEvent;
use crate::config::StoredToken;
use crate::tray::TrayMessage;
use chrono::NaiveDate;
use iced::{Point, Size};

#[derive(Debug, Clone)]
pub enum Message {
    // --- Auth & data -----------------------------------------------------
    /// Result of the initial OAuth flow or a silent token refresh.
    TokenPolled(Result<StoredToken, String>),
    /// Result of `fetch_events`, wrapped with an optional refreshed token.
    DataFetched(ApiResult<Vec<CalendarEvent>>),

    // --- Window plumbing -------------------------------------------------
    WindowResized(Size),
    WindowMoved(Point),
    /// Debounced write of window geometry to disk (triggered 500ms after
    /// the last resize/move).
    FlushWindowState,
    WindowFocused,
    CursorMoved(Point),
    /// Bottom-right resize handle pressed.
    StartResize,
    /// Global left mouse-up. Terminates resize and drag/drop.
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
    /// User confirmed saving with an empty title.
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
    /// Fired after UNDO_WINDOW_SECS; dismisses the banner if nonce matches.
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
    /// Begin OS-level window drag (title bar click).
    StartDrag,

    // --- App lifecycle ---------------------------------------------------
    CloseWindow,
    Reauthenticate,
    RetryAuth,
    /// Periodic retry while in Error state.
    AutoRetryTick,
    CancelAuth,
    /// Applied once the HWND is available; installs hooks and Win32 tweaks.
    ApplyWindowEffects,
    /// Re-applies alpha / taskbar removal at multiple delays after startup.
    ApplyWindowEffectsDeferred,
    HideOnStartup,
    /// Re-pin the window to the bottom of the z-order.
    KeepAtBottom,

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