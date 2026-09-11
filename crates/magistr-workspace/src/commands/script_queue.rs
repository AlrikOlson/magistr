//! Resolve the final round's queue to exact transaction anchors, in guest
//! call order (edits first, then file ops). Reads go through the same
//! admitted reads every verb uses; a refusal is a capability violation. An
//! `old` that no longer (or never) matched answers STALE_ANCHOR with the
//! fresh hash — the identical contract replace_text pins.
use serde_json::Value;

use super::{
    WorkspaceCommands, resolution::Resolution, resolved::ResolvedWrites, script_guest::GuestDone,
    source_reads::SourceReads,
};
use crate::{
    contracts::{ErrorKind, WorkspaceError},
    file_content::FileContent,
    transaction::{ExpectedState, TransactionChange},
};

/// Loaded contents by path, in first-touch order; `None` means absent.
struct Contents {
    entries: Vec<(String, Option<String>)>,
    before: Vec<(String, Option<String>)>,
}

impl Contents {
    fn get(&self, path: &str) -> Option<&Option<String>> {
        self.entries
            .iter()
            .find(|(entry, _)| entry == path)
            .map(|(_, content)| content)
    }

    fn set(&mut self, path: &str, content: Option<String>) {
        match self.entries.iter_mut().find(|(entry, _)| entry == path) {
            Some((_, slot)) => *slot = content,
            None => self.entries.push((path.to_owned(), content)),
        }
    }

    /// Load once through the admitted reader; policy refusals are denials.
    fn load(
        &mut self,
        commands: &WorkspaceCommands<'_>,
        path: &str,
    ) -> Result<Option<String>, Resolution> {
        if let Some(loaded) = self.get(path) {
            return Ok(loaded.clone());
        }
        // Every queued op is a write: the write jail (protected metadata and
        // configuration included) decides before any byte is read.
        if let Err(error) = commands.policy.check_write(path) {
            return Err(cap_denied(format!("{path}: {}", error.message)));
        }
        let text = match commands.reads.read(path) {
            Ok(Some(FileContent::Text(text))) => Some(text),
            Ok(Some(FileContent::Binary(_))) => {
                return Err(Resolution::refuse(WorkspaceError::new(
                    ErrorKind::PatternInvalid,
                    format!("{path:?} contains binary bytes; script writes are text-only"),
                )));
            }
            Ok(None) => None,
            Err(error) => return Err(cap_denied(format!("{path}: {}", error.message))),
        };
        self.entries.push((path.to_owned(), text.clone()));
        self.before.push((path.to_owned(), text.clone()));
        Ok(text)
    }
}

fn cap_denied(message: String) -> Resolution {
    Resolution::refuse(WorkspaceError::new(ErrorKind::SandboxCapDenied, message))
}

fn field<'a>(op: &'a Value, key: &str) -> Option<&'a str> {
    op.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

pub(super) fn resolve(
    commands: &WorkspaceCommands<'_>,
    done: &GuestDone,
) -> Result<Resolution, WorkspaceError> {
    let mut contents = Contents {
        entries: Vec::new(),
        before: Vec::new(),
    };
    let mut notes = Vec::new();
    for (i, op) in done.edits.iter().enumerate() {
        if let Err(refusal) = resolve_edit(commands, &mut contents, i, op) {
            return Ok(refusal);
        }
    }
    for (i, op) in done.files.iter().enumerate() {
        if let Err(refusal) = resolve_file_op(commands, &mut contents, i, op, &mut notes) {
            return Ok(refusal);
        }
    }
    let changes: Vec<TransactionChange> = contents
        .entries
        .iter()
        .filter_map(|(path, content)| {
            let before = contents
                .before
                .iter()
                .find(|(entry, _)| entry == path)
                .map(|(_, before)| before.clone())
                .unwrap_or(None);
            if *content == before {
                return None; // an edit chain that round-tripped
            }
            if content.is_none() && before.is_none() {
                return None;
            }
            Some(TransactionChange {
                path: path.clone(),
                expected: before.as_ref().map_or(ExpectedState::Missing, |text| {
                    ExpectedState::Hash(FileContent::Text(text.clone()).content_hash())
                }),
                content: content.clone().map(FileContent::Text),
                baseline: None,
            })
        })
        .collect();
    if changes.is_empty() {
        return Ok(Resolution::NoOp(notes));
    }
    Ok(Resolution::Writes(ResolvedWrites {
        changes,
        notes,
        always_gated: true,
    }))
}

fn resolve_edit(
    commands: &WorkspaceCommands<'_>,
    contents: &mut Contents,
    i: usize,
    op: &Value,
) -> Result<(), Resolution> {
    let (Some(file), Some(old), Some(new)) = (
        field(op, "file"),
        field(op, "old"),
        op.get("new").and_then(Value::as_str),
    ) else {
        return Err(cap_denied(format!(
            "edit.queue #{i}: requires non-empty file, non-empty old, and new"
        )));
    };
    let path = SourceReads::normalize(file);
    let Some(text) = contents.load(commands, &path)? else {
        return Err(Resolution::refuse(WorkspaceError::new(
            ErrorKind::FileNotFound,
            format!("edit.queue #{i}: {path} does not exist"),
        )));
    };
    let occurrence = match op.get("occurrence") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_u64().filter(|n| *n >= 1) {
            Some(n) => Some(n as usize),
            None => {
                return Err(cap_denied(format!(
                    "edit.queue #{i}: occurrence must be a positive integer"
                )));
            }
        },
    };
    let stale = |message: String| {
        Resolution::refuse(WorkspaceError::new(
            ErrorKind::StaleAnchor {
                content_hash: FileContent::Text(text.clone()).content_hash(),
            },
            message,
        ))
    };
    let next = match occurrence {
        None => {
            if !text.contains(old) {
                return Err(stale(format!(
                    "edit.queue #{i}: old text not found in {path}"
                )));
            }
            text.replace(old, new)
        }
        Some(occurrence) => {
            let Some((at, _)) = text.match_indices(old).nth(occurrence - 1) else {
                return Err(stale(format!(
                    "edit.queue #{i}: occurrence {occurrence} of old text not found in {path}"
                )));
            };
            format!("{}{new}{}", &text[..at], &text[at + old.len()..])
        }
    };
    contents.set(&path, Some(next));
    Ok(())
}

fn resolve_file_op(
    commands: &WorkspaceCommands<'_>,
    contents: &mut Contents,
    i: usize,
    op: &Value,
    notes: &mut Vec<String>,
) -> Result<(), Resolution> {
    match op.get("op").and_then(Value::as_str) {
        Some(kind @ ("create" | "mkdir")) => {
            let Some(to) = field(op, "to") else {
                return Err(cap_denied(format!(
                    "files.queue #{i}: {kind} requires a non-empty to"
                )));
            };
            let path = if kind == "mkdir" {
                format!("{}/.gitkeep", SourceReads::normalize(to))
            } else {
                SourceReads::normalize(to)
            };
            if contents.load(commands, &path)?.is_some() {
                notes.push(format!(
                    "files.queue #{i}: {path} already exists; create never overwrites (§9.5)"
                ));
                return Ok(());
            }
            contents.set(&path, Some(String::new()));
            Ok(())
        }
        Some("delete") => {
            let Some(from) = field(op, "from") else {
                return Err(cap_denied(format!(
                    "files.queue #{i}: delete requires a non-empty from"
                )));
            };
            let path = SourceReads::normalize(from);
            if contents.load(commands, &path)?.is_none() {
                return Err(Resolution::refuse(WorkspaceError::new(
                    ErrorKind::FileNotFound,
                    format!("files.queue #{i}: {path} does not exist"),
                )));
            }
            contents.set(&path, None);
            Ok(())
        }
        _ => Err(cap_denied(format!(
            "files.queue #{i}: op must be create, mkdir or delete (move/copy are not queueable from script; use the files tool)"
        ))),
    }
}
