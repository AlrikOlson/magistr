//! Workflow errors while the bridge waits for an owner. Discovery comes from
//! the admitted owner's initialize and listing responses, never a second store.
use crate::mcp_bridge::SessionPolicy;
use serde_json::{Map, Value, json};

pub(crate) struct Policy;

impl SessionPolicy for Policy {
    fn unavailable_call(
        &self,
        id: &Value,
        _meta: Option<&Map<String, Value>>,
        reason: &str,
    ) -> Value {
        let body = json!({
            "ok": false, "code": "engine_unavailable", "error_kind": "engine_unavailable",
            "message": "The Magistr connection was interrupted. The next call reconnects.",
            "effect": "unknown",
            "recovery": { "action": "read_back",
                "instruction": "Check the affected object or operation receipt before retrying a change." },
            "details": reason,
        });
        json!({"jsonrpc": "2.0", "id": id, "result": {
            "content": [{"type": "text", "text": body.to_string()}],
            "structuredContent": body, "isError": true,
        }})
    }
}

#[cfg(test)]
#[path = "owner_policy.tests.rs"]
mod tests;
