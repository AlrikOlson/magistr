//! Per-archetype verify phase — run quality gates per archetype in dependency order.
//!
//! When a project has multiple archetypes with dependencies between them
//! (e.g., frontend depends on backend), gates must run in topological order:
//! dependencies first, then dependents.

use magistr_core::types::{Archetype, ArchetypeRelationship, RelationshipKind};

/// Result of running a quality gate for a single archetype.
#[derive(Debug, Clone)]
pub struct ArchetypeGateResult {
    /// The archetype ID.
    pub archetype_id: String,
    /// Whether all gates passed.
    pub passed: bool,
    /// Individual gate outcomes.
    pub gates: Vec<GateOutcome>,
}

/// Outcome of a single quality gate.
#[derive(Debug, Clone)]
pub struct GateOutcome {
    /// Gate name (check, test, lint).
    pub name: String,
    /// Whether it passed.
    pub passed: bool,
    /// Exit code from the command.
    pub exit_code: Option<i32>,
}

/// Compute the execution order for archetype gates based on dependencies.
///
/// Returns archetype IDs in topological order: dependencies first. If the
/// frontend depends on the backend, backend gates run first so that any
/// API contract violations are caught before the frontend even starts.
///
/// Falls back to insertion order when there are no relevant dependencies.
#[must_use]
pub fn gate_execution_order(
    archetypes: &[Archetype],
    relationships: &[ArchetypeRelationship],
) -> Vec<String> {
    if archetypes.is_empty() {
        return Vec::new();
    }

    // Build adjacency list: dependency → dependent.
    let ids: Vec<&str> = archetypes.iter().map(|a| a.id.as_str()).collect();
    let mut in_degree: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut adj: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();

    for id in &ids {
        in_degree.insert(id, 0);
        adj.insert(id, Vec::new());
    }

    for rel in relationships {
        if rel.kind != RelationshipKind::DependsOn {
            continue;
        }
        // `from` depends on `to` → `to` must run first.
        if in_degree.contains_key(rel.from.as_str()) && adj.contains_key(rel.to.as_str()) {
            adj.get_mut(rel.to.as_str()).unwrap().push(&rel.from);
            *in_degree.get_mut(rel.from.as_str()).unwrap() += 1;
        }
    }

    // Kahn's algorithm for topological sort.
    let mut queue: std::collections::VecDeque<&str> = in_degree
        .iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    // Sort the initial queue for deterministic ordering.
    let mut sorted_queue: Vec<&str> = queue.drain(..).collect();
    sorted_queue.sort();
    queue.extend(sorted_queue);

    let mut result = Vec::with_capacity(ids.len());
    while let Some(id) = queue.pop_front() {
        result.push(id.to_string());
        if let Some(dependents) = adj.get(id) {
            let mut next: Vec<&str> = Vec::new();
            for &dep in dependents {
                let deg = in_degree.get_mut(dep).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    next.push(dep);
                }
            }
            next.sort();
            queue.extend(next);
        }
    }

    // If there's a cycle, append any remaining IDs.
    for id in &ids {
        if !result.iter().any(|r| r == *id) {
            result.push(id.to_string());
        }
    }

    result
}

/// Aggregate per-archetype gate results into an overall pass/fail.
///
/// The overall result fails if any archetype fails.
#[must_use]
pub fn aggregate_results(results: &[ArchetypeGateResult]) -> bool {
    results.iter().all(|r| r.passed)
}

#[cfg(test)]
#[path = "archetype_verify.tests.rs"]
mod tests;
