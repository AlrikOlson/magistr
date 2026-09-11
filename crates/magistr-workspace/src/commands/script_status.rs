use serde::Serialize;

/// Whether the guest program returned or threw. A throw is data, like a
/// failing test; sandbox kills are typed refusals instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptStatus {
    Ok,
    Threw,
}
