use crate::run::approval::ApprovalOption;

/// A typed request to open the existing human approval approval.
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub question: String,
    pub body: String,
    pub options: Option<Vec<ApprovalOption>>,
    pub default_key: Option<String>,
    pub timeout_secs: Option<i64>,
}
