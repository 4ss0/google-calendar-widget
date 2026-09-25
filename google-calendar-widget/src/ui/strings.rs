//! User-facing UI strings, centralized so a translation can be swapped in
//! by editing a single `CURRENT` alias.
//!
//! `CURRENT` is a `LazyLock<&'static Strings>`: the language is resolved
//! once at first use based on the OS locale. To force a specific language
//! regardless of the system, replace the body of the `LazyLock::new` with
//! `&EN` or `&IT`.

use std::sync::LazyLock;

pub struct Strings {
    pub today: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,
    pub autostart_on: &'static str,
    pub autostart_off: &'static str,
    pub settings: &'static str,
    pub search_placeholder: &'static str,

    pub setup_title: &'static str,
    pub setup_subtitle: &'static str,
    pub setup_help: &'static str,
    pub setup_client_id: &'static str,
    pub setup_client_secret: &'static str,
    pub setup_calendar_id: &'static str,
    pub setup_save_continue: &'static str,

    pub auth_title: &'static str,
    pub auth_browser_opened: &'static str,
    pub auth_complete_login: &'static str,
    pub auth_waiting: &'static str,
    pub auth_retry: &'static str,
    pub auth_cancel: &'static str,
    pub loading_events: &'static str,
    pub error_prefix: &'static str,
    pub error_hint: &'static str,
    pub error_reauth: &'static str,
    pub error_configure: &'static str,

    pub undo_event_deleted: &'static str,
    pub undo: &'static str,

    pub form_title: &'static str,
    pub form_title_placeholder: &'static str,
    pub form_date: &'static str,
    pub form_end_date: &'static str,
    pub form_all_day: &'static str,
    pub form_start: &'static str,
    pub form_end: &'static str,
    pub form_color: &'static str,
    #[allow(dead_code)] 
    pub form_today: &'static str,
    #[allow(dead_code)] 
    pub form_now: &'static str,
    pub form_save: &'static str,
    pub form_saving: &'static str,
    pub form_cancel: &'static str,
    pub form_please_wait: &'static str,
    pub form_delete: &'static str,
    pub form_operation_in_progress: &'static str,
    pub form_apply_to: &'static str,
    pub form_recurring: &'static str,
    pub form_every: &'static str,
    pub form_interval: &'static str,
    pub form_until: &'static str,
    pub form_empty_title: &'static str,
    pub form_yes_save: &'static str,
    pub form_no: &'static str,
    pub form_confirm_delete: &'static str,
    pub form_delete_scope: &'static str,
    pub form_yes_delete: &'static str,

    pub no_events: &'static str,
    pub no_matching_events: &'static str,
}

pub const EN: Strings = Strings {
    today: "Today",
    theme_light: "Light",
    theme_dark: "Dark",
    autostart_on: "Auto: ON",
    autostart_off: "Auto: OFF",
    settings: "Cfg",
    search_placeholder: "Search…",

    setup_title: "Welcome to Google Calendar Widget",
    setup_subtitle: "To get started, enter the OAuth credentials of your Google Cloud application.",
    setup_help: "If you don't have them, create a project on console.cloud.google.com, enable the Calendar API, create OAuth credentials of type \"Desktop app\" and paste the Client ID and Client Secret here.",
    setup_client_id: "Client ID",
    setup_client_secret: "Client Secret",
    setup_calendar_id: "Calendar ID (optional)",
    setup_save_continue: "Save and continue",

    auth_title: "Google Calendar Authorization",
    auth_browser_opened: "The browser has been opened for authorization.",
    auth_complete_login: "Complete login and grant access.",
    auth_waiting: "Waiting for confirmation...",
    auth_retry: "Retry",
    auth_cancel: "Cancel",
    loading_events: "Loading events...",
    error_prefix: "Error:",
    error_hint: "If the problem is a missing internet connection, click Retry once it is back. Use \"Re-authenticate\" only if you want to sign in with a different account.",
    error_reauth: "Re-authenticate",
    error_configure: "Configure credentials",

    undo_event_deleted: "Event deleted:",
    undo: "Undo",

    form_title: "Title:",
    form_title_placeholder: "Event title",
    form_date: "Date:",
    form_end_date: "End date:",
    form_all_day: "All day",
    form_start: "Start:",
    form_end: "End:",
    form_color: "Color:",
    form_today: "Today",
    form_now: "Now",
    form_save: "Save",
    form_saving: "Saving...",
    form_cancel: "Cancel",
    form_please_wait: "Please wait...",
    form_delete: "Delete",
    form_operation_in_progress: "Operation in progress...",
    form_apply_to: "Apply changes to:",
    form_recurring: "Recurring",
    form_every: "Every:",
    form_interval: "Interval:",
    form_until: "Until (optional):",
    form_empty_title: "Title is empty. Save anyway?",
    form_yes_save: "Yes, save",
    form_no: "No",
    form_confirm_delete: "Are you sure?",
    form_delete_scope: "Delete:",
    form_yes_delete: "Yes, delete",

    no_events: "No events",
    no_matching_events: "No matching events",
};

pub const IT: Strings = Strings {
    today: "Oggi",
    theme_light: "Chiaro",
    theme_dark: "Scuro",
    autostart_on: "Auto: Sì",
    autostart_off: "Auto: No",
    settings: "Cfg",
    search_placeholder: "Cerca…",

    setup_title: "Benvenuto in Google Calendar Widget",
    setup_subtitle: "Per iniziare, inserisci le credenziali OAuth della tua applicazione Google Cloud.",
    setup_help: "Se non le hai, crea un progetto su console.cloud.google.com, abilita la Calendar API, crea credenziali OAuth di tipo \"Desktop app\" e incolla qui Client ID e Client Secret.",
    setup_client_id: "Client ID",
    setup_client_secret: "Client Secret",
    setup_calendar_id: "Calendar ID (opzionale)",
    setup_save_continue: "Salva e continua",

    auth_title: "Autorizzazione Google Calendar",
    auth_browser_opened: "Il browser è stato aperto per l'autorizzazione.",
    auth_complete_login: "Completa il login e concedi l'accesso.",
    auth_waiting: "In attesa di conferma...",
    auth_retry: "Riprova",
    auth_cancel: "Annulla",
    loading_events: "Caricamento eventi...",
    error_prefix: "Errore:",
    error_hint: "Se il problema è l'assenza di connessione, clicca Riprova quando torna. Usa \"Riautentica\" solo per accedere con un account diverso.",
    error_reauth: "Riautentica",
    error_configure: "Configura credenziali",

    undo_event_deleted: "Evento eliminato:",
    undo: "Annulla",

    form_title: "Titolo:",
    form_title_placeholder: "Titolo evento",
    form_date: "Data:",
    form_end_date: "Fine:",
    form_all_day: "Tutto il giorno",
    form_start: "Inizio:",
    form_end: "Fine:",
    form_color: "Colore:",
    form_today: "Oggi",
    form_now: "Adesso",
    form_save: "Salva",
    form_saving: "Salvataggio...",
    form_cancel: "Annulla",
    form_please_wait: "Attendere...",
    form_delete: "Elimina",
    form_operation_in_progress: "Operazione in corso...",
    form_apply_to: "Applica a:",
    form_recurring: "Ricorrente",
    form_every: "Ogni:",
    form_interval: "Intervallo:",
    form_until: "Fino al (opzionale):",
    form_empty_title: "Il titolo è vuoto. Salvare comunque?",
    form_yes_save: "Sì, salva",
    form_no: "No",
    form_confirm_delete: "Sei sicuro?",
    form_delete_scope: "Elimina:",
    form_yes_delete: "Sì, elimina",

    no_events: "Nessun evento",
    no_matching_events: "Nessun evento corrispondente",
};

fn system_language_is_italian() -> bool {
    sys_locale::get_locale()
        .map(|loc| loc.to_lowercase().starts_with("it"))
        .unwrap_or(false)
}

/// Active language, resolved once at first use from the OS locale.
pub static CURRENT: LazyLock<&'static Strings> = LazyLock::new(|| {
    if system_language_is_italian() {
        &IT
    } else {
        &EN
    }
});