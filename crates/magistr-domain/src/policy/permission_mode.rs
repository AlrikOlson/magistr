use serde::{Deserialize, Serialize};

/// Explicit local allowance mode; neither mode bypasses remembered or organization rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionMode {
    /// Apply profile defaults and the existing safe-operation allowances.
    Standard,
    /// Also allow the configured workflow's standard development operations.
    Workflow,
}
