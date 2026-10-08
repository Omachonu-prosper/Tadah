//! Presentation helper: unix seconds -> local "2026-09-30 16:42". Kept out of the core.

use chrono::{DateTime, Local, TimeZone};

pub fn format_local(unix_seconds: i64) -> String {
    match Local.timestamp_opt(unix_seconds, 0).single() {
        Some(t) => format_datetime(t),
        None => unix_seconds.to_string(),
    }
}

fn format_datetime<Tz: TimeZone>(t: DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    t.format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn formats_year_month_day_hour_minute() {
        let t = Utc.with_ymd_and_hms(2026, 9, 30, 16, 42, 59).unwrap();
        assert_eq!(format_datetime(t), "2026-09-30 16:42");
    }

    #[test]
    fn local_formatting_has_the_same_shape() {
        let s = format_local(1_790_000_000);
        assert_eq!(s.len(), 16);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[10..11], " ");
    }
}
