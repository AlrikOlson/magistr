//! A thread read from the Codex runtime, with ACP replay already translated.

use super::translate::Translator;
use crate::AgentError;
use agent_client_protocol::schema::v1::SessionUpdate;
use serde_json::Value;

#[derive(Debug, Clone)]
/// Persisted runtime history and its translated ACP notifications.
pub struct ThreadRecord {
    /// Codex's persisted thread id.
    pub id: String,
    /// The model stored on the thread, if the runtime supplied it.
    pub model: Option<String>,
    /// Full hydrated history in the runtime's original order.
    pub replay: Vec<SessionUpdate>,
    /// The original thread object for callers that need other metadata.
    pub raw: Value,
}

#[cfg(test)]
#[path = "thread_record.tests.rs"]
mod tests;

impl ThreadRecord {
    pub(crate) fn parse(response: &Value) -> Result<Self, AgentError> {
        let raw = &response["thread"];
        let id = raw["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                AgentError::provider("malformed Codex thread response: missing thread id")
            })?;
        let turns = raw["turns"].as_array().ok_or_else(|| {
            AgentError::provider("malformed Codex thread response: missing hydrated turns")
        })?;
        let mut replay = Vec::new();
        for turn in turns {
            let mut translator = Translator::default();
            let items = turn["items"].as_array().ok_or_else(|| {
                AgentError::provider("malformed Codex thread history: missing items")
            })?;
            if turn["itemsView"]
                .as_str()
                .is_some_and(|view| view != "full")
            {
                return Err(AgentError::provider(
                    "Codex returned incomplete thread history; full-history hydration is required",
                ));
            }
            for item in items {
                replay.extend(translator.replay_item(item)?);
            }
        }
        let model = response["model"]
            .as_str()
            .or_else(|| raw["model"].as_str())
            .map(str::to_owned);
        Ok(Self {
            id: id.into(),
            model,
            replay,
            raw: raw.clone(),
        })
    }
}
