//! Who a flat line is from.

/// The role label a flat line leads with — what a screen reader says
/// before the text, so a listener knows who is talking without any
/// colour, indent or glyph to see.
///
/// The vocabulary is deliberately one word per role and never a
/// sentence: it is spoken before *every* line of that role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// The session's own opening line.
    Session,
    /// The user's prompt, as it went to the agent.
    User,
    /// The agent's message text.
    Agent,
    /// The agent's thinking.
    Thought,
    /// A tool call.
    Tool,
    /// A quality gate recorded on the ship trace.
    Gate,
    /// A workbench edit.
    Edit,
    /// An image the agent sent, named by its alt text — a flat cockpit
    /// has no pixels to show it with.
    Image,
    /// A think-and-ship event.
    Event,
    /// Something the cockpit noted.
    Note,
    /// Something that failed.
    Error,
    /// A line off the announcement channel (a phase change, a Needs-you
    /// card) — the same text the OSC 9 notification carries.
    Attention,
}

impl Role {
    /// The word this role is spoken as.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::User => "you",
            Self::Agent => "agent",
            Self::Thought => "thinking",
            Self::Tool => "tool",
            Self::Gate => "gate",
            Self::Edit => "edit",
            Self::Image => "image",
            Self::Event => "event",
            Self::Note => "note",
            Self::Error => "error",
            Self::Attention => "attention",
        }
    }
}

#[cfg(test)]
#[path = "role.tests.rs"]
mod tests;
