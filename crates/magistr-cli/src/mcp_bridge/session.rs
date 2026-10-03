//! What the bridge remembers about its one client between owners.

use std::collections::HashMap;

use rmcp::model::ProtocolVersion;
use serde_json::{Map, Value, json};

use super::SessionPolicy;

/// One client's session as the bridge sees it: the durable identity stamped
/// on its calls, the `initialize` it sent (answered locally when no owner is
/// reachable, replayed to every later owner), the requests still waiting for
/// an owner's answer and the last tool listing. While no owner is reachable
/// the listing answers — the last one an owner gave, else the shared
/// capability definition — so the surface never disappears, not even before
/// any owner was ever reached.
pub(crate) struct ClientSession<P: SessionPolicy> {
    identity: String,
    policy: P,
    initialize_result: Option<Value>,
    initialize: Option<Value>,
    outstanding: HashMap<String, Outstanding>,
    tools: Option<Value>,
    replays: u64,
}

/// A request forwarded to an owner and not yet answered.
struct Outstanding {
    method: String,
    meta: Option<Map<String, Value>>,
}

/// A client message after the bridge has looked at it.
pub(crate) enum Inbound {
    /// Forward these bytes to the owner.
    Forward(String),
    /// The message asks for an answer; nothing was forwarded because no owner
    /// is linked. The bridge answers it locally.
    Unanswered(Value),
    /// A notification or response with no owner to receive it.
    Dropped,
}

impl<P: SessionPolicy> ClientSession<P> {
    pub(crate) fn new(identity: String, policy: P) -> Self {
        Self {
            identity,
            policy,
            initialize_result: None,
            initialize: None,
            outstanding: HashMap::new(),
            tools: None,
            replays: 0,
        }
    }

    /// The identity stamped on this client's calls.
    pub(crate) fn identity(&self) -> &str {
        &self.identity
    }

    /// Whether a request forwarded to the owner still awaits its answer.
    pub(crate) fn has_outstanding(&self) -> bool {
        !self.outstanding.is_empty()
    }

    /// Whether this client has initialized against some owner already, so a
    /// new owner must be initialized on its behalf.
    pub(crate) fn initialized(&self) -> bool {
        self.initialize.is_some()
    }

    /// Look at one client line: remember `initialize`, stamp the session
    /// identity on `tools/call`, and record what now awaits an answer.
    pub(crate) fn inbound(&mut self, line: &str, linked: bool) -> Inbound {
        let Ok(mut message) = serde_json::from_str::<Value>(line) else {
            // Not JSON: let the owner refuse it with its own words.
            return if linked {
                Inbound::Forward(line.to_owned())
            } else {
                Inbound::Dropped
            };
        };
        let method = message["method"].as_str().map(str::to_owned);
        let id = message.get("id").filter(|id| !id.is_null()).cloned();
        let mut stamped = false;
        match method.as_deref() {
            Some("initialize") => self.initialize = Some(message["params"].clone()),
            Some("tools/call") => stamped = self.stamp(&mut message),
            _ => {}
        }
        if !linked {
            return match (id, method) {
                (Some(_), Some(_)) => Inbound::Unanswered(message),
                _ => Inbound::Dropped,
            };
        }
        if let (Some(id), Some(method)) = (id, method) {
            let meta = message["params"]["_meta"].as_object().cloned();
            self.outstanding
                .insert(key(&id), Outstanding { method, meta });
        }
        Inbound::Forward(if stamped {
            message.to_string()
        } else {
            line.to_owned()
        })
    }

    /// Look at one owner line: settle the request it answers and keep the
    /// last successful tool listing.
    pub(crate) fn outbound(&mut self, line: &str) {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if message.get("method").is_some() {
            return;
        }
        let Some(id) = message.get("id").filter(|id| !id.is_null()) else {
            return;
        };
        if let Some(answered) = self.outstanding.remove(&key(id))
            && let Some(result) = message.get("result")
        {
            match answered.method.as_str() {
                "initialize" => self.initialize_result = Some(result.clone()),
                "tools/list" => self.tools = Some(result.clone()),
                _ => {}
            }
        }
    }

    /// The `initialize` to send a new owner on this client's behalf, under an
    /// id no client uses, and the `initialized` notification that follows it.
    pub(crate) fn replay(&mut self) -> Option<(Value, Value, Value)> {
        let params = self.initialize.clone()?;
        self.replays += 1;
        let id = json!(format!("magistr-bridge-replay-{}", self.replays));
        let initialize =
            json!({"jsonrpc": "2.0", "id": id, "method": "initialize", "params": params});
        let initialized = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        Some((id, initialize, initialized))
    }

    /// Refresh the client's discovered tools after a successful owner change.
    /// The initialize response owns whether this notification is supported.
    pub(crate) fn tools_changed(&self) -> Option<Value> {
        let info = self.initialize_result.clone().or_else(|| {
            let requested =
                serde_json::from_value(self.initialize.as_ref()?["protocolVersion"].clone())
                    .ok()?;
            self.policy.initialize(requested)
        })?;
        (info["capabilities"]["tools"]["listChanged"] == true)
            .then(|| json!({"jsonrpc":"2.0","method":"notifications/tools/list_changed"}))
    }

    /// Every request still waiting when the owner went away, answered with
    /// the typed refusal so the client sees one failure per call, never a
    /// silent hang or a replay against the next owner.
    pub(crate) fn owner_lost(&mut self, reason: &str) -> Vec<Value> {
        let outstanding: Vec<_> = self.outstanding.drain().collect();
        let mut answers: Vec<Value> = outstanding
            .iter()
            .map(|(id, waiting)| {
                let id: Value = serde_json::from_str(id).unwrap_or(Value::Null);
                self.local_answer(&id, &waiting.method, waiting.meta.as_ref(), reason)
            })
            .collect();
        answers.sort_by_key(|answer| answer["id"].to_string());
        answers
    }

    /// The local answer to a request that found no owner.
    pub(crate) fn answer_unavailable(&self, request: &Value, reason: &str) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request["method"].as_str().unwrap_or_default();
        let meta = request["params"]["_meta"].as_object();
        self.local_answer(&id, method, meta, reason)
    }

    /// The answer a client gets for `method` while no owner is reachable:
    /// `initialize` the shared capability negotiated for the recorded
    /// request, a tool listing the last listing an owner gave or the core
    /// definition, a tool call the retained `ENGINE_UNAVAILABLE` envelope
    /// (recovery `retry`), a ping an empty result, anything else a JSON-RPC
    /// error naming the reason.
    fn local_answer(
        &self,
        id: &Value,
        method: &str,
        meta: Option<&Map<String, Value>>,
        reason: &str,
    ) -> Value {
        match method {
            "initialize" => self.initialize_answer(id),
            "tools/list" => match self.tools.clone().or_else(|| self.policy.listing()) {
                Some(tools) => json!({"jsonrpc": "2.0", "id": id, "result": tools}),
                None => rpc_error(id, reason),
            },
            "tools/call" => self.policy.unavailable_call(id, meta, reason),
            "ping" => json!({"jsonrpc": "2.0", "id": id, "result": {}}),
            _ => rpc_error(id, reason),
        }
    }

    /// The recorded `initialize`, negotiated exactly as an owner would
    /// negotiate it. A request without a protocol version is invalid, as
    /// rmcp would refuse it.
    fn initialize_answer(&self, id: &Value) -> Value {
        let requested = self
            .initialize
            .as_ref()
            .and_then(|params| params.get("protocolVersion"))
            .and_then(|version| serde_json::from_value::<ProtocolVersion>(version.clone()).ok());
        match requested {
            Some(requested) => match self
                .initialize_result
                .clone()
                .or_else(|| self.policy.initialize(requested))
            {
                Some(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
                None => rpc_error(id, "the project owner is unavailable"),
            },
            None => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32602, "message": "initialize requires params.protocolVersion"},
            }),
        }
    }

    fn stamp(&self, message: &mut Value) -> bool {
        let Some(session_meta) = self.policy.session_meta() else {
            return false;
        };
        let Some(params) = message.get_mut("params") else {
            return false;
        };
        if params.is_null() {
            *params = json!({});
        }
        let Some(params) = params.as_object_mut() else {
            return false;
        };
        let meta = params.entry("_meta").or_insert_with(|| json!({}));
        if !meta.is_object() {
            return false;
        }
        let Some(meta) = meta.as_object_mut() else {
            return false;
        };
        if meta.contains_key(session_meta) {
            return false;
        }
        meta.insert(session_meta.to_owned(), json!(self.identity));
        true
    }
}

/// JSON-RPC ids are compared by their JSON text, so `1` and `"1"` differ as
/// the protocol says they do.
fn key(id: &Value) -> String {
    id.to_string()
}

fn rpc_error(id: &Value, reason: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": -32000, "message": format!("The connection is temporarily unavailable: {reason}")},
    })
}
