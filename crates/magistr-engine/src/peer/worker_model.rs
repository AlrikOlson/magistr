use serde::{Deserialize, Serialize};

/// Exact provider/model/effort selection; resolving availability belongs to the host.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerModel {
    pub provider: String,
    pub model: String,
    pub effort: String,
}

impl WorkerModel {
    pub(super) fn validate(&self) -> Result<(), super::AgreementError> {
        for (name, value) in [
            ("provider", &self.provider),
            ("model", &self.model),
            ("effort", &self.effort),
        ] {
            super::require_text(name, value)?;
            if value.contains(['*', '?']) {
                return Err(super::AgreementError::Invalid(format!(
                    "{name} must be exact"
                )));
            }
        }
        Ok(())
    }
}
