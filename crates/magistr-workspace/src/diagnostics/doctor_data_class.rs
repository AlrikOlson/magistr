use serde::Serialize;

/// The remediation vocabulary: cache data is rebuildable, durable data is
/// set aside rather than deleted, and some stores are neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DoctorDataClass {
    #[serde(rename = "cache")]
    Cache,
    #[serde(rename = "durable")]
    Durable,
    #[serde(rename = "n/a")]
    NotApplicable,
}
