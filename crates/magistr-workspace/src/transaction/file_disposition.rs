/// Observed effect state; compensation is reported only after byte read-back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileDisposition {
    Unchanged,
    NotApplied,
    Applied,
    RolledBack,
    Indeterminate,
}
