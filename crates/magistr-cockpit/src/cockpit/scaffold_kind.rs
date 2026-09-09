//! What the scaffold writes, and how each receipt reads.

/// One of the four things `magistr init` leaves on the ground (Birth
/// F2's Files written column): the project config, the rules, the
/// hooks, and the think-and-ship store. Each is a receipt — a real
/// file written, a real hook armed, a real store opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScaffoldKind {
    /// `.magistr/project.json`.
    Config,
    /// `.claude/rules`.
    Rules,
    /// The write guard and the gates.
    Hooks,
    /// The think-and-ship store.
    Store,
}

impl ScaffoldKind {
    /// The four receipts, in the order the scaffold writes them.
    pub const ALL: [Self; 4] = [Self::Config, Self::Rules, Self::Hooks, Self::Store];

    /// The receipt's key, as shown on screen.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Rules => "rules",
            Self::Hooks => "hooks",
            Self::Store => "store",
        }
    }

    /// The word the receipt's verdict uses when the thing is there:
    /// a file is written, a hook is armed, a store is opened.
    #[must_use]
    pub const fn verdict(self) -> &'static str {
        match self {
            Self::Config | Self::Rules => "Written",
            Self::Hooks => "Armed",
            Self::Store => "Opened",
        }
    }
}

#[cfg(test)]
#[path = "scaffold_kind.tests.rs"]
mod tests;
