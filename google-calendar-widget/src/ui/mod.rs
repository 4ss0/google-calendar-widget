//! Reusable UI building blocks: theme constants, responsive layout logic,
//! event-box rendering, and the three calendar views (month/week/day).
//!
//! The view layer (src/view/) composes these blocks into the final Element
//! tree. Splitting them like this keeps the app shell (top bar, form, setup)
//! separate from the calendar rendering internals.

pub mod common;
pub mod layout;
pub mod month_view;
pub mod week_view;
pub mod day_view;
pub mod theme;

pub use layout::{build_view, Layout};
pub use theme::AppTheme;