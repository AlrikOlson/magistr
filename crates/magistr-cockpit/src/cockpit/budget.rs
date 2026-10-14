//! The session's spending picture, as the rail shows it.

/// The bound budget on the native rail: what the session has spent,
/// the cap the meter runs to, and the breaker it stops at — all in
/// cents, so the rail's state stays `Eq` and two frames of one budget
/// can never differ by a float's residue.
///
/// The limit is the meter's full width and the breaker is the tick on
/// it (view 05: `1.84 / 12.00`, `STOP AT 10.00`): the governed cap can
/// sit inside a wider local limit, which is exactly the picture the
/// cost governor's `min()` produces. Spend is optional because nothing
/// meters a live session's dollars yet — a budget with no spend still
/// shows its cap, with the meter empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    spent_cents: Option<u64>,
    limit_cents: u64,
    stop_cents: Option<u64>,
}

impl Budget {
    /// A budget capped at `limit_cents`. Zero is refused — a meter to
    /// nowhere is furniture, and the seeder simply sets no budget.
    #[must_use]
    pub fn capped(limit_cents: u64) -> Option<Self> {
        (limit_cents > 0).then_some(Self {
            spent_cents: None,
            limit_cents,
            stop_cents: None,
        })
    }

    /// The same budget with `spent_cents` on the meter.
    #[must_use]
    pub const fn spent(mut self, spent_cents: u64) -> Self {
        self.spent_cents = Some(spent_cents);
        self
    }

    /// The same budget with the breaker at `stop_cents`.
    #[must_use]
    pub const fn stop_at(mut self, stop_cents: u64) -> Self {
        self.stop_cents = Some(stop_cents);
        self
    }

    /// The meter's readout — `1.84 / 12.00`, or `— / 12.00` while
    /// nothing meters the spend.
    #[must_use]
    pub fn readout(&self) -> String {
        match self.spent_cents {
            Some(spent) => format!(
                "{} / {}",
                Self::amount(spent),
                Self::amount(self.limit_cents)
            ),
            None => format!("— / {}", Self::amount(self.limit_cents)),
        }
    }

    /// The breaker line — `STOP AT 10.00` — when one is set.
    #[must_use]
    pub fn breaker(&self) -> Option<String> {
        self.stop_cents
            .map(|stop| format!("STOP AT {}", Self::amount(stop)))
    }

    /// How full the meter is, in per-mille of the limit.
    /// The breaker's amount alone — `10.00` — when one is set.
    #[must_use]
    pub fn stop_readout(&self) -> Option<String> {
        self.stop_cents.map(Self::amount)
    }

    /// Whether the spend has reached the breaker: the stop that pauses
    /// a session, which the bad day reports as the Limit card.
    #[must_use]
    pub fn is_tripped(&self) -> bool {
        match (self.spent_cents, self.stop_cents) {
            (Some(spent), Some(stop)) => spent >= stop,
            _ => false,
        }
    }

    #[must_use]
    pub fn fill_per_mille(&self) -> u16 {
        self.per_mille(self.spent_cents.unwrap_or(0))
    }

    /// Where the breaker's tick sits, in per-mille of the limit.
    #[must_use]
    pub fn tick_per_mille(&self) -> Option<u16> {
        self.stop_cents.map(|stop| self.per_mille(stop))
    }

    fn per_mille(&self, cents: u64) -> u16 {
        let share = cents.saturating_mul(1000) / self.limit_cents;
        u16::try_from(share.min(1000)).unwrap_or(1000)
    }

    /// `184` cents as `1.84` — the copy law's own spelling, spelled
    /// once in [`Copy::amount`].
    fn amount(cents: u64) -> String {
        format!("{}.{:02}", cents / 100, cents % 100)
    }
}

#[cfg(test)]
#[path = "budget.tests.rs"]
mod tests;
