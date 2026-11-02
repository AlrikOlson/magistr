use super::{ExportFormat, ExportTarget};

/// A typed request for the plan export capability.
#[derive(Debug, Clone, Copy)]
pub struct ExportRequest {
    pub format: ExportFormat,
    pub target: ExportTarget,
}
