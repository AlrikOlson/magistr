//! One line of the app-server's stdout, as far as the provider reads it.
//!
//! Future methods and extra fields are accepted; broken JSON-RPC envelopes
//! are reported explicitly so a request cannot hang on a silently lost reply.

use super::rpc_error::RpcError;
use serde_json::Value;

/// A JSON-RPC line the app-server wrote.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Frame {
    /// The result of a request the provider sent.
    Response { id: Value, result: Value },
    /// The error a request the provider sent was refused with.
    Error { id: Value, error: RpcError },
    /// The server asks the provider something (an approval, a question).
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// The server tells the provider something.
    Notification { method: String, params: Value },
    /// Invalid JSON or a malformed JSON-RPC envelope.
    Malformed(String),
}

impl Frame {
    /// Parse one stdout line without echoing potentially sensitive payloads.
    pub fn parse(line: &str) -> Self {
        let value = match serde_json::from_str::<Value>(line) {
            Ok(value) => value,
            Err(error) => return Self::Malformed(format!("invalid JSON: {error}")),
        };
        Self::from_value(&value)
    }

    /// Parse an already-decoded line.
    pub fn from_value(value: &Value) -> Self {
        if !value.is_object()
            || value
                .get("id")
                .is_some_and(|id| !id.is_string() && !id.is_i64() && !id.is_u64())
        {
            return Self::Malformed(
                "expected a JSON-RPC object with a string or integer id".into(),
            );
        }
        if value.get("result").is_some() && value.get("error").is_some() {
            return Self::Malformed("response contains both result and error".into());
        }
        if value.get("method").is_some()
            && (value.get("result").is_some() || value.get("error").is_some())
        {
            return Self::Malformed("request contains response fields".into());
        }
        if let Some(error) = value.get("error")
            && (error["code"].as_i64().is_none() || error["message"].as_str().is_none())
        {
            return Self::Malformed("invalid JSON-RPC error object".into());
        }
        let id = value.get("id").cloned();
        match (value["method"].as_str(), id) {
            (Some(method), Some(id)) => Self::Request {
                id,
                method: method.to_owned(),
                params: value["params"].clone(),
            },
            (Some(method), None) => Self::Notification {
                method: method.to_owned(),
                params: value["params"].clone(),
            },
            (None, Some(id)) if value.get("error").is_some() => Self::Error {
                id,
                error: RpcError::parse(&value["error"]),
            },
            (None, Some(id)) if value.get("result").is_some() => Self::Response {
                id,
                result: value["result"].clone(),
            },
            _ => Self::Malformed("unrecognized JSON-RPC envelope".into()),
        }
    }

    /// The thread a request or notification is about: `params.threadId`,
    /// or the thread's own id for `thread/started`.
    pub fn thread_id(&self) -> Option<&str> {
        let params = match self {
            Self::Request { params, .. } | Self::Notification { params, .. } => params,
            _ => return None,
        };
        params["threadId"]
            .as_str()
            .or_else(|| params["thread"]["id"].as_str())
    }

    /// Turn association used to discard late frames from earlier turns.
    pub fn turn_id(&self) -> Option<&str> {
        match self {
            Self::Request { params, .. } | Self::Notification { params, .. } => params["turnId"]
                .as_str()
                .or_else(|| params["turn"]["id"].as_str()),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "frame.tests.rs"]
mod tests;
