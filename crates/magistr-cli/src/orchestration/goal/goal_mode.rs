//! Who drives the goal loop.

/// Which judge drives the loop — `MAGISTR_GOAL`.
///
/// `Native` is magistr's own fresh judge and the default; the three
/// delegated modes hand the loop to the agent's own `/goal` (a
/// token-cost optimisation that costs every verdict, reason and
/// iteration count magistr could otherwise show); `Off` runs plain
/// single iterations with no judge at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum GoalMode {
    /// magistr's own judge over the full acceptance list and the real
    /// gate evidence.
    #[default]
    Native,
    /// The agent's `_session/goal` extension.
    Extension,
    /// The agent's advertised `goal` command.
    Command,
    /// Try the extension, then the command, and report what took.
    Auto,
    /// No goal loop: one iteration at a time, as before.
    Off,
}

impl GoalMode {
    /// The environment variable that selects the mode.
    pub const ENV: &'static str = "MAGISTR_GOAL";

    /// Parse a value of [`Self::ENV`]; `None` for one that names no mode.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "native" => Some(Self::Native),
            "extension" => Some(Self::Extension),
            "command" => Some(Self::Command),
            "auto" => Some(Self::Auto),
            "off" | "none" => Some(Self::Off),
            _ => None,
        }
    }

    /// The mode `MAGISTR_GOAL` names, and a note when it named none:
    /// an unknown value is `Native` *and said so*, never silently.
    #[must_use]
    pub fn from_env() -> (Self, Option<String>) {
        Self::from_value(std::env::var(Self::ENV).ok().as_deref())
    }

    /// [`Self::from_env`] over an explicit value, for tests.
    #[must_use]
    pub fn from_value(value: Option<&str>) -> (Self, Option<String>) {
        match value {
            None => (Self::Native, None),
            Some(raw) => Self::parse(raw).map_or_else(
                || {
                    (
                        Self::Native,
                        Some(format!(
                            "{}={raw:?} names no goal mode (native, extension, command, auto, off) — running native",
                            Self::ENV
                        )),
                    )
                },
                |mode| (mode, None),
            ),
        }
    }

    /// Whether this mode hands the loop to the agent's own evaluator —
    /// where magistr is blind inside the loop.
    #[must_use]
    pub const fn is_delegated(self) -> bool {
        matches!(self, Self::Extension | Self::Command | Self::Auto)
    }

    /// The word the cockpit says when it names who is driving.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Extension => "extension",
            Self::Command => "command",
            Self::Auto => "auto",
            Self::Off => "off",
        }
    }
}

#[cfg(test)]
#[path = "goal_mode.tests.rs"]
mod tests;
