use std::collections::{BTreeMap, BTreeSet};

use super::{
    SearchConfidence, SearchHit, SearchInput, WorkspaceQueries,
    ranking_candidate::RankingCandidate, search_scope::SearchScope, source_entry::SourceEntry,
};
use crate::{
    contracts::{ErrorKind, Freshness, WorkspaceError},
    file_content::FileContent,
};

type ReferenceCandidates = (
    Vec<RankingCandidate>,
    BTreeMap<String, String>,
    Freshness,
    Vec<String>,
);

/// Name-resolution-free Tier-1 edges from validated definitions and live bytes.
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
    let mut definitions = BTreeMap::<String, Vec<_>>::new();
    for row in symbols.rows {
        if row.symbol.name.eq_ignore_ascii_case(&input.query) {
            definitions
                .entry(row.file.path)
                .or_default()
                .push(row.symbol);
        }
    }
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
    let mut candidates = Vec::new();
    let mut freshness = symbols.freshness;
    let mut hashes = BTreeMap::new();
    let mut total = 0;
    let mut bytes = 0usize;
    let mut omitted = 0usize;
    for entry in scoped {
        let indexed = queries.index_file(&entry.path)?;
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
            omitted += 1;
            if freshness != Freshness::Cold {
                freshness = Freshness::Warming;
            }
            continue;
        };
        let snapshot = queries.snapshot(&entry.path)?;
        if snapshot.freshness == Freshness::Cold {
            freshness = Freshness::Cold;
        } else if freshness == Freshness::Warm && indexed.freshness == Freshness::Warming {
            freshness = Freshness::Warming;
        }
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
        let file_definitions = definitions
            .get(&entry.path)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let linked = definitions.contains_key(&entry.path)
            || file
                .imports
                .iter()
                .any(|target| stems.iter().any(|stem| import_links(target, stem)));
        let hits = reference_hits(&content, file_definitions, &input.query, linked, &mut total)?;
        if !hits.is_empty() {
            hashes.insert(entry.path.clone(), snapshot.content_hash);
            candidates.push(RankingCandidate {
                file: entry.path.clone(),
                junk: entry.junk,
                test: SearchScope::is_test(&entry.path),
                hits,
                test_ranges: file.test_ranges,
            });
        }
    }
    let mut notes = vec!["references are tree-sitter fidelity (name-resolution-free): high confidence is a Tier-1 definition row, medium an occurrence in an import-linked file, low a bare name match that may be a false positive".into()];
    if omitted > 0 {
        index.request_refresh();
        notes.push(format!("{omitted} live file(s) absent from the validated index were omitted; refresh requested. Reference absence is not semantic proof."));
    }
    Ok((candidates, hashes, freshness, notes))
}

fn reference_hits(
    content: &str,
    file_definitions: &[crate::worker::WorkerSymbol],
    query: &str,
    linked: bool,
    total: &mut usize,
) -> Result<Vec<SearchHit>, WorkspaceError> {
    if file_definitions.len() > 200 || total.saturating_add(file_definitions.len()) > 20_000 {
        return Err(candidate_limit());
    }
    *total += file_definitions.len();
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
    for (number, line) in lines.iter().enumerate() {
        if file_definitions
            .iter()
            .any(|symbol| symbol.start_line as usize == number + 1)
        {
            continue;
        }
        for (offset, _) in line.match_indices(query) {
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
            *total += 1;
            if hits.len() > 200 || *total > 20_000 {
                return Err(candidate_limit());
            }
        }
    }
    Ok(hits)
}

fn candidate_limit() -> WorkspaceError {
    WorkspaceError::new(
        ErrorKind::BudgetTooSmall,
        "reference candidates exceed the bounded scan; narrow the path or symbol",
    )
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
