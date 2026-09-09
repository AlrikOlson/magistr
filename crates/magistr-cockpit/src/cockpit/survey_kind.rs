//! What the survey detects, and what it writes.

/// One of the four facts `magistr init`'s autodetect finds about the
/// ground (Birth F2's Detected column): the stack, the tests, git, and
/// the agent on PATH. Each lands as its own row, once, as it is found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SurveyKind {
    /// The language, its edition and its workspace shape.
    Stack,
    /// The test runner and the lints configured.
    Tests,
    /// The branch, its cleanliness and its history.
    Git,
    /// The agent binary on PATH.
    Agent,
}

impl SurveyKind {
    /// The four facts, in the order the sweep finds them.
    pub const ALL: [Self; 4] = [Self::Stack, Self::Tests, Self::Git, Self::Agent];

    /// The row's key, as shown on screen.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stack => "stack",
            Self::Tests => "tests",
            Self::Git => "git",
            Self::Agent => "agent",
        }
    }
}

#[cfg(test)]
#[path = "survey_kind.tests.rs"]
mod tests;
