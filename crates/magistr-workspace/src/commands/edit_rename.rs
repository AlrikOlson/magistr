use std::collections::BTreeSet;

use regex::Regex;

use super::{
    EditInput, WorkspaceCommands, edit_resolve::text_change, resolution::Resolution,
    resolved::ResolvedWrites, source_reads::SourceReads,
};
use crate::{
    contracts::{ErrorKind, WorkspaceError, WorkspaceResponse},
    file_content::FileContent,
    query::{SearchConfidence, SearchInput, SearchMode},
};

/// rename: the definition file plus every file the references search links,
/// word-boundary identifier replacement per file. Name-resolution-free by
/// design; the notes carry the confidence provenance instead of overclaiming,
/// and parse verification guards every touched file in the pipeline.
pub(super) async fn resolve(
    service: &WorkspaceCommands<'_>,
    input: &EditInput,
    file: &str,
    old_name: &str,
    definition_before: &str,
) -> Result<Resolution, WorkspaceError> {
    let new_name = input.new_name.as_deref().unwrap_or("");
    if !is_identifier(new_name) {
        return Ok(Resolution::refuse(WorkspaceError::new(
            ErrorKind::PatternInvalid,
            format!("rename: newName {new_name:?} is not a valid identifier"),
        )));
    }
    let member = old_name
        .rsplit("::")
        .next()
        .unwrap_or(old_name)
        .rsplit('.')
        .next()
        .unwrap_or(old_name)
        .to_owned();
    let mut search = SearchInput::new(member.clone());
    search.mode = SearchMode::References;
    search.context = 0;
    search.budget = 1 << 40;
    let queries = service.queries()?;
    let (groups, truncated, mut notes) = match queries.search(&search).await {
        WorkspaceResponse::Success { results, notes, .. } => (
            results.rows().to_vec(),
            results.truncation().is_some(),
            notes.unwrap_or_default(),
        ),
        WorkspaceResponse::Failure { error, notes, .. } => {
            return Ok(Resolution::Refused(error, notes.unwrap_or_default()));
        }
    };
    let mut paths = BTreeSet::from([file.to_owned()]);
    let (mut high, mut medium, mut low) = (0, 0, 0);
    for group in &groups {
        paths.insert(SourceReads::normalize(&group.file));
        for hit in &group.hits {
            match hit.confidence {
                Some(SearchConfidence::High) => high += 1,
                Some(SearchConfidence::Medium) => medium += 1,
                Some(SearchConfidence::Low) => low += 1,
                None => {}
            }
        }
    }
    let pattern = Regex::new(&format!("(?-u:\\b){}(?-u:\\b)", regex::escape(&member)))
        .map_err(|error| WorkspaceError::new(ErrorKind::PatternInvalid, error.to_string()))?;
    let mut changes = Vec::new();
    for path in paths {
        let before = if path == file {
            definition_before.to_owned()
        } else {
            match service.reads.read(&path) {
                Ok(Some(FileContent::Text(text))) => text,
                // A reference file the index knew but the policy or filesystem
                // now refuses: skip it honestly rather than failing the rename.
                Ok(Some(FileContent::Binary(_))) | Ok(None) => {
                    notes.push(format!("rename: skipped {path} (FILE_NOT_FOUND)"));
                    continue;
                }
                Err(error) => {
                    notes.push(format!("rename: skipped {path} ({})", error.code()));
                    continue;
                }
            }
        };
        let after = pattern.replace_all(&before, new_name).into_owned();
        if after == before {
            continue;
        }
        changes.push(text_change(&path, &before, after));
    }
    if truncated {
        notes.push("rename: the reference scan was truncated; files beyond the scan bound were not rewritten".into());
    }
    notes.insert(
        0,
        format!(
            "rename {member:?} → {new_name:?}: {} file(s) from the definition plus index reference edges (confidence high:{high} medium:{medium} low:{low}); word-boundary matches, name-resolution-free — strings and comments included",
            changes.len()
        ),
    );
    Ok(Resolution::Writes(ResolvedWrites {
        changes,
        notes,
        always_gated: true,
    }))
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || matches!(first, '_' | '$'))
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$'))
}
