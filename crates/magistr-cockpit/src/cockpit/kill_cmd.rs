//! What a completed kill hold reaches.

/// The program a completed hold kills: the agent's terminal by the id
/// `terminal/create` gave it, which the session turns into
/// `terminal/kill` on the ACP transport; or a check by its name, which
/// the loop driver in front of the session turns into a cancel on that
/// check's own child. The terminal store keys both the same way, so a
/// pane, a strip and a receipt read one id whichever it was.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum KillCmd {
    /// The agent's terminal, by its id.
    Terminal(String),
    /// A check's child, by the check's name.
    Check(String),
}

impl KillCmd {
    /// The id the terminal store keys the program's screen by — the
    /// terminal's id, or the check's name.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Terminal(id) | Self::Check(id) => id,
        }
    }
}
