use serde::Serialize;

/// One ranking signal's observable state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingSignalStatus {
    /// Wire name, e.g. "workbench.ranking-visibility".
    pub plugin: String,
    pub tier: String,
    pub promoted: bool,
    pub healthy: bool,
    /// Boot/last outcome kind ("ok", "breakered", "manifest-unreadable", …).
    pub outcome: String,
    pub detail: String,
    pub cached_hits: usize,
}
