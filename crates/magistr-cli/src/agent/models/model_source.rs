//! Where a model definition came from.

/// Where a model definition came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSource {
    /// Built-in default (ships with magistr).
    Builtin,
    /// User-defined in `~/.config/magistr/models.toml`.
    UserConfig,
}

impl ModelSource {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Builtin => "built-in",
            Self::UserConfig => "user config",
        }
    }
}

impl std::fmt::Display for ModelSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
