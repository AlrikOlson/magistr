//! Per-edge provenance merge of resolver-confirmed references (US3): a
//! symbolic edge the resolver confirms is relabeled `resolver-lsp`/high, a
//! new edge is appended, and an unconfirmed symbolic edge is NEVER relabeled.
//! Any resolver problem leaves the symbolic answer byte-identical.
use std::collections::BTreeMap;

use super::{
    SearchConfidence, SearchHit, SearchInput, WorkspaceQueries,
    ranking_candidate::RankingCandidate, search_scope::SearchScope, source_entry::SourceEntry,
};
use crate::{contracts::WorkspaceError, file_content::FileContent};

const PROVENANCE: &str = "resolver-lsp";

pub(super) async fn merge(
    queries: &WorkspaceQueries<'_>,
    input: &SearchInput,
    scoped: &[SourceEntry],
    candidates: &mut Vec<RankingCandidate>,
    hashes: &mut BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> Result<(), WorkspaceError> {
    let Some(resolver) = queries.resolver else {
        return Ok(());
    };
    // references() already required a warm index; the resolver's lazy boot
    // gate is therefore satisfied here.
    let edges = resolver.resolve_references(&input.query, true).await;
    if edges.is_empty() {
        return Ok(());
    }
    let mut confirmed = 0;
    let mut appended = 0;
    for edge in edges {
        if !scoped.iter().any(|entry| entry.path == edge.path) {
            continue;
        }
        let snapshot = queries.snapshot(&edge.path)?;
        let FileContent::Text(content) = snapshot.content else {
            continue;
        };
        if hashes
            .get(&edge.path)
            .is_some_and(|hash| *hash != snapshot.content_hash)
        {
            continue; // the symbolic anchors would no longer match these bytes
        }
        let line = u64::from(edge.line);
        let excerpt = content
            .split('\n')
            .nth(edge.line.saturating_sub(1) as usize)
            .unwrap_or_default()
            .trim_end()
            .to_owned();
        let candidate = match candidates.iter_mut().find(|c| c.file == edge.path) {
            Some(candidate) => candidate,
            None => {
                hashes.insert(edge.path.clone(), snapshot.content_hash.clone());
                candidates.push(RankingCandidate {
                    file: edge.path.clone(),
                    junk: scoped
                        .iter()
                        .find(|entry| entry.path == edge.path)
                        .is_some_and(|entry| entry.junk),
                    test: SearchScope::is_test(&edge.path),
                    hits: Vec::new(),
                    test_ranges: Vec::new(),
                });
                candidates.last_mut().expect("just pushed")
            }
        };
        match candidate.hits.iter_mut().find(|hit| hit.line == line) {
            Some(hit) => {
                hit.provenance = Some(PROVENANCE.into());
                hit.confidence = Some(SearchConfidence::High);
                confirmed += 1;
            }
            None => {
                let mut hit = SearchHit::new(line, edge.column, excerpt);
                hit.kind = Some("use".into());
                hit.confidence = Some(SearchConfidence::High);
                hit.provenance = Some(PROVENANCE.into());
                candidate.hits.push(hit);
                appended += 1;
            }
        }
    }
    notes.push(format!(
        "resolver plugin upgraded references: {confirmed} symbolic edge(s) confirmed, {appended} appended (provenance resolver-lsp, confidence high)"
    ));
    Ok(())
}
