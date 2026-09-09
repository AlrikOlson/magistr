//! Conflict detection and resolution for multi-agent file editing.
//!
//! When multiple agents operate in parallel, they may attempt to modify the
//! same file. This module tracks file ownership, detects conflicts, and
//! provides resolution strategies.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::topology::AgentId;

// ─── Conflict ──────────────────────────────────────────────────────────────

/// A detected conflict where multiple agents claimed the same file.
#[derive(Debug, Clone)]
pub struct Conflict {
    /// The contested file path.
    pub file: PathBuf,
    /// Agents that claimed this file.
    pub agents: Vec<AgentId>,
}

/// Strategy for resolving a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    /// Serialize access — queue agents so only one modifies at a time.
    Serialize,
    /// Attempt a 3-way merge of the changes.
    Merge,
    /// Last writer wins — accept the most recently completed agent's changes.
    LastWriteWins,
}

impl std::fmt::Display for ConflictResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialize => write!(f, "serialize"),
            Self::Merge => write!(f, "merge"),
            Self::LastWriteWins => write!(f, "last-write-wins"),
        }
    }
}

// ─── Conflict Detector ─────────────────────────────────────────────────────

/// Tracks file ownership across agents and detects conflicts.
#[derive(Debug, Default)]
pub struct ConflictDetector {
    /// Map from file path to the set of agents that claimed it.
    claims: HashMap<PathBuf, HashSet<AgentId>>,
    /// Default resolution strategy.
    default_resolution: Option<ConflictResolution>,
}

impl ConflictDetector {
    /// Create a new conflict detector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            claims: HashMap::new(),
            default_resolution: None,
        }
    }

    /// Create a new conflict detector with a default resolution strategy.
    #[must_use]
    pub fn with_default_resolution(resolution: ConflictResolution) -> Self {
        Self {
            claims: HashMap::new(),
            default_resolution: Some(resolution),
        }
    }

    /// Register an agent's claim on a file.
    pub fn claim_file(&mut self, agent_id: AgentId, file: PathBuf) {
        self.claims.entry(file).or_default().insert(agent_id);
    }

    /// Release an agent's claim on a file.
    pub fn release_file(&mut self, agent_id: AgentId, file: &Path) {
        if let Some(agents) = self.claims.get_mut(file) {
            agents.remove(&agent_id);
            if agents.is_empty() {
                self.claims.remove(file);
            }
        }
    }

    /// Release all claims for an agent.
    pub fn release_all(&mut self, agent_id: AgentId) {
        self.claims.retain(|_, agents| {
            agents.remove(&agent_id);
            !agents.is_empty()
        });
    }

    /// Detect all current conflicts (files claimed by 2+ agents).
    #[must_use]
    pub fn detect_conflicts(&self) -> Vec<Conflict> {
        self.claims
            .iter()
            .filter(|(_, agents)| agents.len() > 1)
            .map(|(file, agents)| Conflict {
                file: file.clone(),
                agents: agents.iter().copied().collect(),
            })
            .collect()
    }

    /// Check if a specific file has a conflict.
    #[must_use]
    pub fn has_conflict(&self, file: &Path) -> bool {
        self.claims.get(file).is_some_and(|agents| agents.len() > 1)
    }

    /// Get the agents that claimed a specific file.
    #[must_use]
    pub fn agents_for_file(&self, file: &Path) -> Vec<AgentId> {
        self.claims
            .get(file)
            .map(|agents| agents.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Record an agent editing a file and return a conflict if this creates one.
    ///
    /// This is the event-driven entry point: call it each time a `FileChange`
    /// event arrives. If the file is already claimed by a different agent,
    /// a [`Conflict`] is returned with all involved agents.
    pub fn record_edit(&mut self, agent_id: AgentId, file: PathBuf) -> Option<Conflict> {
        self.claim_file(agent_id, file.clone());
        if self.has_conflict(&file) {
            Some(Conflict {
                file: file.clone(),
                agents: self.agents_for_file(&file),
            })
        } else {
            None
        }
    }

    /// Get the number of tracked files.
    #[must_use]
    pub fn tracked_file_count(&self) -> usize {
        self.claims.len()
    }

    /// Get the default resolution strategy.
    #[must_use]
    pub fn default_resolution(&self) -> Option<ConflictResolution> {
        self.default_resolution
    }
}

#[cfg(test)]
#[path = "conflict.tests.rs"]
mod tests;
