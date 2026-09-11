use serde::Serialize;

/// A finding's weight: only `unhealthy` findings make the report a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorSeverity {
    Healthy,
    Info,
    Unhealthy,
}

impl DoctorSeverity {
    #[must_use]
    pub const fn mark(self) -> &'static str {
        match self {
            Self::Healthy => "ok",
            Self::Info => "--",
            Self::Unhealthy => "!!",
        }
    }
}
