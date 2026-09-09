//! Model information extracted from a provider probe.

use super::probe_model::ProbeModel;

/// Model information extracted from a provider probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderModelSet {
    /// Stable provider key (e.g. `"claude"`, `"copilot"`).
    pub provider_id: String,
    /// Human-readable provider name (e.g. `"Claude Code"`, `"GitHub Copilot"`).
    pub label: String,
    /// Available models advertised by this provider.
    pub models: Vec<ProbeModel>,
    /// Whether the probe succeeded (false = provider binary exists but probe failed).
    pub probed: bool,
}
