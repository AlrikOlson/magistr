use serde::Serialize;

use super::{DoctorDataClass, DoctorSeverity};

/// One probed fact about a shipped artifact or environment detail. Every
/// unhealthy finding names its remediation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorFinding {
    /// What was probed: `native-index.db`, `plugin-health.json`, `config`, …
    pub store: String,
    pub data_class: DoctorDataClass,
    pub severity: DoctorSeverity,
    pub symptom: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl DoctorFinding {
    #[must_use]
    pub fn healthy(store: impl Into<String>, symptom: impl Into<String>) -> Self {
        Self {
            store: store.into(),
            data_class: DoctorDataClass::NotApplicable,
            severity: DoctorSeverity::Healthy,
            symptom: symptom.into(),
            remediation: None,
        }
    }

    #[must_use]
    pub fn info(
        store: impl Into<String>,
        data_class: DoctorDataClass,
        symptom: impl Into<String>,
    ) -> Self {
        Self {
            store: store.into(),
            data_class,
            severity: DoctorSeverity::Info,
            symptom: symptom.into(),
            remediation: None,
        }
    }

    #[must_use]
    pub fn unhealthy(
        store: impl Into<String>,
        data_class: DoctorDataClass,
        symptom: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            store: store.into(),
            data_class,
            severity: DoctorSeverity::Unhealthy,
            symptom: symptom.into(),
            remediation: Some(remediation.into()),
        }
    }
}
