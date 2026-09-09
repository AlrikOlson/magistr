//! The persisted session transcript, replayed on `session/load`.
//!
//! `--resume` restores the conversation inside the CLI but replays nothing on
//! the wire, so the history a client expects on `session/load` is read from
//! where Claude Code persists it: `<config dir>/projects/<cwd with every `/`
//! as `-`>/<session id>.jsonl`, one JSON object per line, of which the
//! `user` and `assistant` lines carry the messages. A transcript that is
//! missing or unreadable replays nothing — the resume itself still works.

use std::path::{Path, PathBuf};

use agent_client_protocol::schema::v1::{
    ContentBlock as AcpContent, ContentChunk, SessionUpdate, TextContent,
};

use crate::claude::frame::ContentBlock;

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
        .map(|c| if c == '/' || c == '\\' { '-' } else { c })
        .collect();
    config_dir
        .join("projects")
        .join(encoded)
        .join(format!("{session_id}.jsonl"))
}

/// The messages of the session's transcript as session updates: each user
/// prompt as a `user_message_chunk`, each assistant text as an
/// `agent_message_chunk`. Tool traffic, thinking and the CLI's own notes are
/// not replayed.
pub(crate) fn replay(config_dir: &Path, cwd: &Path, session_id: &str) -> Vec<SessionUpdate> {
    let path = transcript_path(config_dir, cwd, session_id);
    let Ok(text) = std::fs::read_to_string(&path) else {
        tracing::debug!(path = %path.display(), "no transcript to replay");
        return Vec::new();
    };
    text.lines().filter_map(line_update).collect()
}

fn line_update(line: &str) -> Option<SessionUpdate> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value["isSidechain"] == true || value["isMeta"] == true {
        return None;
    }
    let blocks: Vec<ContentBlock> = match &value["message"]["content"] {
        serde_json::Value::String(text) => vec![ContentBlock::Text { text: text.clone() }],
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| serde_json::from_value(item.clone()).ok())
            .collect(),
        _ => return None,
    };
    let text: String = blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() || text.starts_with("[Request interrupted") {
        return None;
    }
    let chunk = ContentChunk::new(AcpContent::Text(TextContent::new(text)));
    match value["type"].as_str() {
        Some("user") => Some(SessionUpdate::UserMessageChunk(chunk)),
        Some("assistant") => Some(SessionUpdate::AgentMessageChunk(chunk)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "transcript.tests.rs"]
mod tests;
