//! Invoke current public operations while testing native owner behavior.
use serde_json::{Value, json};

pub fn request(name: &str, args: Value) -> (String, Value, bool) {
    let public = match name {
        "magistr_context" => "context",
        "ship_plan" => "plan_work",
        "ship_start" => "start_work",
        "ship_record" => "record_progress",
        "ship_check" => "check_work",
        "ship_complete" => "finish_work",
        name => name,
    };
    if [
        "context",
        "search",
        "read",
        "edit",
        "write",
        "plan_work",
        "start_work",
        "record_progress",
        "check_work",
        "finish_work",
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
