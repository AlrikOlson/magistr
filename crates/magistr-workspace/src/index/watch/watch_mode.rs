use serde::{Deserialize, Serialize};

/// Requested or effective workspace invalidation adapter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WatchMode {
    #[default]
    Auto,
    Native,
    Poll,
    Off,
}

impl WatchMode {
    #[must_use]
    pub fn effective(self, network: bool) -> Self {
        match self {
            Self::Auto if network => Self::Poll,
            Self::Auto => Self::Native,
            other => other,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Native => "native",
            Self::Poll => "poll",
            Self::Off => "off",
        }
    }
}
