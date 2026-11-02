use super::{CheckOptions, TimeoutOrigin};

/// One resolved deadline shared by all verification entry points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckBudget {
    pub seconds: u64,
    pub origin: TimeoutOrigin,
}

impl CheckBudget {
    pub fn resolve(
        per_call: Option<u64>,
        gate: Option<u64>,
        project: Option<u64>,
    ) -> Result<Self, String> {
        let (seconds, origin) = if let Some(seconds) = per_call {
            (seconds, TimeoutOrigin::PerCall)
        } else if let Some(seconds) = gate {
            (seconds, TimeoutOrigin::Gate)
        } else if let Some(seconds) = project {
            (seconds, TimeoutOrigin::Project)
        } else {
            (CheckOptions::DEFAULT_TIMEOUT_SECS, TimeoutOrigin::Default)
        };
        if !(1..=CheckOptions::MAX_TIMEOUT_SECS).contains(&seconds) {
            return Err(format!(
                "Check timeout must be between 1 and {} seconds.",
                CheckOptions::MAX_TIMEOUT_SECS
            ));
        }
        Ok(Self { seconds, origin })
    }
}
