//! Unix-time formatting for human output. Always UTC, so output does not
//! depend on the machine's time zone.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `2026-09-26 14:03:11 UTC`
pub fn utc(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

/// `3 minutes ago`; `just now` if the clock is behind the timestamp.
pub fn ago(ts: i64, now: i64) -> String {
    let d = now - ts;
    if d < 0 {
        return "just now".into();
    }
    let (n, unit) = match d {
        0..60 => (d, "second"),
        60..3_600 => (d / 60, "minute"),
        3_600..86_400 => (d / 3_600, "hour"),
        86_400..2_592_000 => (d / 86_400, "day"),
        2_592_000..31_536_000 => (d / 2_592_000, "month"),
        _ => (d / 31_536_000, "year"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian.
/// Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc() {
        assert_eq!(utc(0), "1970-01-01 00:00:00 UTC");
        // Dogecoin genesis block.
        assert_eq!(utc(1_386_325_540), "2013-12-06 10:25:40 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00:00 UTC");
    }

    #[test]
    fn formats_relative_time() {
        assert_eq!(ago(100, 100), "0 seconds ago");
        assert_eq!(ago(0, 1), "1 second ago");
        assert_eq!(ago(0, 180), "3 minutes ago");
        assert_eq!(ago(0, 7_200), "2 hours ago");
        assert_eq!(ago(0, 86_400), "1 day ago");
        assert_eq!(ago(10, 0), "just now");
    }
}
