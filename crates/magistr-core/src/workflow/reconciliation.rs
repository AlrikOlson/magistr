use magistr_domain::workflow::OperationRecord;

/// What recovery established about the attempts a lost owner left in
/// flight: the ones it could settle, with their settled outcome, and the
/// ones still open — a runtime that is still alive, a store it cannot ask.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reconciliation {
    pub settled: Vec<OperationRecord>,
    pub open: Vec<OperationRecord>,
}

impl Reconciliation {
    /// Nothing was left in flight.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.settled.is_empty() && self.open.is_empty()
    }

    /// Everything in flight has an established outcome.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.open.is_empty()
    }
}
