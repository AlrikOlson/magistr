//! What a completed kill hold reaches.

/// The program a completed hold kills: the agent's terminal by the id
/// `terminal/create` gave it, which the session turns into
/// `terminal/kill` on the ACP transport; or a gate by its name, which
/// the loop driver in front of the session turns into a cancel on that
/// gate's own child. The terminal store keys both the same way, so a
/// pane, a strip and a receipt read one id whichever it was.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum KillCmd {
    /// The agent's terminal, by its id.
    Terminal(String),
    /// A gate's child, by the gate's name.
    Gate(String),
}

impl KillCmd {
    /// The id the terminal store keys the program's screen by — the
    /// terminal's id, or the gate's name.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Terminal(id) | Self::Gate(id) => id,
        }
    }
}
