use serde::Serialize;

/// The filesystem probes, followed by the host's owner endpoint and run
/// checkpoint observations. Live worker health belongs to workspace status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorSectionId {
    Environment,
    Index,
    Config,
    Plugins,
    /// Live ownership: the durable generation and lock record, and what one
    /// bounded probe of the owner endpoint answered.
    Owner,
    /// Operation settlement: the run checkpoint's ledger, correlated.
    Operations,
}

impl DoctorSectionId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Environment => "environment",
            Self::Index => "index",
            Self::Config => "config",
            Self::Plugins => "plugins",
            Self::Owner => "owner",
            Self::Operations => "operations",
        }
    }
}
