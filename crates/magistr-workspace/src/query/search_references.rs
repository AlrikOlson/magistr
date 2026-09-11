use std::collections::{BTreeMap, BTreeSet};

use super::{
    SearchConfidence, SearchHit, SearchInput, WorkspaceQueries,
    ranking_candidate::RankingCandidate, search_scope::SearchScope, source_entry::SourceEntry,
};
use crate::{
    contracts::{ErrorKind, Freshness, WorkspaceError},
    file_content::FileContent,
};

/// The retained bounds of one references answer: definition rows, files
/// scanned for occurrences, occurrences per file and in total. Reaching a
/// bound caps the answer and says so; it never refuses.
pub(super) const MAX_REFERENCE_DEFINITIONS: usize = 200;
pub(super) const MAX_REFERENCE_FILES: usize = 200;
pub(super) const MAX_REFERENCE_EDGES_PER_FILE: usize = 50;
pub(super) const MAX_REFERENCE_EDGES_TOTAL: usize = 1000;

type ReferenceCandidates = (
    Vec<RankingCandidate>,
    BTreeMap<String, String>,
    Freshness,
    Vec<String>,
);

/// Name-resolution-free Tier-1 edges from validated definitions and live
/// bytes. Candidates come from the index — the defining files and the
/// content-index (FTS) files that mention the name — never from a scan of
/// every file in scope, so a miss costs a few index queries and a hot name
/// costs the retained file cap; each candidate is still validated against
/// its live bytes before a hit is presented.
pub(super) fn references(
    queries: &WorkspaceQueries<'_>,
    input: &SearchInput,
    scoped: &[SourceEntry],
) -> Result<ReferenceCandidates, WorkspaceError> {
    let index = queries.index.ok_or_else(unavailable)?;
    let status = index.status();
    if status.generation == 0 || status.freshness == Freshness::Cold {
        return Err(unavailable());
    }
    let symbols = index
        .symbols(&input.query, 1000)
        .map_err(|error| WorkspaceError::new(ErrorKind::IndexUnavailable, error.to_string()))?;
    if symbols.freshness == Freshness::Cold
        || symbols.truncated
        || is_index_unavailable(&symbols.notes)
    {
        return Err(WorkspaceError::new(
            ErrorKind::IndexUnavailable,
            "reference definitions are stale or exceed the candidate bound; refresh or narrow the symbol",
        ));
    }
    let scoped_paths: BTreeSet<&str> = scoped.iter().map(|entry| entry.path.as_str()).collect();
    let mut capped = Vec::new();
    let definitions = definitions_in_scope(symbols.rows, &input.query, &scoped_paths, &mut capped);
    let stems: BTreeSet<_> = definitions
        .keys()
        .filter_map(|path| {
            std::path::Path::new(path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_owned)
        })
        .take(8)
        .collect();

    let scan = candidate_files(
        index,
        &input.query,
        &definitions,
        &scoped_paths,
        &mut capped,
    )?;
    let mut freshness = observed(symbols.freshness, status.freshness);

    // Live files the index does not know cannot be candidates; say so
    // rather than letting absence read as proof.
    let live: Vec<&str> = scoped_paths.iter().copied().collect();
    let omitted = index
        .unknown_paths(&live)
        .map_err(|error| WorkspaceError::new(ErrorKind::IndexUnavailable, error.to_string()))?
        .len();

    let mut candidates = Vec::new();
    let mut hashes = BTreeMap::new();
    let mut total = 0;
    let mut bytes = 0usize;
    for path in &scan {
        if total >= MAX_REFERENCE_EDGES_TOTAL {
            capped.push(format!("{MAX_REFERENCE_EDGES_TOTAL} occurrences"));
            break;
        }
        let indexed = queries.index_file(path)?;
        if indexed.freshness == Freshness::Cold
            || indexed.truncated
            || is_index_unavailable(&indexed.notes)
        {
            return Err(WorkspaceError::new(
                ErrorKind::IndexUnavailable,
                "reference anchors changed; retry after the index refresh",
            ));
        }
        let Some(file) = indexed.rows.into_iter().next() else {
            freshness = observed(freshness, Freshness::Warming);
            continue;
        };
        let snapshot = queries.snapshot(path)?;
        freshness = observed(freshness, snapshot.freshness);
        freshness = observed(freshness, indexed.freshness);
        if file.content_hash != snapshot.content_hash {
            return Err(WorkspaceError::new(
                ErrorKind::IndexUnavailable,
                "reference source changed during validation; retry after the index refresh",
            ));
        }
        let FileContent::Text(content) = snapshot.content else {
            continue;
        };
        bytes = bytes.saturating_add(content.len());
        if bytes > 64 * 1024 * 1024 {
            return Err(WorkspaceError::new(
                ErrorKind::BudgetTooSmall,
                "reference scan exceeds 64 MiB; narrow the path",
            ));
        }
        let file_definitions = definitions.get(path).map(Vec::as_slice).unwrap_or_default();
        let linked = definitions.contains_key(path)
            || file
                .imports
                .iter()
                .any(|target| stems.iter().any(|stem| import_links(target, stem)));
        let (hits, per_file_capped) =
            reference_hits(&content, file_definitions, &input.query, linked, &mut total);
        if per_file_capped {
            capped.push(format!(
                "{MAX_REFERENCE_EDGES_PER_FILE} occurrences in {path}"
            ));
        }
        if !hits.is_empty() {
            hashes.insert(path.clone(), snapshot.content_hash);
            candidates.push(RankingCandidate {
                file: path.clone(),
                junk: scoped
                    .iter()
                    .find(|entry| &entry.path == path)
                    .is_some_and(|entry| entry.junk),
                test: SearchScope::is_test(path),
                hits,
                test_ranges: file.test_ranges,
            });
        }
    }
    let mut notes = vec!["references are tree-sitter fidelity (name-resolution-free): high confidence is a Tier-1 definition row, medium an occurrence in an import-linked file, low a bare name match that may be a false positive".into()];
    if omitted > 0 {
        index.request_refresh();
        freshness = observed(freshness, Freshness::Warming);
        notes.push(format!("{omitted} live file(s) absent from the validated index were omitted; refresh requested. Reference absence is not semantic proof."));
    }
    if !capped.is_empty() {
        capped.dedup();
        notes.push(format!(
            "references were capped at the retained bounds ({}); narrow the path or symbol for the rest",
            capped.join(", ")
        ));
    }
    Ok((candidates, hashes, freshness, notes))
}

/// Exact-name definition rows within scope, per file, within the retained
/// definition cap.
fn definitions_in_scope(
    rows: Vec<crate::index::cache::CachedSymbol>,
    name: &str,
    scoped_paths: &BTreeSet<&str>,
    capped: &mut Vec<String>,
) -> BTreeMap<String, Vec<crate::worker::WorkerSymbol>> {
    let mut definitions = BTreeMap::<String, Vec<_>>::new();
    let mut definition_rows = 0;
    for row in rows {
        if !row.symbol.name.eq_ignore_ascii_case(name)
            || !scoped_paths.contains(row.file.path.as_str())
        {
            continue;
        }
        if definition_rows == MAX_REFERENCE_DEFINITIONS {
            capped.push(format!("{MAX_REFERENCE_DEFINITIONS} definitions"));
            break;
        }
        definition_rows += 1;
        definitions
            .entry(row.file.path)
            .or_default()
            .push(row.symbol);
    }
    definitions
}

/// The files to scan: the defining files first, then the content-index
/// (FTS) files that mention the name, within the retained file cap and the
/// caller's scope. Nothing outside the index is ever a candidate, and a
/// candidate is only named here — it is verified against its live bytes
/// when it is scanned.
fn candidate_files(
    index: &crate::index::WorkspaceIndex,
    name: &str,
    definitions: &BTreeMap<String, Vec<crate::worker::WorkerSymbol>>,
    scoped_paths: &BTreeSet<&str>,
    capped: &mut Vec<String>,
) -> Result<Vec<String>, WorkspaceError> {
    let (mentions, more) = index
        .fts_paths(name, MAX_REFERENCE_FILES)
        .map_err(|error| WorkspaceError::new(ErrorKind::IndexUnavailable, error.to_string()))?;
    let mut scan: Vec<String> = definitions.keys().cloned().collect();
    for path in mentions {
        if scan.len() >= MAX_REFERENCE_FILES {
            break;
        }
        if scoped_paths.contains(path.as_str()) && !scan.contains(&path) {
            scan.push(path);
        }
    }
    if more || scan.len() >= MAX_REFERENCE_FILES {
        capped.push(format!("{MAX_REFERENCE_FILES} files"));
    }
    Ok(scan)
}

/// The worse of two freshness observations.
fn observed(current: Freshness, seen: Freshness) -> Freshness {
    match (current, seen) {
        (Freshness::Cold, _) | (_, Freshness::Cold) => Freshness::Cold,
        (Freshness::Warming, _) | (_, Freshness::Warming) => Freshness::Warming,
        _ => Freshness::Warm,
    }
}

/// Definition rows first, then identifier-bounded occurrences on every
/// other line, capped per file and in total; `true` when this file hit
/// its own cap.
fn reference_hits(
    content: &str,
    file_definitions: &[crate::worker::WorkerSymbol],
    query: &str,
    linked: bool,
    total: &mut usize,
) -> (Vec<SearchHit>, bool) {
    let mut hits = Vec::new();
    let lines: Vec<_> = content.split('\n').collect();
    for symbol in file_definitions {
        let excerpt = lines
            .get(symbol.start_line.saturating_sub(1) as usize)
            .copied()
            .unwrap_or_default()
            .trim_end();
        let mut hit = SearchHit::new(symbol.start_line.into(), symbol.start_col, excerpt.into());
        hit.kind = Some("definition".into());
        hit.confidence = Some(SearchConfidence::High);
        hit.provenance = Some("tier1-symbols".into());
        hits.push(hit);
    }
    let mut kept = 0;
    for (number, line) in lines.iter().enumerate() {
        if file_definitions
            .iter()
            .any(|symbol| symbol.start_line as usize == number + 1)
        {
            continue;
        }
        for (offset, _) in line.match_indices(query) {
            if kept >= MAX_REFERENCE_EDGES_PER_FILE || *total >= MAX_REFERENCE_EDGES_TOTAL {
                return (hits, kept >= MAX_REFERENCE_EDGES_PER_FILE);
            }
            if line[..offset]
                .chars()
                .next_back()
                .is_some_and(is_identifier)
                || line[offset + query.len()..]
                    .chars()
                    .next()
                    .is_some_and(is_identifier)
            {
                continue;
            }
            let column = line[..offset].encode_utf16().count() as u32 + 1;
            let mut hit = SearchHit::new(number as u64 + 1, column, line.trim_end().into());
            hit.kind = Some("use".into());
            hit.confidence = Some(if linked {
                SearchConfidence::Medium
            } else {
                SearchConfidence::Low
            });
            hit.provenance = Some(
                if linked {
                    "tier1-imports"
                } else {
                    "tier1-text"
                }
                .into(),
            );
            hits.push(hit);
            kept += 1;
            *total += 1;
        }
    }
    (hits, false)
}

fn unavailable() -> WorkspaceError {
    WorkspaceError::new(
        ErrorKind::IndexUnavailable,
        "search.mode \"references\" requires a warm index; retry once the Tier-1 build completes",
    )
}

// The existing index exposes a nonblocking cache miss through these notes.
// Warming alone also means a valid watcher-off query and must remain usable.
fn is_index_unavailable(notes: &[String]) -> bool {
    notes.iter().any(|note| {
        ["Index is opening", "Index is updating", "Index is stopped"]
            .iter()
            .any(|prefix| note.starts_with(prefix))
    })
}

fn is_identifier(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '$')
}

fn import_links(target: &str, stem: &str) -> bool {
    target == stem
        || target.ends_with(&format!("/{stem}"))
        || target.contains(&format!("/{stem}."))
        || target.starts_with(&format!("{stem}."))
        || target.ends_with(&format!(".{stem}"))
        || target.starts_with(&format!("{stem}::"))
        || target.ends_with(&format!("::{stem}"))
        || target.contains(&format!("::{stem}::"))
}

#[cfg(test)]
#[path = "search_references.tests.rs"]
mod tests;
