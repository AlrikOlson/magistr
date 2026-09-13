//! `fs/read_text_file`. There is no direct `fs/write_text_file`: a session's
//! source writes take the host's [`SourceRoute`](magistr_application::effects::SourceRoute)
//! through the local effect port, or are refused.

use agent_client_protocol::Error;
use agent_client_protocol::schema::v1::{ReadTextFileRequest, ReadTextFileResponse};
use tokio::sync::mpsc;

use crate::{AcpEvent, Result};

/// JSON-RPC internal error code, used for I/O failures.
const INTERNAL_ERROR: i32 = -32603;

pub(crate) async fn read_text_file(
    args: ReadTextFileRequest,
    event_tx: &mpsc::UnboundedSender<Result<AcpEvent>>,
) -> std::result::Result<ReadTextFileResponse, Error> {
    let path = &args.path;
    let _ = event_tx.send(Ok(AcpEvent::System {
        subtype: "fs_read".to_string(),
        data: serde_json::json!({ "path": path.display().to_string() }),
    }));

    let content = tokio::fs::read_to_string(path).await.map_err(|e| {
        Error::new(
            INTERNAL_ERROR,
            format!("Failed to read {}: {e}", path.display()),
        )
    })?;
    Ok(ReadTextFileResponse::new(apply_line_params(
        &content, args.line, args.limit,
    )))
}

/// Apply the request's `line` (1-based start) and `limit` (max lines).
pub(crate) fn apply_line_params(content: &str, line: Option<u32>, limit: Option<u32>) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = line.map(|l| l.saturating_sub(1) as usize).unwrap_or(0);
    if start >= lines.len() {
        return String::new();
    }
    let end = match limit {
        Some(lim) => (start + lim as usize).min(lines.len()),
        None => lines.len(),
    };
    lines[start..end].join("\n")
}

#[cfg(test)]
#[path = "fs.tests.rs"]
mod tests;
