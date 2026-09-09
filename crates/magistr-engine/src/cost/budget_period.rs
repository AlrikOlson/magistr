use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};

/// The window a budget rule's limit resets over.
///
/// Windows are calendar-aligned in UTC — a daily cap resets at midnight,
/// a weekly cap on Monday, a monthly cap on the first — so two people in
/// different time zones agree on when the money came back.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    /// No window — the limit applies to this session's own spend only.
    #[default]
    Session,
    /// Since midnight UTC today.
    Daily,
    /// Since Monday of the current ISO week, UTC.
    Weekly,
    /// Since the first of the current month, UTC.
    Monthly,
}

impl BudgetPeriod {
    /// The serialized name of this period.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            BudgetPeriod::Session => "session",
            BudgetPeriod::Daily => "daily",
            BudgetPeriod::Weekly => "weekly",
            BudgetPeriod::Monthly => "monthly",
        }
    }

    /// When the current window opened, or `None` for [`BudgetPeriod::Session`]
    /// (which has no historical window — nothing prior counts against it).
    #[must_use]
    pub fn window_start(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let midnight = |d: DateTime<Utc>| {
            Utc.with_ymd_and_hms(d.year(), d.month(), d.day(), 0, 0, 0)
                .single()
        };
        match self {
            BudgetPeriod::Session => None,
            BudgetPeriod::Daily => midnight(now),
            BudgetPeriod::Weekly => {
                let back = i64::from(now.weekday().num_days_from_monday());
                midnight(now - Duration::days(back))
            }
            BudgetPeriod::Monthly => Utc
                .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
                .single(),
        }
    }
}

impl std::fmt::Display for BudgetPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "budget_period.tests.rs"]
mod tests;
