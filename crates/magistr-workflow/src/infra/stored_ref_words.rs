//! Which vocabulary a saved document spelled its references in.

/// The vocabulary a saved document spelled its references in. Each family
/// codec knows this from the document's own version, never from the
/// references themselves: after the plan words, `task:` names a plan task,
/// while before the run words it named a run step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredRefWords {
    /// Before the run words: `step:<n>` was a reasoning step, `task:<id>` a
    /// run step, `action:<n>` a progress record and `chunk:<id>` a plan task.
    BeforeRunWords,
    /// After the run words, before the plan words: only `chunk:<id>` is old.
    BeforePlanWords,
}
