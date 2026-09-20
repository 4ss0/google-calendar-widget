use chrono::{Datelike, NaiveDate};

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