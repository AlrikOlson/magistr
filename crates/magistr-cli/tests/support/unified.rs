//! Invoke current public operations while testing native owner behavior.
use serde_json::{Value, json};

pub fn request(name: &str, args: Value) -> (String, Value, bool) {
    let public = match name {
        "magistr_context" => "context",
        name => name,
    };
    if [
        "context",
        "search",
        "read",
        "edit",
        "write",
        "arrange_steps",
        "start_step",
        "record_progress",
        "check_step",
        "finish_step",
        "discover",
        "execute",
    ]
    .contains(&public)
    {
        return (public.into(), args, false);
    }
    let source = format!("return magistr.call({}, {});", json!(public), args);
    (
        "execute".into(),
        json!({"source":source,"maxOperations":1}),
        true,
    )
}

pub fn value(body: Value, composed: bool) -> Value {
    if !composed {
        return body;
    }
    if body["ok"] == true {
        return body["value"].clone();
    }
    if let Some(receipt) = body["receipts"].as_array().and_then(|r| r.last()) {
        return receipt["result"].clone();
    }
    body.get("failure")
        .cloned()
        .unwrap_or_else(|| panic!("Missing composition failure: {body}"))
}
