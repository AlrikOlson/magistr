use serde::Serialize;

/// The six sections in contract order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorSectionId {
    Environment,
    Index,
    Config,
    Plugins,
    Quarantine,
    Crashes,
}

impl DoctorSectionId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Index => "index",
            Self::Config => "config",
            Self::Plugins => "plugins",
            Self::Quarantine => "quarantine",
            Self::Crashes => "crashes",
        }
    }
}
