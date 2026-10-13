//! Translate only the protocol fields pinned by the recorded schema.

use crate::AgentError;
use agent_client_protocol::schema::v1::{
    ContentBlock, ContentChunk, Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus, SessionUpdate,
    TextContent, ToolCall, ToolCallLocation, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
    ToolKind, UsageUpdate,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct Translator {
    streamed: HashSet<String>,
    thought_streamed: HashSet<String>,
    command_output: HashMap<String, String>,
    pub items: HashMap<String, Value>,
}

impl Translator {
    pub fn notification(
        &mut self,
        method: &str,
        params: &Value,
    ) -> Result<Vec<SessionUpdate>, AgentError> {
        match method {
            "item/agentMessage/delta"
            | "item/reasoning/textDelta"
            | "item/reasoning/summaryTextDelta" => {
                let id = required(params, "itemId")?;
                let text = required(params, "delta")?;
                let update = if method == "item/agentMessage/delta" {
                    self.streamed.insert(id.into());
                    SessionUpdate::AgentMessageChunk(chunk(text))
                } else {
                    self.thought_streamed.insert(id.into());
                    SessionUpdate::AgentThoughtChunk(chunk(text))
                };
                Ok(vec![update])
            }
            "item/started" => self.item(&params["item"], false),
            "item/completed" => self.item(&params["item"], true),
            "item/commandExecution/outputDelta" => {
                let id = required(params, "itemId")?;
                let delta = required(params, "delta")?;
                let output = self.command_output.entry(id.to_owned()).or_default();
                output.push_str(delta);
                Ok(vec![SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                    id.to_owned(),
                    ToolCallUpdateFields::new().content(vec![
                        ContentBlock::Text(TextContent::new(output.clone())).into(),
                    ]),
                ))])
            }
            "thread/tokenUsage/updated" => {
                let usage = &params["tokenUsage"];
                let used = usage["last"]["totalTokens"].as_u64().ok_or_else(|| {
                    AgentError::provider("malformed Codex token usage: missing last.totalTokens")
                })?;
                Ok(usage["modelContextWindow"]
                    .as_u64()
                    .filter(|size| *size > 0)
                    .map(|size| SessionUpdate::UsageUpdate(UsageUpdate::new(used, size)))
                    .into_iter()
                    .collect())
            }
            "turn/plan/updated" => {
                let plan = params["plan"].as_array().ok_or_else(|| {
                    AgentError::provider("malformed Codex plan: missing plan array")
                })?;
                let entries = plan
                    .iter()
                    .map(|entry| {
                        let status = match required(entry, "status")? {
                            "pending" => PlanEntryStatus::Pending,
                            "inProgress" => PlanEntryStatus::InProgress,
                            "completed" => PlanEntryStatus::Completed,
                            status => {
                                return Err(AgentError::provider(format!(
                                    "unknown Codex plan status: {status}"
                                )));
                            }
                        };
                        Ok(PlanEntry::new(
                            required(entry, "step")?,
                            PlanEntryPriority::Medium,
                            status,
                        ))
                    })
                    .collect::<Result<Vec<_>, AgentError>>()?;
                Ok(vec![SessionUpdate::Plan(Plan::new(entries))])
            }
            _ => Ok(Vec::new()),
        }
    }

    pub fn replay_item(&mut self, item: &Value) -> Result<Vec<SessionUpdate>, AgentError> {
        if item["type"] == "userMessage" {
            return Ok(item["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|content| content["type"] == "text")
                .filter_map(|content| content["text"].as_str())
                .map(|text| SessionUpdate::UserMessageChunk(chunk(text)))
                .collect());
        }
        let mut result = self.item(item, false)?;
        result.extend(self.item(item, true)?);
        Ok(result)
    }

    fn item(&mut self, item: &Value, completed: bool) -> Result<Vec<SessionUpdate>, AgentError> {
        let id = required(item, "id")?;
        let kind = required(item, "type")?;
        match kind {
            "agentMessage" if completed => {
                let text = required(item, "text")?;
                Ok((!self.streamed.remove(id) && !text.is_empty())
                    .then(|| SessionUpdate::AgentMessageChunk(chunk(text)))
                    .into_iter()
                    .collect())
            }
            "reasoning" if completed => {
                if self.thought_streamed.remove(id) {
                    return Ok(Vec::new());
                }
                let summary = item["summary"]
                    .as_array()
                    .filter(|summary| !summary.is_empty())
                    .or_else(|| item["content"].as_array());
                Ok(summary
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|text| SessionUpdate::AgentThoughtChunk(chunk(text)))
                    .collect())
            }
            "commandExecution"
            | "fileChange"
            | "mcpToolCall"
            | "dynamicToolCall"
            | "webSearch"
            | "imageView"
            | "collabAgentToolCall" => {
                let mut updates = Vec::new();
                if !self.items.contains_key(id) {
                    updates.push(SessionUpdate::ToolCall(
                        ToolCall::new(id.to_owned(), title(item))
                            .kind(tool_kind(kind))
                            .status(ToolCallStatus::InProgress)
                            .raw_input(item.get("arguments").unwrap_or(item).clone())
                            .locations(locations(item)),
                    ));
                }
                self.items.insert(id.into(), item.clone());
                if completed {
                    let failed = matches!(item["status"].as_str(), Some("failed" | "declined"))
                        || item["exitCode"].as_i64().is_some_and(|exit| exit != 0)
                        || !item["error"].is_null()
                        || item["success"].as_bool() == Some(false);
                    let mut fields = ToolCallUpdateFields::new()
                        .status(if failed {
                            ToolCallStatus::Failed
                        } else {
                            ToolCallStatus::Completed
                        })
                        .raw_output(if kind == "mcpToolCall" {
                            if item["error"].is_null() {
                                item["result"].clone()
                            } else {
                                item["error"].clone()
                            }
                        } else {
                            item.clone()
                        });
                    let texts: Vec<String> = match kind {
                        "mcpToolCall" => item["result"]["content"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|block| block["text"].as_str().map(str::to_owned))
                            .collect(),
                        "dynamicToolCall" => item["contentItems"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|block| block["text"].as_str().map(str::to_owned))
                            .collect(),
                        "commandExecution" => {
                            let streamed = self.command_output.remove(id);
                            item["aggregatedOutput"]
                                .as_str()
                                .map(str::to_owned)
                                .or(streamed)
                                .into_iter()
                                .collect()
                        }
                        _ => Vec::new(),
                    };
                    if !texts.is_empty() {
                        fields = fields.content(
                            texts
                                .into_iter()
                                .map(|text| ContentBlock::Text(TextContent::new(text)).into())
                                .collect::<Vec<_>>(),
                        );
                    }
                    updates.push(SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                        id.to_owned(),
                        fields,
                    )));
                }
                Ok(updates)
            }
            _ => Ok(Vec::new()),
        }
    }
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str, AgentError> {
    value[field].as_str().ok_or_else(|| {
        AgentError::provider(format!("malformed Codex notification: missing {field}"))
    })
}

fn chunk(text: &str) -> ContentChunk {
    ContentChunk::new(ContentBlock::Text(TextContent::new(text)))
}

fn tool_kind(kind: &str) -> ToolKind {
    match kind {
        "commandExecution" => ToolKind::Execute,
        "fileChange" => ToolKind::Edit,
        "webSearch" => ToolKind::Search,
        "imageView" => ToolKind::Read,
        _ => ToolKind::Other,
    }
}

fn title(item: &Value) -> String {
    match item["type"].as_str() {
        Some("commandExecution") => item["command"].as_str().unwrap_or("Command").into(),
        Some("fileChange") => "Edit files".into(),
        Some("mcpToolCall") => format!(
            "mcp__{}__{}",
            item["server"].as_str().unwrap_or_default(),
            item["tool"].as_str().unwrap_or_default()
        ),
        Some("dynamicToolCall") => match item["namespace"].as_str() {
            Some(namespace) => format!("{namespace}.{}", item["tool"].as_str().unwrap_or("Tool")),
            None => item["tool"].as_str().unwrap_or("Tool").into(),
        },
        Some("webSearch") => format!("Search: {}", item["query"].as_str().unwrap_or_default()),
        Some(kind) => kind.into(),
        None => "Tool".into(),
    }
}

fn locations(item: &Value) -> Vec<ToolCallLocation> {
    item["changes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|change| change["path"].as_str())
        .map(ToolCallLocation::new)
        .collect()
}

#[cfg(test)]
#[path = "translate.tests.rs"]
mod tests;
