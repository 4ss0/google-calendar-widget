//! Small date helpers used by the calendar navigation.

use chrono::{Datelike, NaiveDate};

/// Shifts a date by `delta` months, always landing on the 1st of the target
/// month. Clamps to the original date only if the year would go out of range
/// (extremely unlikely).
///
/// Used by Prev/Next in month layout so that e.g. 31 Jan + 1 month => 1 Feb.
pub fn shift_month(d: NaiveDate, delta: i32) -> NaiveDate {
    let mut y = d.year();
    let mut m = d.month() as i32 + delta;
    while m < 1 {
        m += 12;
        y -= 1;
    }
    while m > 12 {
        m -= 12;
        y += 1;
    }
    NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap_or(d)
}