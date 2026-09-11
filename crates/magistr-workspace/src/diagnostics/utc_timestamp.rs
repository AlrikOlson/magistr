/// Milliseconds since the Unix epoch now; a clock before the epoch reads 0.
#[must_use]
pub(crate) fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// RFC 3339 UTC with milliseconds (`2026-08-22T00:00:00.000Z`), the shape
/// JavaScript's `toISOString` prints — no calendar dependency needed.
#[must_use]
pub fn utc_timestamp(unix_ms: u64) -> String {
    let millis = unix_ms % 1000;
    let seconds = unix_ms / 1000;
    let (hours, minutes, secs) = (seconds / 3600 % 24, seconds / 60 % 60, seconds % 60);
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    // Howard Hinnant's civil-from-days, valid for the whole Unix era.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{secs:02}.{millis:03}Z")
}

#[cfg(test)]
#[path = "utc_timestamp.tests.rs"]
mod tests;
