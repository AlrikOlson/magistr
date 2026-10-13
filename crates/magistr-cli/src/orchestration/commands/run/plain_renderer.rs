//! Ordered plain-text presentation of the shared session stream.

use std::collections::HashMap;
use std::io::{self, Write};

use crate::agent::message::{AgentMessage, ContentBlock, UserBody};
use serde_json::Value;

#[derive(Default)]
struct Tool {
    title: String,
    output: String,
    snapshot: String,
    observed: usize,
    terminal: bool,
}

/// One turn's presentation state; it owns no session or workflow facts.
#[derive(Default)]
pub(super) struct PlainRenderer {
    tools: HashMap<String, Tool>,
    text_open: bool,
    activity_open: bool,
    observation: usize,
}

impl PlainRenderer {
    pub fn render(
        &mut self,
        message: &AgentMessage,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> io::Result<()> {
        match message {
            AgentMessage::Assistant { message, .. } => {
                for block in &message.content {
                    self.block(block, out, err)?;
                }
            }
            AgentMessage::User { message, .. } => {
                if let Some(UserBody::Blocks(blocks)) = &message.content {
                    for block in blocks {
                        if matches!(block, ContentBlock::ToolResult { .. }) {
                            self.block(block, out, err)?;
                        }
                    }
                }
            }
            AgentMessage::StreamEvent { event, .. } if event["type"] == "tool_call_update" => {
                let id = event["tool_call_id"].as_str().unwrap_or("tool");
                if !self.tools.contains_key(id) {
                    let title = event["title"].as_str().unwrap_or("Tool").to_owned();
                    self.start(id, &title, &Value::Null, out, err)?;
                }
                if let Some(title) = event["title"].as_str() {
                    self.tools.entry(id.to_owned()).or_default().title =
                        tool_title(title, &event["raw_input"]);
                }
                let content = event["content"].as_str().unwrap_or_default();
                let text =
                    if !event["raw_output"]["structuredContent"].is_null() || content.is_empty() {
                        if event["raw_output"].is_null() {
                            String::new()
                        } else {
                            super::plain_output::display_output(&event["raw_output"])
                        }
                    } else {
                        content.to_owned()
                    };
                let status = event["status"].as_str().unwrap_or_default();
                self.output(
                    id,
                    &text,
                    matches!(status, "Completed" | "Failed"),
                    out,
                    err,
                )?;
                if matches!(status, "Completed" | "Failed") {
                    let tool = self.tools.entry(id.to_owned()).or_default();
                    if !tool.terminal {
                        tool.terminal = true;
                        let title = tool.title.clone();
                        self.line(
                            &format!(
                                "[{}] {title}",
                                if status == "Failed" { "failed" } else { "done" }
                            ),
                            out,
                            err,
                        )?;
                    }
                }
            }
            AgentMessage::Plan { entries } => {
                self.line("Plan:", out, err)?;
                for entry in entries {
                    let status = match entry.status {
                        crate::agent::message::PlanEntryStatus::Pending => "Planned",
                        crate::agent::message::PlanEntryStatus::InProgress => "In progress",
                        crate::agent::message::PlanEntryStatus::Completed => "Completed",
                    };
                    self.line(&format!("  {status}: {}", entry.content), out, err)?;
                }
            }
            AgentMessage::System { subtype, data } if subtype == "terminal_output" => {
                if let Ok(bytes) = serde_json::from_value::<Vec<u8>>(data["bytes"].clone()) {
                    self.end_text(out)?;
                    err.write_all(&bytes)?;
                    if let Some(last) = bytes.last() {
                        self.activity_open = *last != b'\n';
                    }
                }
            }
            AgentMessage::Result {
                is_error,
                num_turns,
                duration_ms,
                total_cost_usd,
                session_id,
                result,
                ..
            } => {
                self.finish(out, err)?;
                let cost = total_cost_usd
                    .map(|cost| format!("${cost:.4}"))
                    .unwrap_or_else(|| "unknown".into());
                let seconds = *duration_ms as f64 / 1000.0;
                if *is_error {
                    self.line(&format!("error: agent ended with error ({num_turns} turns, {seconds:.1}s, cost: {cost})"), out, err)?;
                    if let Some(result) = result {
                        self.line(result, out, err)?;
                    }
                } else {
                    self.line(&format!("done: {num_turns} turns, {seconds:.1}s, cost: {cost}, provider conversation: {}", session_id.as_str()), out, err)?;
                }
                self.tools.clear();
            }
            AgentMessage::Cancelled { reason, .. } => {
                self.finish(out, err)?;
                self.line(&format!("cancelled: {reason}"), out, err)?;
            }
            _ => {}
        }
        err.flush()
    }

    fn block(
        &mut self,
        block: &ContentBlock,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> io::Result<()> {
        match block {
            ContentBlock::Text { text } => {
                if self.activity_open {
                    writeln!(err)?;
                    err.flush()?;
                    self.activity_open = false;
                }
                write!(out, "{text}")?;
                self.text_open = !text.ends_with('\n') && (!text.is_empty() || self.text_open);
                out.flush()
            }
            ContentBlock::ToolUse {
                id, name, input, ..
            } => self.start(id, name, input, out, err),
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                // Stream updates already carried the same terminal output.
                if self
                    .tools
                    .get(tool_use_id)
                    .is_some_and(|tool| tool.terminal)
                {
                    return Ok(());
                }
                if let Some(content) = content {
                    let value = serde_json::from_str(content)
                        .unwrap_or_else(|_| Value::String(content.clone()));
                    self.output(
                        tool_use_id,
                        &super::plain_output::display_output(&value),
                        true,
                        out,
                        err,
                    )?;
                }
                let tool = self.tools.entry(tool_use_id.clone()).or_default();
                tool.terminal = true;
                let title = if tool.title.is_empty() {
                    "Tool".to_owned()
                } else {
                    tool.title.clone()
                };
                self.line(
                    &format!(
                        "[{}] {title}",
                        if *is_error == Some(true) {
                            "failed"
                        } else {
                            "done"
                        }
                    ),
                    out,
                    err,
                )
            }
            _ => Ok(()),
        }
    }

    fn start(
        &mut self,
        id: &str,
        name: &str,
        input: &Value,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> io::Result<()> {
        let title = tool_title(name, input);
        self.tools.insert(
            id.to_owned(),
            Tool {
                title: title.clone(),
                ..Tool::default()
            },
        );
        self.line(&format!("[tool] {title}"), out, err)?;
        if tool_name(name) == "think_record_step" {
            if let Some(outcome) = input["outcome"].as_str() {
                self.preview(outcome, out, err)?;
            }
        } else if let Some(command) = input["command"].as_str().or_else(|| input["cmd"].as_str())
            && title != command
        {
            self.preview(command, out, err)?;
        }
        Ok(())
    }

    fn output(
        &mut self,
        id: &str,
        text: &str,
        complete: bool,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> io::Result<()> {
        self.observation = self.observation.saturating_add(1);
        let tool = self.tools.entry(id.to_owned()).or_default();
        tool.observed = self.observation;
        if !text.is_empty() {
            tool.snapshot = text.to_owned();
        }
        let text = if complete {
            tool.snapshot.as_str()
        } else {
            tool.snapshot
                .rfind('\n')
                .map(|at| &tool.snapshot[..=at])
                .unwrap_or_default()
        };
        let delta = text.strip_prefix(&tool.output).unwrap_or(text).to_owned();
        tool.output = text.to_owned();
        if !delta.is_empty() {
            self.preview(&delta, out, err)?;
        }
        Ok(())
    }

    fn preview(&mut self, text: &str, out: &mut dyn Write, err: &mut dyn Write) -> io::Result<()> {
        let lines: Vec<_> = text.lines().collect();
        let omitted = lines.len().saturating_sub(12);
        let selected: Vec<_> = if omitted == 0 {
            lines
        } else {
            lines[..6]
                .iter()
                .copied()
                .chain(lines[lines.len() - 6..].iter().copied())
                .collect()
        };
        for (index, line) in selected.iter().enumerate() {
            if index == 6 && omitted > 0 {
                self.line(&format!("  … {omitted} lines omitted …"), out, err)?;
            }
            let shortened: String = line.chars().take(240).collect();
            self.line(
                &format!(
                    "  {shortened}{}",
                    if shortened.len() < line.len() {
                        " …"
                    } else {
                        ""
                    }
                ),
                out,
                err,
            )?;
        }
        Ok(())
    }

    pub fn finish(&mut self, out: &mut dyn Write, err: &mut dyn Write) -> io::Result<()> {
        let mut pending: Vec<_> = self
            .tools
            .iter()
            .filter(|(_, tool)| tool.snapshot != tool.output)
            .map(|(id, tool)| (tool.observed, id.clone(), tool.title.clone()))
            .collect();
        pending.sort_by_key(|(observed, _, _)| *observed);
        for (_, id, title) in pending {
            self.line(&format!("[output] {title}"), out, err)?;
            self.output(&id, "", true, out, err)?;
        }
        self.end_text(out)
    }

    fn end_text(&mut self, out: &mut dyn Write) -> io::Result<()> {
        if self.text_open {
            writeln!(out)?;
            out.flush()?;
            self.text_open = false;
        }
        Ok(())
    }

    fn line(&mut self, text: &str, out: &mut dyn Write, err: &mut dyn Write) -> io::Result<()> {
        self.end_text(out)?;
        if self.activity_open {
            writeln!(err)?;
            self.activity_open = false;
        }
        writeln!(err, "{text}")?;
        err.flush()
    }
}

fn tool_name(name: &str) -> &str {
    name.rsplit("__")
        .next()
        .unwrap_or(name)
        .rsplit('.')
        .next()
        .unwrap_or(name)
}

fn tool_title(name: &str, input: &Value) -> String {
    let tool = tool_name(name);
    match tool {
        "magistr_set_phase" => format!("Phase: {}", input["phase"].as_str().unwrap_or("unknown")),
        "think_record_step" => format!(
            "Finding: {}",
            input["purpose"].as_str().unwrap_or("Record finding")
        ),
        "ship_check" => format!("Check: {}", input["name"].as_str().unwrap_or("Run check")),
        "magistr_gate" => format!("Check: {}", input["gate"].as_str().unwrap_or("Run check")),
        "ship_plan" => format!("Plan: {}", input["title"].as_str().unwrap_or("Update plan")),
        "ship_start" => "Start planned work".into(),
        "ship_complete" => "Finish planned work".into(),
        "ship_finalize" => "Finish work".into(),
        "roadmap_complete_chunk" => "Complete roadmap item".into(),
        _ => {
            let detail = ["file", "path", "query"]
                .iter()
                .find_map(|key| input[key].as_str());
            let label = if let Some(name) = name.strip_prefix("mcp__") {
                name.replace("_mcp__", "__")
                    .replace("__", " · ")
                    .replace('_', " ")
            } else {
                name.to_owned()
            };
            match detail {
                Some(detail) => format!("{label}: {detail}"),
                None => label,
            }
        }
    }
}

#[cfg(test)]
#[path = "plain_renderer.tests.rs"]
mod tests;
