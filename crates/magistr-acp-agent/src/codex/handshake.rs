//! Runtime-managed authentication and discovered model/effort catalog.

use super::process::CodexProcess;
use crate::{AgentError, ModelChoice};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct Handshake {
    pub models: Vec<ModelChoice>,
    efforts: HashMap<String, Vec<String>>,
    defaults: HashMap<String, String>,
}

impl Handshake {
    pub async fn discover(process: &CodexProcess) -> Result<Self, AgentError> {
        let initialized = process.request("initialize", json!({"clientInfo":{"name":"magistr","title":"Magistr","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":false}})).await?;
        if initialized["userAgent"].as_str().is_none() {
            return Err(AgentError::provider(
                "malformed Codex initialize response: missing userAgent",
            ));
        }
        process.write(&json!({"method":"initialized"})).await?;
        Self::check_account(
            &process
                .request("account/read", json!({"refreshToken":false}))
                .await?,
        )?;
        let mut handshake = Self::default();
        let mut cursor = None;
        let mut seen = HashSet::new();
        loop {
            let mut params = json!({"includeHidden":false,"limit":50});
            if let Some(cursor) = &cursor {
                params["cursor"] = json!(cursor);
            }
            let page = process.request("model/list", params).await?;
            handshake.add_page(&page)?;
            cursor = page["nextCursor"].as_str().map(str::to_owned);
            let Some(cursor) = &cursor else {
                break;
            };
            if !seen.insert(cursor.clone()) {
                return Err(AgentError::provider(
                    "Codex model/list repeated its pagination cursor",
                ));
            }
        }
        if handshake.models.is_empty() {
            return Err(AgentError::provider(
                "Codex account has no available models",
            ));
        }
        Ok(handshake)
    }

    pub fn check_account(response: &Value) -> Result<(), AgentError> {
        match response["account"]["type"].as_str() {
            Some("chatgpt") => Ok(()),
            Some("apiKey") => Err(AgentError::provider(
                "Codex is using an API key. Sign in to Codex with ChatGPT to use the subscription provider; API billing fallback is disabled",
            )),
            _ => Err(AgentError::provider(
                "Codex has no supported runtime ChatGPT login. Run `codex login` and sign in with ChatGPT; API billing fallback is disabled",
            )),
        }
    }

    fn add_page(&mut self, page: &Value) -> Result<(), AgentError> {
        let data = page["data"].as_array().ok_or_else(|| {
            AgentError::provider("malformed Codex model/list response: missing data array")
        })?;
        for row in data {
            if row["hidden"].as_bool() == Some(true) {
                continue;
            }
            let id = row["model"]
                .as_str()
                .or_else(|| row["id"].as_str())
                .filter(|id| !id.is_empty())
                .ok_or_else(|| AgentError::provider("malformed Codex model: missing model id"))?;
            if self.efforts.contains_key(id) {
                continue;
            }
            let efforts: Vec<_> = row["supportedReasoningEfforts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|effort| effort["reasoningEffort"].as_str().map(str::to_owned))
                .collect();
            let default = row["defaultReasoningEffort"]
                .as_str()
                .filter(|default| efforts.iter().any(|effort| effort == default))
                .ok_or_else(|| {
                    AgentError::provider(format!(
                        "Codex model {id} has no supported default reasoning effort"
                    ))
                })?;
            self.efforts.insert(id.into(), efforts);
            self.defaults.insert(id.into(), default.into());
            let choice = ModelChoice::new(id, row["displayName"].as_str().unwrap_or(id));
            if row["isDefault"].as_bool() == Some(true) {
                self.models.insert(0, choice);
            } else {
                self.models.push(choice);
            }
        }
        Ok(())
    }

    pub fn effort(&self, model: &str, wanted: Option<&str>) -> Result<String, AgentError> {
        let efforts = self
            .efforts
            .get(model)
            .ok_or_else(|| AgentError::UnknownModel(model.into()))?;
        let chosen = wanted
            .or_else(|| self.defaults.get(model).map(String::as_str))
            .ok_or_else(|| {
                AgentError::provider(format!(
                    "Codex model {model} has no default reasoning effort"
                ))
            })?;
        if !efforts.iter().any(|effort| effort == chosen) {
            return Err(AgentError::provider(format!(
                "Codex model {model} does not support reasoning effort {chosen}; supported: {}",
                efforts.join(", ")
            )));
        }
        Ok(chosen.into())
    }
}

#[cfg(test)]
#[path = "handshake.tests.rs"]
mod tests;
