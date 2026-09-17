use serde::Serialize;

/// The sections in contract order: the six retained probes over the
/// `.workbench` artifacts, then the two the host composes from what only
/// it can observe — the owner endpoint and the run checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorSectionId {
    Environment,
    Index,
    Config,
    Plugins,
    Quarantine,
    Crashes,
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
            Self::Quarantine => "quarantine",
            Self::Crashes => "crashes",
            Self::Owner => "owner",
            Self::Operations => "operations",
        }
    }
}
