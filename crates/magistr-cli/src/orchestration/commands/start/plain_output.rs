//! Concise display text for reported tool results, independent of providers.

use serde_json::Value;

/// Keep useful check and workflow outcomes without dumping their full records.
pub(super) fn display_output(value: &Value) -> String {
    display(value, 0)
}

fn display(value: &Value, depth: usize) -> String {
    if depth >= 4 {
        return value.to_string();
    }
    if let Some(text) = value.as_str() {
        return match serde_json::from_str::<Value>(text) {
            Ok(parsed) if parsed.is_object() || parsed.is_array() => display(&parsed, depth + 1),
            _ => text.to_owned(),
        };
    }
    if let Some(structured) = value.get("structuredContent").filter(|v| !v.is_null()) {
        return display(structured, depth + 1);
    }
    if let Some(passed) = value["passed"].as_bool() {
        let name = value["name"].as_str().unwrap_or("check");
        let exit = value["exit_code"]
            .as_i64()
            .map(|code| format!(" (exit {code})"))
            .unwrap_or_default();
        let details = value["details"].as_str().unwrap_or_default();
        return format!(
            "Check {name}: {}{exit}\n{details}",
            if passed { "passed" } else { "failed" }
        );
    }
    if value["ok"].as_bool() == Some(false) {
        return format!(
            "Failed: {}",
            value["message"]
                .as_str()
                .or_else(|| value["error"].as_str())
                .unwrap_or("The operation failed.")
        );
    }
    if value["step_number"].is_number() && value["outcome_excerpt"].is_string() {
        return "Finding recorded.".into();
    }
    if let Some(tasks) = value["tasks"].as_array() {
        return tasks
            .iter()
            .map(|task| {
                format!(
                    "{}: {}",
                    task["status"].as_str().unwrap_or("planned"),
                    task["title"].as_str().unwrap_or("Work item")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    if let (Some(title), Some(status)) = (value["title"].as_str(), value["status"].as_str()) {
        return format!("{status}: {title}");
    }
    if let Some(summary) = value["summary"].as_str() {
        return summary.to_owned();
    }
    if let Some(text) = value["output"]
        .as_str()
        .or_else(|| value["aggregatedOutput"].as_str())
    {
        return text.to_owned();
    }
    if let Some(blocks) = value["content"]
        .as_array()
        .or_else(|| value["contentItems"].as_array())
    {
        let texts: Vec<_> = blocks
            .iter()
            .filter_map(|block| block["text"].as_str())
            .collect();
        if !texts.is_empty() {
            return texts.join("\n");
        }
    }
    if let Some(rows) = value["results"].as_array() {
        let texts: Vec<_> = rows.iter().filter_map(workspace_result).collect();
        if !texts.is_empty() {
            return texts.join("\n");
        }
    }
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn workspace_result(row: &Value) -> Option<String> {
    if let Some(text) = row["text"].as_str() {
        return Some(text.to_owned());
    }
    if let (Some(path), Some(action)) = (row["path"].as_str(), row["action"].as_str()) {
        return Some(format!("{action}: {path}"));
    }
    if let Some(hits) = row["hits"].as_array() {
        let file = row["file"].as_str().unwrap_or("File");
        return Some(
            hits.iter()
                .filter_map(|hit| {
                    hit["excerpt"]
                        .as_str()
                        .map(|text| format!("{file}:{}: {text}", hit["line"]))
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(command) = row["resolvedLine"].as_str() {
        let output: Vec<_> = ["stdoutHead", "stdoutTail", "stderrHead", "stderrTail"]
            .iter()
            .filter_map(|key| row[key].as_array())
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        return Some(format!(
            "$ {command}\nexit {}\n{}",
            row["exitCode"],
            output.join("\n")
        ));
    }
    None
}

#[cfg(test)]
#[path = "plain_output.tests.rs"]
mod tests;
