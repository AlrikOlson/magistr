//! Time period for grouping cost entries.

/// Time period for grouping cost entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimePeriod {
    Daily,
    Weekly,
    Monthly,
}
