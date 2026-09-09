//! One structured row of a workbench run's result.

/// A row of a run's result — a failing test, a diagnostic, a passing
/// suite — as the workbench's runner adapter structured it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunRow {
    /// What the row names: a test, a diagnostic's location, a suite.
    pub name: String,
    /// Whether it passed.
    pub passed: bool,
    /// The assertion, message or timing behind the row; empty when the
    /// name says it all.
    pub detail: String,
}
