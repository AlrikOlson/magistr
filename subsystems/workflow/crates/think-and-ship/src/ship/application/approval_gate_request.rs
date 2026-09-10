use crate::ship::gate::GateOption;

/// A typed request to open the existing human approval gate.
#[derive(Debug, Clone)]
pub struct ApprovalGateRequest {
    pub question: String,
    pub body: String,
    pub options: Option<Vec<GateOption>>,
    pub default_key: Option<String>,
    pub timeout_secs: Option<i64>,
}
