//! What kind of thing a tool call does, as a one-word verb column.

/// The category of a tool call — ACP's `ToolKind`, carried into the
/// cockpit as the verb column a collapsed tool row leads with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToolVerb {
    /// Reading files or data.
    Read,
    /// Modifying files or content.
    Edit,
    /// Removing files or data.
    Delete,
    /// Moving or renaming files.
    Move,
    /// Searching for information.
    Search,
    /// Running commands or code.
    Run,
    /// Internal reasoning or planning.
    Think,
    /// Retrieving external data.
    Fetch,
    /// Switching the session mode.
    Mode,
    /// Anything else (ACP's default).
    #[default]
    Tool,
}

impl ToolVerb {
    /// The verb column's word. Every label fits the six-column verb
    /// field of a tool row.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Edit => "edit",
            Self::Delete => "delete",
            Self::Move => "move",
            Self::Search => "search",
            Self::Run => "run",
            Self::Think => "think",
            Self::Fetch => "fetch",
            Self::Mode => "mode",
            Self::Tool => "tool",
        }
    }
}
