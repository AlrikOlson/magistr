/// Ordered event position within one run and owner generation. Sequences from
/// different generations are incomparable even when their numbers coincide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EventSequence(pub u64);
