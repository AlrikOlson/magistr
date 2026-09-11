use serde::Serialize;

use super::{DoctorFinding, DoctorSectionId};

/// One report section: the artifact it was read from, its findings, and a
/// machine-readable detail the bundle carries as `<id>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorSection {
    pub id: DoctorSectionId,
    /// The on-disk artifact or mechanism this section was read from.
    pub source: String,
    pub findings: Vec<DoctorFinding>,
    pub detail: serde_json::Value,
}
