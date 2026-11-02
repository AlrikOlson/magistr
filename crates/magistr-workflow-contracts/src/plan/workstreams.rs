//! Current work, terminal history and exact saved focus are projections of the
//! same complete plan. These types share readiness and name resolution so
//! every host offers the same work without persisting a second catalog.
use super::{Readiness, Task, TaskStatus, WaitingOn, next_ready};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamCounts {
    pub total: usize,
    pub idea: usize,
    pub to_do: usize,
    pub in_progress: usize,
    pub waiting: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamReadinessCounts {
    pub ready: usize,
    pub dependent: usize,
    pub waiting: usize,
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
    pub retired: usize,
}

impl WorkstreamHistory {
    fn include(&mut self, status: TaskStatus) {
        match status {
            TaskStatus::Done => self.done += 1,
            TaskStatus::Retired => self.retired += 1,
            _ => unreachable!("terminal members only"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamHistoryRecords {
    #[serde(flatten)]
    pub counts: WorkstreamHistory,
    /// Terminal records in canonical insertion order, available on inspection.
    pub task_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamCatalog {
    pub current: BTreeMap<String, WorkstreamActivity>,
    pub history: BTreeMap<String, WorkstreamHistory>,
    pub unassigned: WorkstreamActivity,
    pub unassigned_history: WorkstreamHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamDependency {
    pub id: String,
    /// None means the dependency is absent from the complete snapshot.
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorkstreamReadiness {
    Ready,
    /// To do, with a dependency that is not done yet.
    Dependent {
        dependencies: Vec<WorkstreamDependency>,
    },
    /// Waiting on something named; `waiting_on` says what when it is recorded.
    Waiting {
        waiting_on: Option<WaitingOn>,
    },
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamItem {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub status: TaskStatus,
    pub readiness: WorkstreamReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkstreamFrontier {
    /// None selects unassigned work; it never means all workstreams.
    pub feature: Option<String>,
    pub activity: WorkstreamActivity,
    /// Active records in canonical insertion order.
    pub items: Vec<WorkstreamItem>,
}

/// The exact saved workstream's relationship to the current plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FocusState {
    Unset,
    Current { feature: String },
    Closed { feature: String },
    Missing { feature: String },
}

/// Explicit selection may resolve a current name, but an exact closed name
/// never falls through to a similarly named current workstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorkstreamResolution {
    Current {
        feature: String,
    },
    Closed {
        feature: String,
        choices: Vec<String>,
    },
    Ambiguous {
        matches: Vec<String>,
    },
    Unknown {
        choices: Vec<String>,
    },
}

impl WorkstreamResolution {
    #[must_use]
    pub fn current(&self) -> Option<&str> {
        match self {
            Self::Current { feature } => Some(feature),
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
            Self::Current { feature } => format!("'{query}' resolved to '{feature}'"),
            Self::Closed {
                feature,
                choices: names,
            } => format!(
                "'{feature}' has no unfinished work. Focus is unchanged. Current workstreams: {}",
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
    pub fn focus_state(&self, feature: Option<&str>) -> FocusState {
        match feature {
            None => FocusState::Unset,
            Some(feature) if self.current.contains_key(feature) => FocusState::Current {
                feature: feature.into(),
            },
            Some(feature) if self.history.contains_key(feature) => FocusState::Closed {
                feature: feature.into(),
            },
            Some(feature) => FocusState::Missing {
                feature: feature.into(),
            },
        }
    }

    #[must_use]
    pub fn resolve_current(&self, query: &str) -> WorkstreamResolution {
        let query = query.trim();
        if self.current.contains_key(query) {
            return WorkstreamResolution::Current {
                feature: query.into(),
            };
        }
        let choices = self.current_names();
        if self.history.contains_key(query) {
            return WorkstreamResolution::Closed {
                feature: query.into(),
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
        if let Some(feature) = exact.into_iter().next() {
            return if self.current.contains_key(&feature) {
                WorkstreamResolution::Current { feature }
            } else {
                WorkstreamResolution::Closed { feature, choices }
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
                feature: matches.into_iter().next().expect("one match"),
            },
            _ => WorkstreamResolution::Ambiguous { matches },
        }
    }
}

fn readiness(task: &Task, snapshot: &BTreeMap<&str, &Task>) -> WorkstreamReadiness {
    match task.readiness(|id| snapshot.get(id).map(|dependency| dependency.status)) {
        Readiness::Ready => WorkstreamReadiness::Ready,
        Readiness::Dependent(dependencies) => WorkstreamReadiness::Dependent {
            dependencies: dependencies
                .into_iter()
                .map(|dependency| WorkstreamDependency {
                    id: dependency.id.into(),
                    status: dependency.status,
                })
                .collect(),
        },
        Readiness::Waiting(_) | Readiness::Inactive(TaskStatus::Waiting) => {
            WorkstreamReadiness::Waiting {
                waiting_on: task.waiting_on.clone(),
            }
        }
        Readiness::Inactive(_) => WorkstreamReadiness::Inactive,
    }
}

fn activity(members: &[&Task], snapshot: &BTreeMap<&str, &Task>) -> WorkstreamActivity {
    let mut activity = WorkstreamActivity::default();
    for task in members {
        activity.counts.total += 1;
        match task.status {
            TaskStatus::Idea => activity.counts.idea += 1,
            TaskStatus::ToDo => activity.counts.to_do += 1,
            TaskStatus::InProgress => activity.counts.in_progress += 1,
            TaskStatus::Waiting => activity.counts.waiting += 1,
            TaskStatus::Done | TaskStatus::Retired => unreachable!("current members only"),
        }
        match readiness(task, snapshot) {
            WorkstreamReadiness::Ready => activity.readiness.ready += 1,
            WorkstreamReadiness::Dependent { .. } => activity.readiness.dependent += 1,
            WorkstreamReadiness::Waiting { .. } => activity.readiness.waiting += 1,
            WorkstreamReadiness::Inactive => {}
        }
    }
    activity.next_ready = next_ready(
        members.iter().copied(),
        |task| {
            task.readiness(|id| snapshot.get(id).map(|dep| dep.status))
                .is_ready()
        },
        |task| task.priority,
    )
    .map(|task| task.id.clone());
    activity
}

/// Derive current activity and terminal history from the complete snapshot.
/// Authored feature names are preserved byte-for-byte, including an empty name
/// if a record contains one; only None belongs to the unassigned bucket.
#[must_use]
pub fn workstream_catalog(tasks: &[Task]) -> WorkstreamCatalog {
    let snapshot: BTreeMap<_, _> = tasks.iter().map(|task| (task.id.as_str(), task)).collect();
    let mut current: BTreeMap<Option<&str>, Vec<&Task>> = BTreeMap::new();
    let mut catalog = WorkstreamCatalog::default();
    for task in tasks {
        if task.status.is_active() {
            current
                .entry(task.feature.as_deref())
                .or_default()
                .push(task);
            continue;
        }
        let history = match &task.feature {
            Some(feature) => catalog.history.entry(feature.clone()).or_default(),
            None => &mut catalog.unassigned_history,
        };
        history.include(task.status);
    }
    for (feature, members) in current {
        let activity = activity(&members, &snapshot);
        match feature {
            Some(feature) => {
                catalog.current.insert(feature.into(), activity);
            }
            None => catalog.unassigned = activity,
        }
    }
    catalog
}

/// Project exactly one current workstream using the complete dependency graph.
/// An absent or closed feature has an empty frontier, never another feature's work.
#[must_use]
pub fn workstream_frontier(tasks: &[Task], feature: Option<&str>) -> WorkstreamFrontier {
    let snapshot: BTreeMap<_, _> = tasks.iter().map(|task| (task.id.as_str(), task)).collect();
    let members: Vec<_> = tasks
        .iter()
        .filter(|task| task.status.is_active() && task.feature.as_deref() == feature)
        .collect();
    WorkstreamFrontier {
        feature: feature.map(str::to_owned),
        activity: activity(&members, &snapshot),
        items: members
            .into_iter()
            .map(|task| WorkstreamItem {
                id: task.id.clone(),
                name: task.name.clone(),
                priority: task.priority,
                status: task.status,
                readiness: readiness(task, &snapshot),
            })
            .collect(),
    }
}

/// Inspect terminal records for exactly one authored feature, or unassigned
/// history when None. Catalogs carry only counts; record IDs are read on demand.
#[must_use]
pub fn workstream_history(tasks: &[Task], feature: Option<&str>) -> WorkstreamHistoryRecords {
    let mut history = WorkstreamHistoryRecords::default();
    for task in tasks
        .iter()
        .filter(|task| !task.status.is_active() && task.feature.as_deref() == feature)
    {
        history.counts.include(task.status);
        history.task_ids.push(task.id.clone());
    }
    history
}

#[cfg(test)]
#[path = "workstreams.tests.rs"]
mod tests;
