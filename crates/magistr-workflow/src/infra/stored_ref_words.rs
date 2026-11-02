//! Which vocabulary a saved document spelled its references in.

/// The vocabulary a saved document spelled its references in. Each family
/// codec knows this from the document's own version, never from the
/// references themselves: after the plan words, `task:` names a plan task,
/// while before the run words it named a run step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredRefWords {
    /// Before the run words: `step:<n>` was a reasoning step, `task:<id>` a
    /// run step, `action:<n>` a progress record, `chunk:<id>` a plan task and
    /// `think:<n>` a decision note and `signal:<id>` a request.
    BeforeRunWords,
    /// After the run words, before the plan words: `chunk:<id>`,
    /// `think:<n>` and `signal:<id>` are old.
    BeforePlanWords,
    /// After the plan words, before the decision words: `think:<n>` and
    /// `signal:<id>` are old.
    BeforeDecisionWords,
    /// After the decision words, before the request words: only
    /// `signal:<id>` is old.
    BeforeRequestWords,
}
