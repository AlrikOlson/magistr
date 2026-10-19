//! Bounded composition reuses the workspace's JavaScript/WASM sandbox. Re-entry
//! supplies prior results to pure JavaScript; native effects are never replayed.
use super::{
    client,
    service::{Service, response},
};
use rmcp::{
    ErrorData,
    model::CallToolResult,
    service::{RequestContext, RoleServer},
};
use serde_json::{Value, json};

const MAX_RECEIPT_BYTES: usize = 1024 * 1024;
const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_OPERATIONS: u64 = 32;

#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Execution {
    /// Synchronous JavaScript. Encoded source is limited to 65536 bytes.
    #[schemars(length(min = 1, max = 65536))]
    source: String,
    #[serde(default = "default_operation_limit")]
    #[schemars(range(min = 1, max = 32))]
    max_operations: u64,
}
fn default_operation_limit() -> u64 {
    MAX_OPERATIONS
}

pub(super) async fn execute(
    service: &Service,
    args: &Value,
    context: &RequestContext<RoleServer>,
) -> Result<CallToolResult, ErrorData> {
    let args: Execution = serde_json::from_value(args.clone())
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;
    let source = args.source.as_str();
    if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
        return Err(ErrorData::invalid_params(
            "source must contain 1..65536 bytes of JavaScript",
            None,
        ));
    }
    let limit = args.max_operations;
    if !(1..=MAX_OPERATIONS).contains(&limit) {
        return Err(ErrorData::invalid_params(
            "maxOperations must be 1..32",
            None,
        ));
    }
    let mut receipts = Vec::new();
    for _ in 0..=limit {
        if context.ct.is_cancelled() {
            return Ok(stopped(
                receipts,
                json!({"code":"cancelled","message":"No further operations were started."}),
            ));
        }
        let program = program(source, &receipts);
        let evaluated = client::call(&service.workspace, "script", json!({
            "source":program,"dryRun":true,"fuel":50000000,"memoryCapBytes":33554432,"budget":20000
        }), context.meta.clone(), context.ct.clone()).await;
        let result = match evaluated {
            Ok(result) => result,
            Err(error) => {
                return Ok(stopped(
                    receipts,
                    json!({"code":"sandbox_failed","message":error.to_string()}),
                ));
            }
        };
        let envelope = client::value(&result);
        if result.is_error == Some(true) || envelope["ok"] == false {
            return Ok(stopped(receipts, envelope));
        }
        let row = &envelope["results"][0];
        if row["error"].is_string() || row["queuedWrites"].as_u64().unwrap_or(0) != 0 {
            return Ok(stopped(
                receipts,
                json!({"code":"sandbox_failed","details":row}),
            ));
        }
        let frame = &row["value"];
        match frame["kind"].as_str() {
            Some("done") => {
                return Ok(response(
                    json!({"ok":true,"receipts":receipts,"value":frame["value"]}),
                    false,
                ));
            }
            Some("call") => {
                if receipts.len() >= limit as usize {
                    return Ok(stopped(
                        receipts,
                        json!({"code":"operation_limit","message":"No further operation was started."}),
                    ));
                }
                let Some(name) = frame["operation"].as_str() else {
                    return Ok(stopped(
                        receipts,
                        json!({"code":"invalid_operation","message":"magistr.call requires an operation name."}),
                    ));
                };
                let arguments = frame["arguments"].clone();
                let result = match service.invoke(name, arguments.clone(), context).await {
                    Ok(result) => result,
                    Err(error) => {
                        return Ok(stopped(
                            receipts,
                            json!({"operation":name,"code":"operation_failed","message":error.to_string(),"effect":"unknown","recovery":"Read current state before retrying a change."}),
                        ));
                    }
                };
                let value = client::value(&result);
                let failed = result.is_error == Some(true) || value["ok"] == false;
                receipts.push(
                    json!({"operation":name,"arguments":arguments,"result":value,"failed":failed}),
                );
                if failed {
                    return Ok(stopped(
                        receipts,
                        json!({"operation":name,"code":"operation_failed","message":"Stopped after this response. Earlier effects remain applied; do not replay the script."}),
                    ));
                }
                if serde_json::to_vec(&receipts).map_err(client::error)?.len() > MAX_RECEIPT_BYTES {
                    return Ok(stopped(
                        receipts,
                        json!({"code":"receipt_limit","message":"Completed effects are included; no further operation was started."}),
                    ));
                }
            }
            _ => {
                return Ok(stopped(
                    receipts,
                    json!({"code":"invalid_program_result","details":frame,"truncation":envelope.get("truncation")}),
                ));
            }
        }
    }
    Ok(stopped(receipts, json!({"code":"operation_limit"})))
}

fn stopped(receipts: Vec<Value>, failure: Value) -> CallToolResult {
    response(
        json!({"ok":false,"receipts":receipts,"failure":failure,"automaticReplay":false}),
        true,
    )
}

fn program(source: &str, receipts: &[Value]) -> String {
    format!(
        r#"
const history = {history};
let cursor = 0;
let pending = null;
const marker = {{}};
const magistr = Object.freeze({{call(operation, args = {{}}) {{
    if (typeof operation !== "string" || args === null || typeof args !== "object" || Array.isArray(args)) throw Error("magistr.call expects a name and argument object");
    const call = {{kind:"call", operation, arguments:args}};
    if (cursor < history.length) {{
        const prior = history[cursor++];
        if (prior.operation !== operation || JSON.stringify(prior.arguments) !== JSON.stringify(args))
            throw Error("Operation sequence changed during evaluation; prior effects were not replayed");
        return prior.result;
    }}
    pending = call;
    throw marker;
}}}});
try {{
    const value = (function(magistr, workspace) {{
{source}
    }})(magistr, undefined);
    if (pending) return pending;
    if (cursor !== history.length) throw Error("Operation sequence ended before recorded effects");
    if (value && typeof value.then === "function") throw Error("Use synchronous magistr.call; async results are not supported");
    return {{kind:"done", value: value === undefined ? null : value}};
}} catch (error) {{
    if (error === marker && pending) return pending;
    throw error;
}}
"#,
        history = Value::Array(receipts.to_vec())
    )
}

#[cfg(test)]
#[path = "composition.tests.rs"]
mod tests;
