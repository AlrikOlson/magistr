use super::{Diagnostic, RunOutput};

/// The host-side diagnostics seam, consulted before any runner parser: the
/// first claimant decides; `None` from `extract` means "no diagnostics to
/// report here" and the runner parsers get their turn. Host-side by
/// construction — nothing here is a plugin surface; the bundled set is the
/// open registry [`super::bundled_extractors`].
pub trait DiagnosticExtractor: Send + Sync {
    fn name(&self) -> &'static str;
    fn can_extract(&self, command_line: &str) -> bool;
    fn extract(&self, output: &RunOutput) -> Option<Vec<Diagnostic>>;
}
