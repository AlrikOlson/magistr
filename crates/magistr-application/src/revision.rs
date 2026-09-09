/// A project-state revision observed from the authoritative application adapter.
/// It is neither an owner generation nor an event sequence. Comparing revisions
/// is meaningful only within the same project and authority revision history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Revision(pub u64);
