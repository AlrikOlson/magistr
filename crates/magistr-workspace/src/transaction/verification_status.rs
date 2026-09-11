/// Actual shadow validation coverage attached to the exact final preview bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Verified,
    ExistingDiagnostics,
    Unsupported,
    Binary,
    Deletion,
    Unchanged,
}
