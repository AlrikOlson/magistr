//! Which vocabulary a saved document spelled its references in.

/// The vocabulary a saved document spelled its references in. Each family
/// codec knows this from the document's own version, never from the
/// references themselves: after the plan words, `task:` names a plan task,
/// while before the run words it named a run step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredRefWords {
    /// Before the run words: `step:<n>` was a reasoning step, `task:<id>` a
    /// run step, `action:<n>` a progress record, `chunk:<id>` a plan task and
    /// `think:<n>` a decision note.
    BeforeRunWords,
    /// After the run words, before the plan words: `chunk:<id>` and
    /// `think:<n>` are old.
    BeforePlanWords,
    /// After the plan words, before the decision words: only `think:<n>` is
    /// old.
    BeforeDecisionWords,
}
