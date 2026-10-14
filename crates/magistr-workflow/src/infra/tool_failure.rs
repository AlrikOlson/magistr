//! Explicit recovery guidance. No constructor infers that a failed write was rolled back.
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

pub struct ToolFailure;

impl ToolFailure {
    pub fn rejected(
        code: &str,
        message: impl Into<String>,
        problems: Vec<Value>,
    ) -> CallToolResult {
        Self::build(
            code,
            message.into(),
            problems,
            "none",
            json!({
                "action":"correct_arguments",
                "instruction":"Correct the listed arguments or prerequisite, then submit once. Do not repeat an unchanged invalid call."
            }),
        )
    }

    pub fn store(code: &str, error: &magistr_workflow_contracts::WriteError) -> CallToolResult {
        let recovery = if error.effect() == "unknown" {
            json!({
                "action": "read_back",
                "instruction": "Read the affected object's status or operation receipt before retrying. Saving may have completed."
            })
        } else {
            json!({
                "action": "restore_access",
                "instruction": "Resolve the saved-work error, read the affected object, then retry the change."
            })
        };
        Self::build(
            code,
            error.message.clone(),
            vec![json!({"write_failure": error.kind})],
            error.effect(),
            recovery,
        )
    }

    pub fn uncertain(code: &str, message: impl Into<String>) -> CallToolResult {
        Self::build(
            code,
            message.into(),
            Vec::new(),
            "unknown",
            json!({
                "action":"read_back",
                "instruction":"Read the affected object's status or operation receipt before retrying. Effects may have occurred; do not repeat the write blindly."
            }),
        )
    }

    /// Only for a producer that knows no effect started and the fault is transient.
    pub fn transient(code: &str, message: impl Into<String>, backoff_ms: u64) -> CallToolResult {
        Self::build(
            code,
            message.into(),
            Vec::new(),
            "none",
            json!({
                "action":"retry_bounded", "max_attempts":3, "backoff_ms":backoff_ms,
                "instruction":"Wait the backoff, retry at most three times, then surface the original diagnostic. Do not retry uncertain writes."
            }),
        )
    }

    fn build(
        code: &str,
        message: String,
        problems: Vec<Value>,
        effect: &str,
        recovery: Value,
    ) -> CallToolResult {
        CallToolResult::structured_error(json!({
            "ok":false, "code":code, "error_kind":code,
            "message":message, "problems":problems, "effect":effect, "recovery":recovery
        }))
    }
}
