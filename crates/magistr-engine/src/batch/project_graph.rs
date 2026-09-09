use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::error::EngineError;

use super::project_dep::ProjectDep;

/// The projects a batch run covers and the edges between them.
///
/// The graph is built once and refuses two things at construction: a
/// duplicate project id, and — through [`order`] — a dependency cycle.
/// Refusing early is what lets [`BatchRun`] hold an order rather than
/// re-deriving one every time it is asked, and it means a batch that
/// cannot be sequenced is never a live batch that must be cancelled.
///
/// Two rules the ordering keeps.
///
/// *An edge to a project the batch does not carry is dropped, not
/// waited on.* A batch is a selection; a dependency outside it is one
/// the operator left out. Waiting on it would deadlock the run against
/// something nothing in the batch can ever satisfy.
///
/// *The order is deterministic.* The frontier is a [`BTreeSet`], so
/// among the projects that are ready at the same moment the lower id
/// comes first — the same tie-break [`WorkQueue::place`] uses, and for
/// the same reason: an answer that depends on hash iteration order is an
/// answer that cannot be tested.
///
/// [`order`]: ProjectGraph::order
/// [`BatchRun`]: super::batch_run::BatchRun
/// [`WorkQueue::place`]: crate::fleet::WorkQueue::place
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectGraph {
    projects: BTreeMap<String, ProjectDep>,
}

impl ProjectGraph {
    /// Build a graph from the declared projects, refusing a duplicate id.
    pub fn new(projects: impl IntoIterator<Item = ProjectDep>) -> Result<Self, EngineError> {
        let mut map: BTreeMap<String, ProjectDep> = BTreeMap::new();
        for project in projects {
            if map.contains_key(&project.id) {
                return Err(EngineError::Precondition(format!(
                    "project '{}' is declared twice in this batch",
                    project.id
                )));
            }
            map.insert(project.id.clone(), project);
        }
        Ok(Self { projects: map })
    }

    /// Look up one project's declaration.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ProjectDep> {
        self.projects.get(id)
    }

    /// Every project id, in id order.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.projects.keys().map(String::as_str).collect()
    }

    /// How many projects the batch covers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.projects.len()
    }

    /// Whether the batch covers nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.projects.is_empty()
    }

    /// The declared dependencies of `id` that this batch actually
    /// carries — the edges the ordering is built from.
    #[must_use]
    pub fn deps_of(&self, id: &str) -> Vec<&str> {
        self.projects
            .get(id)
            .map(|project| {
                project
                    .deps
                    .iter()
                    .filter(|dep| self.projects.contains_key(*dep))
                    .map(String::as_str)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every project that depends on `id`, directly or through another
    /// project — the set a failure of `id` takes down with it.
    ///
    /// `id` itself is never in the answer, even when the graph contains
    /// a cycle through it, because the caller is always already holding
    /// `id` and would otherwise mark it twice.
    #[must_use]
    pub fn dependents_of(&self, id: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut queue = VecDeque::from([id.to_owned()]);

        while let Some(current) = queue.pop_front() {
            for (candidate, project) in &self.projects {
                if candidate == id || found.contains(candidate) {
                    continue;
                }
                if project.deps.contains(&current) {
                    found.insert(candidate.clone());
                    queue.push_back(candidate.clone());
                }
            }
        }
        found
    }

    /// The projects in an order that satisfies every edge, or a refusal
    /// naming the projects a cycle holds.
    ///
    /// Kahn's algorithm: repeatedly take the ready projects — those
    /// whose in-batch dependencies are all placed — lowest id first.
    /// Whatever is left when nothing is ready is exactly the set no
    /// order can satisfy, which makes the refusal name the cycle rather
    /// than merely report that there is one.
    pub fn order(&self) -> Result<Vec<String>, EngineError> {
        let mut remaining: BTreeMap<&str, BTreeSet<&str>> = self
            .projects
            .keys()
            .map(|id| (id.as_str(), self.deps_of(id).into_iter().collect()))
            .collect();

        let mut ordered: Vec<String> = Vec::with_capacity(remaining.len());
        while !remaining.is_empty() {
            let ready: Vec<&str> = remaining
                .iter()
                .filter(|(_, deps)| deps.is_empty())
                .map(|(id, _)| *id)
                .collect();

            if ready.is_empty() {
                let cycle: Vec<&str> = remaining.keys().copied().collect();
                return Err(EngineError::Precondition(format!(
                    "projects {} form a dependency cycle and cannot be ordered",
                    cycle.join(", ")
                )));
            }

            for id in ready {
                remaining.remove(id);
                for deps in remaining.values_mut() {
                    deps.remove(id);
                }
                ordered.push(id.to_owned());
            }
        }
        Ok(ordered)
    }
}

#[cfg(test)]
#[path = "project_graph.tests.rs"]
mod tests;
