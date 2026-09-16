//! Stable identity of the object shown in the inspector.

/// One selection shared by the roadmap, reasoning and changes views.
/// Identities survive sorting and reseeding; rows are only hit targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selected {
    Chunk(String),
    Step(u32),
    Change(String),
    Signal(String),
    /// Recorded action or check, keyed by its source identity.
    Receipt(String),
    /// Latest Workbench run result.
    Run(usize),
}
