//! Who a prose line is from.

/// The speaker of a prose line: the agent's message, its thinking, or
/// the user's prompt as the agent echoed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Voice {
    /// The agent's message text.
    Agent,
    /// The agent's thinking.
    Thought,
    /// The user's prompt, echoed by the agent.
    User,
}
