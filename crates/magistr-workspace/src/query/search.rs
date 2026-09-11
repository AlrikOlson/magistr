use std::collections::BTreeMap;

use super::{
    SearchGroup, SearchHit, SearchInput, SearchMode, WorkspaceQueries, budget, ranking::Ranking,
    ranking_candidate::RankingCandidate, search_references::references, search_scope::SearchScope,
    source_entry::SourceEntry,
};
use crate::{
    contracts::{
        Budget, EnvelopeVersion, ErrorKind, Freshness, ResultSet, TruncationReason, WorkspaceError,
        WorkspaceResponse,
    },
    file_content::FileContent,
    worker::{WorkerRequest, WorkerResponse},
};

/// Search admitted live files and return ranked, budgeted native groups.
pub(super) async fn run(
    queries: &WorkspaceQueries<'_>,
    input: &SearchInput,
) -> WorkspaceResponse<SearchGroup> {
    match execute(queries, input).await {
        Ok(response) => response,
        Err(error)
            if error.kind == ErrorKind::IndexUnavailable
                && input.mode == SearchMode::References =>
        {
            let progress = queries.index.map_or_else(
                || "no index exists yet for this workspace".into(),
                |index| {
                    let status = index.status();
                    format!(
                        "index progress: {}/{} parsed files, building={}; {}",
                        status.parsed,
                        status.files,
                        status.building,
                        status.notes.join(" ")
                    )
                },
            );
            WorkspaceResponse::Failure {
                version: EnvelopeVersion::LATEST,
                error,
                notes: Some(vec![progress]),
            }
        }
        Err(error) => budget::failure(error),
    }
}

async fn execute(
    queries: &WorkspaceQueries<'_>,
    input: &SearchInput,
) -> Result<WorkspaceResponse<SearchGroup>, WorkspaceError> {
    validate(input)?;
    let scope = SearchScope::new(input)?;
    let scoped: Vec<_> = queries
        .files()?
        .into_iter()
        .filter(|file| scope.allows(file))
        .collect();
    if scoped.len() > 10_000 {
        return Err(WorkspaceError::new(
            ErrorKind::BudgetTooSmall,
            "search scope exceeds 10000 files; narrow the path",
        ));
    }
    let mut notes = Vec::new();
    if scoped.is_empty()
        && let Some(path) = &input.path
    {
        notes.push(format!(
            "path scope {path:?} matched no files; nothing was searched"
        ));
    }
    let mut freshness = queries.freshness();
    let (candidates, hashes) = match input.mode {
        SearchMode::Files => (file_candidates(scoped, &input.query)?, BTreeMap::new()),
        SearchMode::References => {
            let (candidates, hashes, reference_freshness, reference_notes) =
                references(queries, input, &scoped)?;
            freshness = observed(freshness, reference_freshness);
            notes.extend(reference_notes);
            (candidates, hashes)
        }
        _ => {
            if input.mode == SearchMode::Auto {
                notes.push("mode:auto ran lexical; use mode:structural for AST patterns or mode:references for symbol edges".into());
            }
            let (candidates, hashes, observed_freshness) =
                engine_candidates(queries, input, scoped).await?;
            freshness = observed_freshness;
            (candidates, hashes)
        }
    };
    let mut groups = Ranking::rank(candidates, &input.query);
    let mut used = 0;
    let mut keep = 0;
    for group in &groups {
        let cost = budget::tokens(group)?;
        if used + cost > input.budget {
            break;
        }
        used += cost;
        keep += 1;
    }
    if !groups.is_empty() && keep == 0 {
        return Err(WorkspaceError::new(
            ErrorKind::BudgetTooSmall,
            format!(
                "budget {} cannot fit even one result group; raise the budget or narrow the query",
                input.budget
            ),
        ));
    }
    // The pinned contract measures ranked groups before context is attached.
    // Revalidate only selected files, and attach context from those same bytes.
    if input.mode != SearchMode::Files {
        for group in groups.iter_mut().take(keep) {
            let snapshot = queries.snapshot(&group.file)?;
            freshness = observed(freshness, snapshot.freshness);
            if hashes.get(&group.file) != Some(&snapshot.content_hash) {
                return Err(WorkspaceError::new(
                    ErrorKind::StaleAnchor {
                        content_hash: snapshot.content_hash,
                    },
                    "search source changed before its anchors could be returned; retry",
                ));
            }
            if input.context > 0
                && let FileContent::Text(content) = snapshot.content
            {
                attach_context(group, &content, input.context);
            }
        }
    }
    let cursor = groups
        .get(keep)
        .map(|group| format!("file:{}", group.file))
        .unwrap_or_else(|| "-".into());
    let results = ResultSet::truncate(
        groups,
        keep,
        TruncationReason::Budget,
        vec![
            "scope with path (e.g. src/**)".into(),
            "raise budget".into(),
            "make the query more specific".into(),
        ],
        cursor,
    )?;
    Ok(WorkspaceResponse::Success {
        version: EnvelopeVersion::LATEST,
        freshness,
        budget: Budget {
            requested: input.budget,
            used,
        },
        results,
        notes: (!notes.is_empty()).then_some(notes),
    })
}

fn validate(input: &SearchInput) -> Result<(), WorkspaceError> {
    if input.mode == SearchMode::Semantic {
        return Err(WorkspaceError::new(
            ErrorKind::UnsupportedVersion,
            "search.mode \"semantic\" is reserved and rejected in v1 (SDD §1.1); use lexical, files, structural or references",
        ));
    }
    if input.query.is_empty() || input.query.len() > 8192 || input.query.contains('\0') {
        return Err(WorkspaceError::new(
            ErrorKind::PatternInvalid,
            "search query must contain 1..8192 bytes and no NUL",
        ));
    }
    if input.context > 20 {
        return Err(WorkspaceError::new(
            ErrorKind::PatternInvalid,
            "search context must be between 0 and 20",
        ));
    }
    if input.budget == 0 {
        return Err(WorkspaceError::new(
            ErrorKind::BudgetTooSmall,
            "search budget must be positive",
        ));
    }
    Ok(())
}

fn file_candidates(
    scoped: Vec<SourceEntry>,
    query: &str,
) -> Result<Vec<RankingCandidate>, WorkspaceError> {
    let glob = SearchScope::is_glob(query)
        .then(|| SearchScope::glob(query))
        .transpose()?;
    let needle = query.to_lowercase();
    Ok(scoped
        .into_iter()
        .filter(|entry| {
            glob.as_ref().map_or_else(
                || entry.path.to_lowercase().contains(&needle),
                |glob| glob.is_match(&entry.path),
            )
        })
        .map(|entry| RankingCandidate {
            test: SearchScope::is_test(&entry.path),
            file: entry.path,
            junk: entry.junk,
            hits: Vec::new(),
            test_ranges: Vec::new(),
        })
        .collect())
}

type EngineCandidates = (Vec<RankingCandidate>, BTreeMap<String, String>, Freshness);

async fn engine_candidates(
    queries: &WorkspaceQueries<'_>,
    input: &SearchInput,
    scoped: Vec<SourceEntry>,
) -> Result<EngineCandidates, WorkspaceError> {
    let mut candidates = Vec::new();
    let mut hashes = BTreeMap::new();
    let mut bytes = 0usize;
    let mut total = 0usize;
    let mut freshness = queries.freshness();
    for entry in scoped {
        let structural = input.mode == SearchMode::Structural;
        if structural && entry.language.is_none() {
            continue;
        }
        let snapshot = queries.snapshot(&entry.path)?;
        freshness = observed(freshness, snapshot.freshness);
        let FileContent::Text(source) = snapshot.content else {
            continue;
        };
        bytes = bytes.saturating_add(source.len());
        if bytes > 64 * 1024 * 1024 {
            return Err(WorkspaceError::new(
                ErrorKind::BudgetTooSmall,
                "search scan exceeds 64 MiB; narrow the path",
            ));
        }
        let request = if structural {
            WorkerRequest::Structural {
                path: entry.path.clone(),
                language: entry.language.unwrap_or_default(),
                source,
                pattern: input.query.clone(),
            }
        } else {
            WorkerRequest::Search {
                path: entry.path.clone(),
                pattern: input.query.clone(),
                haystack: source,
                case_insensitive: false,
            }
        };
        let mut hits = match queries.call(request).await? {
            WorkerResponse::Search { hits } => hits
                .into_iter()
                .map(|hit| SearchHit::new(hit.line, hit.start + 1, hit.line_text))
                .collect::<Vec<_>>(),
            WorkerResponse::Structural { hits } => hits
                .into_iter()
                .filter(|hit| {
                    input
                        .kind
                        .as_ref()
                        .is_none_or(|kinds| kinds.iter().any(|kind| hit.kind.contains(kind)))
                })
                .map(|hit| {
                    let mut result = SearchHit::new(hit.line, hit.column, hit.line_text);
                    result.kind = Some(hit.kind);
                    result
                })
                .collect(),
            _ => {
                return Err(WorkspaceError::new(
                    ErrorKind::EngineUnavailable,
                    "worker returned a mismatched search response",
                ));
            }
        };
        if hits.is_empty() {
            continue;
        }
        total = total.saturating_add(hits.len());
        if total > 20_000 {
            return Err(WorkspaceError::new(
                ErrorKind::BudgetTooSmall,
                "search exceeds 20000 matched lines; narrow the path or query",
            ));
        }
        let mut test_ranges = Vec::new();
        if let Ok(indexed) = queries.index_file(&entry.path) {
            freshness = observed(freshness, indexed.freshness);
            if let Some(file) = indexed
                .rows
                .into_iter()
                .next()
                .filter(|file| file.content_hash == snapshot.content_hash)
            {
                test_ranges = file.test_ranges;
                if !structural && is_symbolish(&input.query) {
                    for hit in &mut hits {
                        hit.kind = file
                            .symbols
                            .iter()
                            .find(|symbol| {
                                symbol.name.eq_ignore_ascii_case(&input.query)
                                    && symbol.start_line as u64 == hit.line
                                    && input
                                        .kind
                                        .as_ref()
                                        .is_none_or(|kinds| kinds.contains(&symbol.kind))
                            })
                            .map(|symbol| symbol.kind.clone());
                    }
                }
            }
        }
        hashes.insert(entry.path.clone(), snapshot.content_hash);
        candidates.push(RankingCandidate {
            test: SearchScope::is_test(&entry.path),
            file: entry.path,
            junk: entry.junk,
            hits,
            test_ranges,
        });
    }
    Ok((candidates, hashes, freshness))
}

fn is_symbolish(query: &str) -> bool {
    query.len() <= 128
        && query.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '$' | '.' | ':' | '-')
        })
}

fn observed(left: Freshness, right: Freshness) -> Freshness {
    match (left, right) {
        (Freshness::Cold, _) | (_, Freshness::Cold) => Freshness::Cold,
        (Freshness::Warming, _) | (_, Freshness::Warming) => Freshness::Warming,
        _ => Freshness::Warm,
    }
}

fn attach_context(group: &mut SearchGroup, content: &str, context: usize) {
    let lines: Vec<_> = content.split('\n').collect();
    for hit in &mut group.hits {
        let line = hit.line.saturating_sub(1) as usize;
        hit.context_before = Some(
            lines[line.saturating_sub(context).min(lines.len())..line.min(lines.len())]
                .iter()
                .map(|line| (*line).into())
                .collect(),
        );
        hit.context_after = Some(
            lines[(line + 1).min(lines.len())..(line + 1 + context).min(lines.len())]
                .iter()
                .map(|line| (*line).into())
                .collect(),
        );
    }
}

#[cfg(test)]
#[path = "search.tests.rs"]
mod tests;
