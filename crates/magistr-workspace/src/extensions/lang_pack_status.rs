use serde::Serialize;

/// One pack's boot verdict — the host-side half of the lane's observability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LangPackStatus {
    /// Wire name, e.g. "workbench.lang-pack-gleam".
    pub plugin: String,
    pub ok: bool,
    /// Sandbox outcome kind or a boot-stage refusal ("missing-component",
    /// "manifest-unreadable", "breakered", "registration-refused").
    pub outcome: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
    /// "native" (blessed promotion) | "wasm" — the lane the registry chose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queries_ok: Option<bool>,
    /// Every shadowed claim or degradation, verbatim.
    pub notes: Vec<String>,
}
