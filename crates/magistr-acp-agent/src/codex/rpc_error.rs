//! Preserve the runtime's structured failure classification.

use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RpcError {
    code: i64,
    message: String,
    data: Value,
}

impl RpcError {
    pub fn parse(value: &Value) -> Self {
        Self {
            code: value["code"].as_i64().unwrap_or_default(),
            message: value["message"].as_str().unwrap_or_default().into(),
            data: value["data"].clone(),
        }
    }

    pub fn classification(value: &Value) -> Option<String> {
        match value {
            Value::String(name) => Some(name.clone()),
            Value::Object(map) => {
                let (name, detail) = map.iter().next()?;
                Some(match detail["httpStatusCode"].as_u64() {
                    Some(status) => format!("{name} (HTTP {status})"),
                    None => name.clone(),
                })
            }
            _ => None,
        }
    }

    pub fn turn_failure(error: &Value) -> String {
        let message = error["message"]
            .as_str()
            .unwrap_or("Codex turn failed without an error message");
        match Self::classification(&error["codexErrorInfo"]) {
            Some(info) => format!("{message} [{info}]"),
            None => message.into(),
        }
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (code {})", self.message, self.code)?;
        if let Some(info) = Self::classification(&self.data["codexErrorInfo"]) {
            write!(f, " [{info}]")?;
        }
        Ok(())
    }
}
