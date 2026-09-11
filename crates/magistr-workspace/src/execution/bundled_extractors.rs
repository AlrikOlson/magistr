use super::{DiagnosticExtractor, cargo_diagnostics::CargoDiagnostics};

/// The bundled host-side diagnostic extractors in trust order.
#[must_use]
pub fn bundled_extractors() -> Vec<Box<dyn DiagnosticExtractor>> {
    vec![Box::new(CargoDiagnostics)]
}
