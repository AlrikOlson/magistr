/// Durable mutation observation, never an immutable source certification.
/// Pending survives interruption and requires reconciliation by the owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalWriteOutcome {
    Pending,
    Applied,
    RolledBack,
    Indeterminate,
}
