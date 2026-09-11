use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// RFC 3339 UTC timestamps for breaker and promotion records, without a
/// calendar dependency. Tests inject fixed strings through the host instead.
pub struct HostClock;

impl HostClock {
    #[must_use]
    pub fn now() -> String {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        Self::format(secs)
    }

    /// `YYYY-MM-DDTHH:MM:SSZ` for a Unix second count (proleptic Gregorian).
    #[must_use]
    pub fn format(secs: u64) -> String {
        let days = secs / 86_400;
        let rem = secs % 86_400;
        let (year, month, day) = civil_from_days(days as i64);
        format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
            rem / 3600,
            rem % 3600 / 60,
            rem % 60
        )
    }

    /// Unix seconds of an RFC 3339 UTC timestamp (`Z` or `+00:00`, fractional
    /// seconds ignored); `None` for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<u64> {
        let text = text
            .strip_suffix('Z')
            .or_else(|| text.strip_suffix("+00:00"))?;
        let (date, time) = text.split_once('T')?;
        let mut date = date.split('-');
        let year: i64 = date.next()?.parse().ok()?;
        let month: u32 = date.next()?.parse().ok()?;
        let day: u32 = date.next()?.parse().ok()?;
        if date.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        let time = time.split('.').next()?;
        let mut time = time.split(':');
        let hour: u64 = time.next()?.parse().ok()?;
        let minute: u64 = time.next()?.parse().ok()?;
        let second: u64 = time.next()?.parse().ok()?;
        if time.next().is_some() || hour > 23 || minute > 59 || second > 60 {
            return None;
        }
        let days = days_from_civil(year, month, day);
        u64::try_from(days)
            .ok()?
            .checked_mul(86_400)?
            .checked_add(hour * 3600 + minute * 60 + second)
    }
}

// Howard Hinnant's civil-date algorithms (public domain).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
#[path = "host_clock.tests.rs"]
mod tests;
