use std::time::Duration;

use super::{
    EditInput, WorkspaceCommands, edit_resolve::text_change, resolution::Resolution,
    resolved::ResolvedWrites,
};
use crate::{
    contracts::{ErrorKind, WorkspaceError},
    file_content::FileContent,
    query::{SearchScope, worker_error},
    worker::{WorkerRequest, WorkerResponse},
};

const MAX_FILES: usize = 10_000;
const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;

/// rewrite: the structural pattern applied by the native parser worker over
/// every admitted text file in the glob/lang scope; unchanged files are
/// skipped and the note counts candidates honestly.
pub(super) async fn resolve(
    service: &WorkspaceCommands<'_>,
    input: &EditInput,
) -> Result<Resolution, WorkspaceError> {
    let pattern = input.pattern.as_deref().unwrap_or("");
    let replacement = input.rewrite_pattern.as_deref().unwrap_or("");
    let glob = match input.path.as_deref().map(SearchScope::glob).transpose() {
        Ok(glob) => glob,
        Err(error) => return Ok(Resolution::refuse(error)),
    };
    let queries = service.queries()?;
    let entries = queries.files()?;
    let mut changes = Vec::new();
    let mut considered = 0usize;
    let mut skipped_language = 0usize;
    let mut bytes = 0usize;
    for entry in entries {
        if entry.is_dir
            || entry.junk
            || glob
                .as_ref()
                .is_some_and(|glob| !glob.is_match(&entry.path))
            || input
                .lang
                .as_deref()
                .is_some_and(|lang| !matches_lang(&entry.path, lang))
        {
            continue;
        }
        considered += 1;
        if considered > MAX_FILES {
            return Ok(Resolution::refuse(WorkspaceError::new(
                ErrorKind::BudgetTooSmall,
                format!("rewrite scope exceeds {MAX_FILES} files; narrow path or lang"),
            )));
        }
        let Some(language) = crate::source_language::language(&entry.path) else {
            skipped_language += 1;
            continue;
        };
        let before = match service.reads.read(&entry.path) {
            Ok(Some(FileContent::Text(text))) => text,
            Ok(_) => continue,
            Err(error) if error.kind == ErrorKind::BudgetTooSmall => continue,
            Err(_) => continue,
        };
        bytes = bytes.saturating_add(before.len());
        if bytes > MAX_SOURCE_BYTES {
            return Ok(Resolution::refuse(WorkspaceError::new(
                ErrorKind::BudgetTooSmall,
                "rewrite scope exceeds 64 MiB of source; narrow path or lang",
            )));
        }
        let response = service
            .worker
            .call(
                WorkerRequest::Rewrite {
                    path: entry.path.clone(),
                    language: language.into(),
                    source: before.clone(),
                    pattern: pattern.into(),
                    replacement: replacement.into(),
                },
                Duration::from_secs(30),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await;
        let output = match response {
            Ok(WorkerResponse::Rewrite { output, .. }) => output,
            Ok(_) => {
                return Ok(Resolution::refuse(WorkspaceError::new(
                    ErrorKind::EngineUnavailable,
                    "unexpected rewrite response from the parser worker",
                )));
            }
            Err(error) => return Ok(Resolution::refuse(worker_error(error))),
        };
        if output == before {
            continue;
        }
        changes.push(text_change(&entry.path, &before, output));
    }
    let mut notes = vec![format!(
        "rewrite: pattern matched in {} of {considered} candidate file(s){}",
        changes.len(),
        input
            .path
            .as_deref()
            .map_or(String::new(), |path| format!(" under {path}"))
    )];
    if skipped_language > 0 {
        notes.push(format!(
            "rewrite: {skipped_language} candidate file(s) have no bundled grammar and were not rewritten"
        ));
    }
    Ok(Resolution::Writes(ResolvedWrites {
        changes,
        notes,
        always_gated: true,
    }))
}

/// Extension families for the lang filter; an unknown value is a literal extension.
fn matches_lang(path: &str, lang: &str) -> bool {
    let extension = path
        .rsplit_once('.')
        .map_or("", |(_, extension)| extension)
        .to_ascii_lowercase();
    let lang = lang.to_ascii_lowercase();
    let family: &[&str] = match lang.as_str() {
        "ts" | "typescript" => &["ts", "tsx", "mts", "cts"],
        "js" | "javascript" => &["js", "jsx", "mjs", "cjs"],
        "py" | "python" => &["py"],
        "rs" | "rust" => &["rs"],
        "go" => &["go"],
        "java" => &["java"],
        "c" => &["c", "h"],
        "cpp" => &["cpp", "cc", "cxx", "hpp", "hh"],
        "csharp" => &["cs"],
        "ruby" => &["rb"],
        "bash" => &["sh", "bash"],
        "json" => &["json"],
        _ => return extension == lang,
    };
    family.contains(&extension.as_str())
}
