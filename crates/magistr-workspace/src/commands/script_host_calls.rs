//! Service one guest host call through the same typed queries the tool
//! surface uses. A capability violation — unknown method, malformed request,
//! jail-refused path — denies the CALL, which kills the SCRIPT typed.
use serde::Serialize;
use serde_json::Value;

use super::WorkspaceCommands;
use crate::{
    contracts::WorkspaceResponse,
    query::{ReadInput, ReadRange, ReadSurround, SearchInput, SearchScope},
    response::ResponseAdapter,
};

/// `Err` is the SANDBOX_CAP_DENIED message.
pub(super) async fn service(
    commands: &WorkspaceCommands<'_>,
    method: Option<&Value>,
    args: Option<&Value>,
) -> Result<Value, String> {
    match method.and_then(Value::as_str) {
        Some("search") => search(commands, args).await,
        Some("read") => read(commands, args).await,
        Some("glob") => glob(commands, args),
        _ => Err(format!(
            "unknown host method {}",
            method.map_or("null".to_owned(), Value::to_string)
        )),
    }
}

fn envelope<R: Serialize>(response: &WorkspaceResponse<R>) -> Result<Value, String> {
    ResponseAdapter::envelope(response).map_err(|error| error.to_string())
}

async fn search(commands: &WorkspaceCommands<'_>, args: Option<&Value>) -> Result<Value, String> {
    let Some(object) = args.and_then(Value::as_object) else {
        return Err("workspace.search expects an args object".into());
    };
    if object
        .get("query")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return Err("workspace.search requires a non-empty query".into());
    }
    let input: SearchInput = serde_json::from_value(Value::Object(object.clone()))
        .map_err(|error| format!("workspace.search args are malformed: {error}"))?;
    let queries = commands.queries().map_err(|error| error.message)?;
    envelope(&queries.search(&input).await)
}

async fn read(commands: &WorkspaceCommands<'_>, args: Option<&Value>) -> Result<Value, String> {
    let Some(object) = args.and_then(Value::as_object) else {
        return Err("workspace.read expects an args object".into());
    };
    let Some(file) = object
        .get("file")
        .and_then(Value::as_str)
        .filter(|file| !file.is_empty())
    else {
        return Err("workspace.read requires a non-empty file".into());
    };
    // The jail decides every real read (§7.1); a refusal is a capability violation.
    commands
        .policy
        .check_read(file)
        .map_err(|error| format!("workspace.read({file:?}): {}", error.message))?;
    let mut input = ReadInput::new(file);
    input.symbol = object
        .get("symbol")
        .and_then(Value::as_str)
        .map(str::to_owned);
    input.lines = object.get("lines").and_then(|lines| {
        Some(ReadRange {
            start: usize::try_from(lines.get("start")?.as_u64()?).ok()?,
            end: usize::try_from(lines.get("end")?.as_u64()?).ok()?,
        })
    });
    input.surround = match object.get("surround").and_then(Value::as_str) {
        Some("none") => ReadSurround::None,
        Some("imports") => ReadSurround::Imports,
        _ => ReadSurround::Enclosing,
    };
    if let Some(budget) = object.get("budget").and_then(Value::as_u64) {
        input.budget = budget;
    }
    let queries = commands.queries().map_err(|error| error.message)?;
    envelope(&queries.read(&input).await)
}

fn glob(commands: &WorkspaceCommands<'_>, args: Option<&Value>) -> Result<Value, String> {
    let Some(pattern) = args.and_then(Value::as_str).filter(|p| !p.is_empty()) else {
        return Err("workspace.glob expects a non-empty pattern string".into());
    };
    let matcher = SearchScope::glob(pattern).map_err(|error| error.message)?;
    let queries = commands.queries().map_err(|error| error.message)?;
    let mut paths: Vec<String> = queries
        .files()
        .map_err(|error| error.message)?
        .into_iter()
        .filter(|entry| !entry.is_dir && !entry.junk && matcher.is_match(&entry.path))
        .map(|entry| entry.path)
        .collect();
    paths.sort();
    Ok(Value::Array(paths.into_iter().map(Value::String).collect()))
}
