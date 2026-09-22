//! The retained per-tool argument admission: schema-level misuse is a
//! protocol-level `InvalidParams` with the reference's own message, never a
//! guessed default; everything an agent can act on comes back typed in the
//! envelope from the use case itself.

use magistr_workspace::{
    commands::{EditInput, FilesInput, ScriptInput, UndoInput},
    execution::RunInput,
    query::{MapDetail, MapInput, ReadInput, ReadRange, ReadSurround, SearchInput},
    service::JournalListInput,
};
use rmcp::{ErrorData, model::JsonObject};
use serde_json::Value;

const EDIT_OPS: [&str; 6] = [
    "replace_body",
    "insert_after",
    "insert_before",
    "rename",
    "rewrite",
    "replace_text",
];
const FILES_OPS: [&str; 5] = ["move", "copy", "create", "delete", "mkdir"];
const UNDO_STEPS_MAX: u64 = 100;
const UNDO_LIST_LIMIT_MAX: u64 = 200;

/// The two halves of the undo tool, split by `action` at the wire.
#[derive(Debug)]
pub(crate) enum UndoRequest {
    Rewind(UndoInput),
    List(JournalListInput),
}

fn invalid(message: impl Into<String>) -> ErrorData {
    ErrorData::invalid_params(message.into(), None)
}

fn non_empty_string<'a>(args: &'a JsonObject, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

fn require_string(args: &JsonObject, key: &str, tool: &str, op: &str) -> Result<(), ErrorData> {
    if non_empty_string(args, key).is_none() {
        return Err(invalid(format!("{tool}: {op} requires a non-empty {key}")));
    }
    Ok(())
}

fn require_confirm_string(args: &JsonObject, tool: &str) -> Result<(), ErrorData> {
    if args.get("confirm").is_some_and(|value| !value.is_string()) {
        return Err(invalid(format!("{tool}: confirm must be a string token")));
    }
    Ok(())
}

fn integer(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_f64()
            .filter(|number| number.fract() == 0.0 && number.abs() < 9.0e15)
            .map(|number| number as i64)
    })
}

fn decode<T: serde::de::DeserializeOwned>(tool: &str, args: &JsonObject) -> Result<T, ErrorData> {
    serde_json::from_value(Value::Object(args.clone()))
        .map_err(|error| invalid(format!("{tool}: {error}")))
}

pub(crate) fn read_input(args: &JsonObject) -> Result<ReadInput, ErrorData> {
    let Some(file) = non_empty_string(args, "file") else {
        return Err(invalid("read: file must be a non-empty string"));
    };
    if args.contains_key("symbol") && args.contains_key("lines") {
        return Err(invalid(
            "read: symbol and lines are mutually exclusive (SDD §9.3)",
        ));
    }
    let mut input = ReadInput::new(file);
    input.symbol = args
        .get("symbol")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(lines) = args.get("lines") {
        let bound = |key: &str| {
            lines
                .get(key)
                .and_then(integer)
                .filter(|value| *value >= 1)
                .map(|value| value as usize)
        };
        match (bound("start"), bound("end")) {
            (Some(start), Some(end)) => input.lines = Some(ReadRange { start, end }),
            _ => {
                return Err(invalid("read: lines must carry integer start and end >= 1"));
            }
        }
    }
    input.surround = match args.get("surround").and_then(Value::as_str) {
        None | Some("enclosing") => ReadSurround::Enclosing,
        Some("none") => ReadSurround::None,
        Some("imports") => ReadSurround::Imports,
        Some(_) => return Err(invalid("read: surround must be none, enclosing or imports")),
    };
    input.budget = budget(args, "read", 4000)?;
    Ok(input)
}

pub(crate) fn search_input(args: &JsonObject) -> Result<SearchInput, ErrorData> {
    if non_empty_string(args, "query").is_none() {
        return Err(invalid("search: query must be a non-empty string"));
    }
    decode("search", args)
}

pub(crate) fn map_input(args: &JsonObject) -> Result<MapInput, ErrorData> {
    let mut input = MapInput::default();
    if let Some(path) = args.get("path") {
        input.path = path
            .as_str()
            .ok_or_else(|| invalid("map: path must be a string"))?
            .to_owned();
    }
    if let Some(depth) = args.get("depth") {
        input.depth = integer(depth)
            .filter(|depth| (1..=16).contains(depth))
            .ok_or_else(|| invalid("map: depth must be an integer in [1, 16]"))?
            as usize;
    }
    input.detail = match args.get("detail").and_then(Value::as_str) {
        None | Some("outline") => MapDetail::Outline,
        Some("skeleton") => MapDetail::Skeleton,
        Some("full") => MapDetail::Full,
        Some(_) => return Err(invalid("map: detail must be skeleton, outline or full")),
    };
    input.budget = budget(args, "map", 6000)?;
    Ok(input)
}

pub(crate) fn edit_input(args: &JsonObject) -> Result<EditInput, ErrorData> {
    let op = args.get("op").and_then(Value::as_str).unwrap_or_default();
    if !EDIT_OPS.contains(&op) {
        return Err(invalid(format!(
            "edit: op must be one of {}",
            EDIT_OPS.join(", ")
        )));
    }
    let symbol_verb = matches!(
        op,
        "replace_body" | "insert_after" | "insert_before" | "rename"
    );
    if symbol_verb || op == "replace_text" {
        require_string(args, "file", "edit", op)?;
    }
    if symbol_verb {
        require_string(args, "symbol", "edit", op)?;
    }
    if op == "rename" {
        require_string(args, "newName", "edit", op)?;
    }
    if matches!(op, "replace_body" | "insert_after" | "insert_before")
        && !args.get("content").is_some_and(Value::is_string)
    {
        return Err(invalid(format!("edit: {op} requires content")));
    }
    if op == "rewrite" {
        require_string(args, "pattern", "edit", op)?;
        if !args.get("rewritePattern").is_some_and(Value::is_string) {
            return Err(invalid("edit: rewrite requires rewritePattern"));
        }
    }
    if op == "replace_text" {
        if !args.get("old").is_some_and(Value::is_string) {
            return Err(invalid("edit: replace_text requires old"));
        }
        if !args.get("new").is_some_and(Value::is_string) {
            return Err(invalid("edit: replace_text requires new"));
        }
    }
    require_confirm_string(args, "edit")?;
    decode("edit", args)
}

pub(crate) fn files_input(args: &JsonObject) -> Result<FilesInput, ErrorData> {
    let op = args.get("op").and_then(Value::as_str).unwrap_or_default();
    if !FILES_OPS.contains(&op) {
        return Err(invalid(format!(
            "files: op must be one of {}",
            FILES_OPS.join(", ")
        )));
    }
    if matches!(op, "move" | "copy" | "delete") {
        require_string(args, "from", "files", op)?;
    }
    if op != "delete" {
        require_string(args, "to", "files", op)?;
    }
    require_confirm_string(args, "files")?;
    decode("files", args)
}

pub(crate) fn script_input(args: &JsonObject) -> Result<ScriptInput, ErrorData> {
    if non_empty_string(args, "source").is_none() {
        return Err(invalid("script: source must be a non-empty string"));
    }
    require_confirm_string(args, "script")?;
    for key in ["fuel", "memoryCapBytes"] {
        if let Some(value) = args.get(key)
            && integer(value).is_none_or(|value| value < 1)
        {
            return Err(invalid(format!("script: {key} must be a positive integer")));
        }
    }
    decode("script", args)
}

pub(crate) fn run_input(args: &JsonObject) -> Result<RunInput, ErrorData> {
    let Some(command) = non_empty_string(args, "command") else {
        return Err(invalid("run: command must be a non-empty string"));
    };
    let mut input = RunInput::new(command);
    if let Some(extra) = args.get("args") {
        let extra = extra
            .as_array()
            .and_then(|items| {
                items
                    .iter()
                    .map(|item| item.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
            })
            .ok_or_else(|| invalid("run: args must be an array of strings"))?;
        input = input.with_args(extra);
    }
    if let Some(timeout) = args.get("timeout") {
        let timeout = integer(timeout)
            .filter(|value| (1..=RunInput::TIMEOUT_MAX_SECS as i64).contains(value))
            .ok_or_else(|| {
                invalid(format!(
                    "run: timeout must be an integer in [1, {}] seconds",
                    RunInput::TIMEOUT_MAX_SECS
                ))
            })?;
        input = input.with_timeout_secs(timeout as u64);
    }
    Ok(input.with_budget(budget(args, "run", RunInput::BUDGET_DEFAULT)?))
}

pub(crate) fn undo_request(args: &JsonObject) -> Result<UndoRequest, ErrorData> {
    let action = args.get("action").and_then(Value::as_str);
    if !matches!(action, None | Some("rewind") | Some("list")) {
        return Err(invalid("undo: action must be \"rewind\" or \"list\""));
    }
    if let Some(steps) = args.get("steps")
        && !integer(steps).is_some_and(|steps| (1..=UNDO_STEPS_MAX as i64).contains(&steps))
    {
        return Err(invalid(format!(
            "undo: steps must be an integer in [1, {UNDO_STEPS_MAX}]"
        )));
    }
    if let Some(ops) = args.get("ops") {
        let Some(ops) = ops
            .as_array()
            .filter(|ops| !ops.is_empty() && ops.len() as u64 <= UNDO_STEPS_MAX)
        else {
            return Err(invalid(format!(
                "undo: ops must be an array of 1 to {UNDO_STEPS_MAX} journal seqs"
            )));
        };
        if !ops
            .iter()
            .all(|seq| integer(seq).is_some_and(|seq| seq >= 1))
        {
            return Err(invalid("undo: ops must be positive integer journal seqs"));
        }
    }
    if let Some(limit) = args.get("limit")
        && !integer(limit).is_some_and(|limit| (1..=UNDO_LIST_LIMIT_MAX as i64).contains(&limit))
    {
        return Err(invalid(format!(
            "undo: limit must be an integer in [1, {UNDO_LIST_LIMIT_MAX}]"
        )));
    }
    if args.get("diff").is_some_and(|diff| !diff.is_boolean()) {
        return Err(invalid("undo: diff must be a boolean"));
    }
    for key in ["sinceSeq", "beforeSeq"] {
        if let Some(value) = args.get(key)
            && integer(value).is_none_or(|value| value < 0)
        {
            return Err(invalid(format!(
                "undo: {key} must be a non-negative integer journal seq"
            )));
        }
    }
    if !matches!(
        args.get("scope").and_then(Value::as_str),
        None | Some("session") | Some("workspace")
    ) || args.get("scope").is_some_and(|scope| !scope.is_string())
    {
        return Err(invalid("undo: scope must be \"session\" or \"workspace\""));
    }
    if !matches!(
        args.get("onConflict").and_then(Value::as_str),
        None | Some("surface") | Some("force") | Some("keepCurrent")
    ) || args
        .get("onConflict")
        .is_some_and(|policy| !policy.is_string())
    {
        return Err(invalid(
            "undo: onConflict must be \"surface\", \"force\" or \"keepCurrent\"",
        ));
    }
    if action == Some("list") {
        let mut listing = args.clone();
        listing.remove("action");
        return decode("undo", &listing).map(UndoRequest::List);
    }
    let mut rewind = args.clone();
    for key in ["action", "limit", "diff", "sinceSeq", "beforeSeq"] {
        rewind.remove(key);
    }
    decode("undo", &rewind).map(UndoRequest::Rewind)
}

/// The response budget: a positive integer, else the tool's retained default.
fn budget(args: &JsonObject, tool: &str, default: u64) -> Result<u64, ErrorData> {
    match args.get("budget") {
        None => Ok(default),
        Some(value) => integer(value)
            .filter(|budget| *budget >= 1)
            .map(|budget| budget as u64)
            .ok_or_else(|| invalid(format!("{tool}: budget must be a positive integer"))),
    }
}

#[cfg(test)]
#[path = "arguments.tests.rs"]
mod tests;
