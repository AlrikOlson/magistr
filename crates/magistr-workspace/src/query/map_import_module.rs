use serde::Serialize;

/// Source-level import frequency, without a name-resolution claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapImportModule {
    pub module: String,
    pub count: usize,
}
