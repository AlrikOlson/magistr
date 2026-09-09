//! Tool kind classification — semantic category of a tool invocation.

/// Category of tool being invoked (mirrors ACP `ToolKind`).
///
/// Used by frontends to select per-kind icons and optimize display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Read,
    Edit,
    Delete,
    Move,
    Search,
    Execute,
    Think,
    Fetch,
    SwitchMode,
    Other,
}

impl ToolKind {
    /// Icon prefix for display.
    ///
    /// Uses Unicode symbols that work across terminals (no Nerd Fonts required).
    pub fn icon(self) -> &'static str {
        match self {
            Self::Read => "◁",
            Self::Edit => "✎",
            Self::Delete => "✕",
            Self::Move => "↗",
            Self::Search => "🔍",
            Self::Execute => "⚡",
            Self::Think => "◆",
            Self::Fetch => "↓",
            Self::SwitchMode => "⇋",
            Self::Other => "▸",
        }
    }
}

#[cfg(test)]
#[path = "tool_kind.tests.rs"]
mod tests;
