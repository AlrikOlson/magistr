use serde::Serialize;

/// One conformance probe: an id, a verdict and the evidence behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TckProbe {
    pub id: String,
    pub passed: bool,
    pub detail: String,
}

impl TckProbe {
    #[must_use]
    pub fn new(id: impl Into<String>, passed: bool, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            passed,
            detail: detail.into(),
        }
    }
}
