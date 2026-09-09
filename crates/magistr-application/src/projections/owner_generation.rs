/// Owner-authority fencing generation. It changes when ownership changes, not
/// on each state write; absence means the adapter has no ownership evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OwnerGeneration(pub u64);
