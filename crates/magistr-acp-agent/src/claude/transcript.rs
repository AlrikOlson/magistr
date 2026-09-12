//! The persisted session transcript, replayed on `session/load`.
//!
//! `--resume` restores the conversation inside the CLI but replays nothing on
//! the wire, so the history a client expects on `session/load` is read from
//! where Claude Code persists it: `<config dir>/projects/<cwd with every
//! path separator and drive colon as `-`>/<session id>.jsonl`, one JSON
//! object per line, of which the
//! `user` and `assistant` lines carry the messages. A transcript that is
//! missing, malformed, or unsupported fails before a runtime is resumed.

use std::path::{Path, PathBuf};

use agent_client_protocol::schema::v1::{
    ContentBlock as AcpContent, ContentChunk, SessionUpdate, TextContent,
};

use crate::AgentError;
use crate::claude::frame::{ContentBlock, Frame};
use crate::claude::translate::{Step, TurnTranslator};

/// Where Claude Code keeps its state: `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub(crate) fn default_config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".claude")
}

/// The transcript's path for a session opened in `cwd`.
pub(crate) fn transcript_path(config_dir: &Path, cwd: &Path, session_id: &str) -> PathBuf {
    let encoded: String = cwd
        .to_string_lossy()
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':') {
                '-'
            } else {
                c
            }
        })
        .collect();
    config_dir
        .join("projects")
        .join(encoded)
        .join(format!("{session_id}.jsonl"))
}

/// The messages of the session's transcript as session updates: each user
/// prompt, assistant text and reasoning, and tool calls/results in persisted
/// order. Unsupported message content fails instead of producing partial history.
pub(crate) fn replay(
    config_dir: &Path,
    cwd: &Path,
    session_id: &str,
) -> Result<Vec<SessionUpdate>, AgentError> {
    if session_id.contains('/') || session_id.contains('\\') || session_id.is_empty() {
        return Err(AgentError::provider(
            "invalid Claude session transcript identity",
        ));
    }
    let path = transcript_path(config_dir, cwd, session_id);
    let text = std::fs::read_to_string(&path).map_err(|error| {
        AgentError::provider(format!(
            "cannot load Claude transcript {}: {error}",
            path.display()
        ))
    })?;
    let mut replay = Vec::new();
    for (index, line) in text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
    {
        let value: serde_json::Value = serde_json::from_str(line).map_err(|error| {
            AgentError::provider(format!(
                "malformed Claude transcript at line {}: {error}",
                index + 1
            ))
        })?;
        if !value.is_object() {
            return Err(AgentError::provider(format!(
                "malformed Claude transcript at line {}: expected an object",
                index + 1
            )));
        }
        if value
            .get("sessionId")
            .is_some_and(|id| id.as_str() != Some(session_id))
        {
            return Err(AgentError::provider(
                "Claude transcript names a different session",
            ));
        }
        replay.extend(line_updates(&value)?);
    }
    if replay.is_empty() {
        return Err(AgentError::provider(
            "Claude transcript contains no replayable conversation",
        ));
    }
    Ok(replay)
}

fn line_updates(value: &serde_json::Value) -> Result<Vec<SessionUpdate>, AgentError> {
    if value["isSidechain"] == true || value["isMeta"] == true {
        return Ok(Vec::new());
    }
    let role = value["type"].as_str();
    if !matches!(role, Some("user" | "assistant")) {
        return Ok(Vec::new());
    }
    let blocks: Vec<ContentBlock> = match &value["message"]["content"] {
        serde_json::Value::String(text) => vec![ContentBlock::Text { text: text.clone() }],
        serde_json::Value::Array(items) => {
            items.iter().map(parse_block).collect::<Result<_, _>>()?
        }
        _ => {
            return Err(AgentError::provider(
                "Claude transcript message is missing content",
            ));
        }
    };
    let mut updates = Vec::new();
    let mut translator = TurnTranslator::new();
    for block in blocks {
        match (&block, role) {
            (ContentBlock::Text { text }, Some("user")) => {
                if !text.is_empty() && !text.starts_with("[Request interrupted") {
                    updates.push(SessionUpdate::UserMessageChunk(ContentChunk::new(
                        AcpContent::Text(TextContent::new(text.clone())),
                    )));
                }
            }
            (
                ContentBlock::Text { .. }
                | ContentBlock::Thinking { .. }
                | ContentBlock::ToolUse { .. },
                Some("assistant"),
            )
            | (ContentBlock::ToolResult { .. }, Some("user")) => {
                let frame = if role == Some("assistant") {
                    Frame::Assistant(vec![block])
                } else {
                    Frame::User(vec![block])
                };
                updates.extend(translator.translate(&frame).into_iter().filter_map(
                    |step| match step {
                        Step::Update(update) => Some(*update),
                        _ => None,
                    },
                ));
            }
            _ => {
                return Err(AgentError::provider(
                    "Claude transcript has unsupported message content; refusing partial replay",
                ));
            }
        }
    }
    Ok(updates)
}

fn parse_block(value: &serde_json::Value) -> Result<ContentBlock, AgentError> {
    let nonempty = |name: &str| value[name].as_str().is_some_and(|value| !value.is_empty());
    let valid = match value["type"].as_str() {
        Some("text") => value["text"].is_string(),
        Some("thinking") => value["thinking"].is_string(),
        Some("tool_use") => nonempty("id") && nonempty("name") && value["input"].is_object(),
        Some("tool_result") => nonempty("tool_use_id") && value.get("content").is_some(),
        _ => false,
    };
    if !valid {
        return Err(AgentError::provider(
            "Claude transcript has malformed or unsupported content; refusing partial replay",
        ));
    }
    serde_json::from_value(value.clone()).map_err(|error| {
        AgentError::provider(format!("malformed Claude transcript content: {error}"))
    })
}

#[cfg(test)]
#[path = "transcript.tests.rs"]
mod tests;
