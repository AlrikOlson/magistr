//! Current work, terminal history and exact saved focus are projections of the
//! same complete roadmap. These types share readiness and name resolution so
//! every host offers the same work without persisting a second catalog.
use super::{BlockedBy, Chunk, ChunkStatus, Readiness, next_ready};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamCounts {
    pub total: usize,
    pub backlog: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub blocked: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamReadinessCounts {
    pub ready: usize,
    pub waiting: usize,
    pub blocked: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamActivity {
    pub counts: WorkstreamCounts,
    pub readiness: WorkstreamReadinessCounts,
    pub next_ready: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamHistory {
    pub done: usize,
    pub obsoleted: usize,
}

impl WorkstreamHistory {
    fn include(&mut self, status: ChunkStatus) {
        match status {
            ChunkStatus::Done => self.done += 1,
            ChunkStatus::Obsoleted => self.obsoleted += 1,
            _ => unreachable!("terminal members only"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamHistoryRecords {
    #[serde(flatten)]
    pub counts: WorkstreamHistory,
    /// Terminal records in canonical insertion order, available on inspection.
    pub chunk_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamCatalog {
    pub current: BTreeMap<String, WorkstreamActivity>,
    pub history: BTreeMap<String, WorkstreamHistory>,
    pub ungrouped: WorkstreamActivity,
    pub ungrouped_history: WorkstreamHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamDependency {
    pub id: String,
    /// None means the dependency is absent from the complete snapshot.
    pub status: Option<ChunkStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorkstreamReadiness {
    Ready,
    Waiting {
        dependencies: Vec<WorkstreamDependency>,
    },
    Blocked {
        blocker: Option<BlockedBy>,
    },
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamItem {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub status: ChunkStatus,
    pub readiness: WorkstreamReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamFrontier {
    /// None selects ungrouped work; it never means all workstreams.
    pub group: Option<String>,
    pub activity: WorkstreamActivity,
    /// Active records in canonical insertion order.
    pub items: Vec<WorkstreamItem>,
}

/// The exact saved workstream's relationship to the current roadmap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FocusState {
    Unset,
    Current { group: String },
    Closed { group: String },
    Missing { group: String },
}

/// Explicit selection may resolve a current name, but an exact closed name
/// never falls through to a similarly named current workstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorkstreamResolution {
    Current { group: String },
    Closed { group: String, choices: Vec<String> },
    Ambiguous { matches: Vec<String> },
    Unknown { choices: Vec<String> },
}

impl WorkstreamResolution {
    #[must_use]
    pub fn current(&self) -> Option<&str> {
        match self {
            Self::Current { group } => Some(group),
            _ => None,
        }
    }

    #[must_use]
    pub fn explain(&self, query: &str) -> String {
        let choices = |names: &[String]| {
            if names.is_empty() {
                "none; no named workstream has unfinished work".to_owned()
            } else {
                names.join(", ")
            }
        };
        match self {
            Self::Current { group } => format!("'{query}' resolved to '{group}'"),
            Self::Closed {
                group,
                choices: names,
            } => format!(
                "'{group}' has no unfinished work. Focus is unchanged. Current workstreams: {}",
                choices(names)
            ),
            Self::Ambiguous { matches } => format!(
                "'{query}' is ambiguous; it matches {}. Focus is unchanged; name one exactly.",
                choices(matches)
            ),
            Self::Unknown { choices: names } => format!(
                "No current workstream matches '{query}'. Focus is unchanged. Current workstreams: {}",
                choices(names)
            ),
        }
    }
}

impl WorkstreamCatalog {
    #[must_use]
    pub fn authored_names(&self) -> Vec<String> {
        self.current
            .keys()
            .chain(self.history.keys())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    #[must_use]
    pub fn current_names(&self) -> Vec<String> {
        self.current.keys().cloned().collect()
    }

    /// Stored focus is an identity, not a search query. No normalization or
    /// fragment matching is allowed when classifying it.
    #[must_use]
    pub fn focus_state(&self, group: Option<&str>) -> FocusState {
        match group {
            None => FocusState::Unset,
            Some(group) if self.current.contains_key(group) => FocusState::Current {
                group: group.into(),
            },
            Some(group) if self.history.contains_key(group) => FocusState::Closed {
                group: group.into(),
            },
            Some(group) => FocusState::Missing {
                group: group.into(),
            },
        }
    }

    #[must_use]
    pub fn resolve_current(&self, query: &str) -> WorkstreamResolution {
        let query = query.trim();
        if self.current.contains_key(query) {
            return WorkstreamResolution::Current {
                group: query.into(),
            };
        }
        let choices = self.current_names();
        if self.history.contains_key(query) {
            return WorkstreamResolution::Closed {
                group: query.into(),
                choices,
            };
        }
        if query.is_empty() {
            return WorkstreamResolution::Unknown { choices };
        }
        let lower = query.to_lowercase();
        let exact: Vec<_> = self
            .authored_names()
            .into_iter()
            .filter(|name| name.to_lowercase() == lower)
            .collect();
        if exact.len() > 1 {
            return WorkstreamResolution::Ambiguous { matches: exact };
        }
        if let Some(group) = exact.into_iter().next() {
            return if self.current.contains_key(&group) {
                WorkstreamResolution::Current { group }
            } else {
                WorkstreamResolution::Closed { group, choices }
            };
        }
        let matches: Vec<_> = choices
            .iter()
            .filter(|name| name.to_lowercase().contains(&lower))
            .cloned()
            .collect();
        match matches.len() {
            0 => WorkstreamResolution::Unknown { choices },
            1 => WorkstreamResolution::Current {
                group: matches.into_iter().next().expect("one match"),
            },
            _ => WorkstreamResolution::Ambiguous { matches },
        }
    }
}

fn readiness(chunk: &Chunk, snapshot: &BTreeMap<&str, &Chunk>) -> WorkstreamReadiness {
    match chunk.readiness(|id| snapshot.get(id).map(|dependency| dependency.status)) {
        Readiness::Ready => WorkstreamReadiness::Ready,
        Readiness::Waiting(dependencies) => WorkstreamReadiness::Waiting {
            dependencies: dependencies
                .into_iter()
                .map(|dependency| WorkstreamDependency {
                    id: dependency.id.into(),
                    status: dependency.status,
                })
                .collect(),
        },
        Readiness::Blocked(_) | Readiness::Inactive(ChunkStatus::Blocked) => {
            WorkstreamReadiness::Blocked {
                blocker: chunk.blocked_by.clone(),
            }
        }
        Readiness::Inactive(_) => WorkstreamReadiness::Inactive,
    }
}

fn activity(members: &[&Chunk], snapshot: &BTreeMap<&str, &Chunk>) -> WorkstreamActivity {
    let mut activity = WorkstreamActivity::default();
    for chunk in members {
        activity.counts.total += 1;
        match chunk.status {
            ChunkStatus::Backlog => activity.counts.backlog += 1,
            ChunkStatus::Pending => activity.counts.pending += 1,
            ChunkStatus::InProgress => activity.counts.in_progress += 1,
            ChunkStatus::Blocked => activity.counts.blocked += 1,
            ChunkStatus::Done | ChunkStatus::Obsoleted => unreachable!("current members only"),
        }
        match readiness(chunk, snapshot) {
            WorkstreamReadiness::Ready => activity.readiness.ready += 1,
            WorkstreamReadiness::Waiting { .. } => activity.readiness.waiting += 1,
            WorkstreamReadiness::Blocked { .. } => activity.readiness.blocked += 1,
            WorkstreamReadiness::Inactive => {}
        }
    }
    activity.next_ready = next_ready(
        members.iter().copied(),
        |chunk| {
            chunk
                .readiness(|id| snapshot.get(id).map(|dep| dep.status))
                .is_ready()
        },
        |chunk| chunk.priority,
    )
    .map(|chunk| chunk.id.clone());
    activity
}

/// Derive current activity and terminal history from the complete snapshot.
/// Authored group names are preserved byte-for-byte, including an empty name
/// if a record contains one; only None belongs to the ungrouped bucket.
#[must_use]
pub fn workstream_catalog(chunks: &[Chunk]) -> WorkstreamCatalog {
    let snapshot: BTreeMap<_, _> = chunks
        .iter()
        .map(|chunk| (chunk.id.as_str(), chunk))
        .collect();
    let mut current: BTreeMap<Option<&str>, Vec<&Chunk>> = BTreeMap::new();
    let mut catalog = WorkstreamCatalog::default();
    for chunk in chunks {
        if chunk.status.is_active() {
            current
                .entry(chunk.group.as_deref())
                .or_default()
                .push(chunk);
            continue;
        }
        let history = match &chunk.group {
            Some(group) => catalog.history.entry(group.clone()).or_default(),
            None => &mut catalog.ungrouped_history,
        };
        history.include(chunk.status);
    }
    for (group, members) in current {
        let activity = activity(&members, &snapshot);
        match group {
            Some(group) => {
                catalog.current.insert(group.into(), activity);
            }
            None => catalog.ungrouped = activity,
        }
    }
    catalog
}

/// Project exactly one current workstream using the complete dependency graph.
/// An absent or closed group has an empty frontier, never another group's work.
#[must_use]
pub fn workstream_frontier(chunks: &[Chunk], group: Option<&str>) -> WorkstreamFrontier {
    let snapshot: BTreeMap<_, _> = chunks
        .iter()
        .map(|chunk| (chunk.id.as_str(), chunk))
        .collect();
    let members: Vec<_> = chunks
        .iter()
        .filter(|chunk| chunk.status.is_active() && chunk.group.as_deref() == group)
        .collect();
    WorkstreamFrontier {
        group: group.map(str::to_owned),
        activity: activity(&members, &snapshot),
        items: members
            .into_iter()
            .map(|chunk| WorkstreamItem {
                id: chunk.id.clone(),
                name: chunk.name.clone(),
                priority: chunk.priority,
                status: chunk.status,
                readiness: readiness(chunk, &snapshot),
            })
            .collect(),
    }
}

/// Inspect terminal records for exactly one authored group, or ungrouped
/// history when None. Catalogs carry only counts; record IDs are read on demand.
#[must_use]
pub fn workstream_history(chunks: &[Chunk], group: Option<&str>) -> WorkstreamHistoryRecords {
    let mut history = WorkstreamHistoryRecords::default();
    for chunk in chunks
        .iter()
        .filter(|chunk| !chunk.status.is_active() && chunk.group.as_deref() == group)
    {
        history.counts.include(chunk.status);
        history.chunk_ids.push(chunk.id.clone());
    }
    history
}

#[cfg(test)]
#[path = "workstreams.tests.rs"]
mod tests;
