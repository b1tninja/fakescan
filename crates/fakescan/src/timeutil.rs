//! UTC wall-clock ISO-8601 helpers (no extra crate deps).

use std::time::{SystemTime, UNIX_EPOCH};

/// Current UTC as `YYYY-MM-DDTHH:MM:SSZ` (second resolution).
pub fn utc_now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_unix_secs_utc(secs)
}

/// Format unix seconds as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn format_unix_secs_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// True when `s` is exactly `YYYY-MM-DDTHH:MM:SSZ` with ASCII digits.
pub fn is_utc_iso(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'Z'
        && b[0..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[11..13].iter().all(u8::is_ascii_digit)
        && b[14..16].iter().all(u8::is_ascii_digit)
        && b[17..19].iter().all(u8::is_ascii_digit)
}

/// Days since Unix epoch (1970-01-01) → Gregorian `(year, month, day)`.
///
/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(mut z: i64) -> (i32, u32, u32) {
    z += 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_formats() {
        assert_eq!(format_unix_secs_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_secs_utc(1), "1970-01-01T00:00:01Z");
        // 2026-01-01T00:00:00Z
        assert_eq!(format_unix_secs_utc(1_767_225_600), "2026-01-01T00:00:00Z");
        // leap day
        assert_eq!(format_unix_secs_utc(1_582_934_400), "2020-02-29T00:00:00Z");
        // mid-day
        assert_eq!(
            format_unix_secs_utc(1_767_225_600 + 45_230),
            "2026-01-01T12:33:50Z"
        );
    }

    #[test]
    fn is_utc_iso_shape() {
        assert!(is_utc_iso("2026-01-01T00:00:00Z"));
        assert!(is_utc_iso(&utc_now_iso()));
        assert!(!is_utc_iso("2026-01-01T00:00:00"));
        assert!(!is_utc_iso("2026-1-01T00:00:00Z"));
        assert!(!is_utc_iso(""));
    }
}
