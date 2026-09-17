use super::{PluginComponent, PluginGuestPolicy};

/// One installed lang pack as the owner admitted it at open: the carrier
/// bytes and their hash (the breaker key), the effective capabilities, the
/// guest policy and the trust verdict. This is the configuration every
/// worker incarnation restores — captured once, so a replacement worker
/// registers exactly what the owner admitted, not whatever is on disk later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedLangPack {
    /// Wire name, e.g. "workbench.lang-pack-gleam".
    pub plugin: String,
    pub component: PluginComponent,
    pub capabilities: Vec<String>,
    /// Admission-time degradations (an ignored grant), repeated on every
    /// status row for this pack.
    pub notes: Vec<String>,
    pub policy: PluginGuestPolicy,
    /// Host trust verdict; selects the native promotion lane only.
    pub blessed: bool,
}
