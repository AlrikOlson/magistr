/// Estimated token accounting; both counters are nonnegative by construction.
///
/// This records caller-supplied accounting. Row selection and token ceilings
/// belong to the navigation use cases; these counters do not measure a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub requested: u64,
    pub used: u64,
}
