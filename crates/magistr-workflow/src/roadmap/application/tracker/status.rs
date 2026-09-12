/// Read-only tracker configuration and native scope under the bound project.
#[derive(Debug, serde::Serialize)]
pub struct TrackerStatus {
    pub project_id: String,
    pub enabled: bool,
    pub provider: Option<String>,
    pub target: Option<String>,
    pub included: usize,
    pub not_included: usize,
    pub inherits_new_chunks: bool,
}
