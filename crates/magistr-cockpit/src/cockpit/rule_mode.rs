//! How a charter rule answers: ask, act, or refuse.

/// The mode a charter rule wears on the native rail — the governance
/// policy's effect vocabulary folded to the three words the design's
/// mode badges print (view 05: `ASK` / `AUTO` / `OFF`).
///
/// The fold is the policy engine's own reading, not a new one:
/// `RequireApproval` is the ask, `Allow` and `Warn` both act without
/// stopping (a warning is permitted but recorded),
/// and `Deny` is off. The mapping lives here rather than in the CLI so
/// the badge and the word can never disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleMode {
    /// The rule stops and asks — `RequireApproval`.
    Ask,
    /// The rule acts without stopping — `Allow`, and `Warn`, which
    /// permits and records.
    Auto,
    /// The rule refuses — `Deny`.
    Off,
}

impl RuleMode {
    /// The badge's word, exactly as the capture prints it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ask => "ASK",
            Self::Auto => "AUTO",
            Self::Off => "OFF",
        }
    }
}

#[cfg(test)]
#[path = "rule_mode.tests.rs"]
mod tests;
