//! Orchestration presets — predefined multi-agent patterns.
//!
//! Each preset defines a topology, partition strategy, budget distribution,
//! and agent prompts for common multi-agent workflows.

use serde::{Deserialize, Serialize};

use super::budget::PoolBudget;
use super::conflict::ConflictResolution;
use super::partition::PartitionStrategy;

// ─── Orchestrator Preset ───────────────────────────────────────────────────

/// Predefined multi-agent orchestration patterns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrchestratorPreset {
    /// Sequential: research first, then implement based on findings.
    ///
    /// - 1 research worker → findings published to shared context
    /// - 1 implementation worker → reads research findings, writes code
    /// - 1 test worker → writes tests for the implementation
    ResearchThenImplement,

    /// Parallel TDD: run implementation and testing in parallel.
    ///
    /// - N workers, each handling a component with TDD
    /// - Conflict detection for shared files
    /// - Split-by-component strategy
    ParallelTdd,

    /// Review pair: one worker implements, another reviews.
    ///
    /// - 1 implementation worker → writes code
    /// - 1 review worker → reads the implementation and provides feedback
    ReviewPair,

    /// Fullstack feature: parallel backend + frontend agents with shared context.
    ///
    /// - 1 worker per archetype, running in parallel
    /// - Cross-archetype context injection for contract files
    /// - Conflict detection for shared types and API contracts
    FullstackFeature,

    /// API-first: backend implements first, then frontend consumes.
    ///
    /// - Backend agent implements API changes first
    /// - Frontend agent runs after, with backend's contract files as context
    /// - Sequential handoff with cross-archetype context bridging
    ApiFirst,

    /// Test across stacks: per-archetype test agents run in parallel.
    ///
    /// - 1 tester agent per archetype, running in parallel
    /// - Each tester gets archetype-specific test conventions
    /// - Per-archetype gate verification
    TestAcrossStacks,
}

impl OrchestratorPreset {
    /// Get all available presets.
    #[must_use]
    pub fn all() -> Vec<Self> {
        vec![
            Self::ResearchThenImplement,
            Self::ParallelTdd,
            Self::ReviewPair,
            Self::FullstackFeature,
            Self::ApiFirst,
            Self::TestAcrossStacks,
        ]
    }

    /// Get the partition strategy for this preset.
    #[must_use]
    pub fn partition_strategy(&self) -> PartitionStrategy {
        match self {
            Self::ResearchThenImplement => PartitionStrategy::SplitByPhase,
            Self::ParallelTdd => PartitionStrategy::SplitByComponent,
            Self::ReviewPair => PartitionStrategy::SplitByFile,
            Self::FullstackFeature | Self::ApiFirst | Self::TestAcrossStacks => {
                PartitionStrategy::SplitByArchetype
            }
        }
    }

    /// Get the default conflict resolution for this preset.
    #[must_use]
    pub fn conflict_resolution(&self) -> ConflictResolution {
        match self {
            Self::ResearchThenImplement | Self::ApiFirst => ConflictResolution::Serialize,
            Self::ParallelTdd | Self::FullstackFeature | Self::TestAcrossStacks => {
                ConflictResolution::Merge
            }
            Self::ReviewPair => ConflictResolution::LastWriteWins,
        }
    }

    /// Get the default pool budget for this preset.
    #[must_use]
    pub fn default_budget(&self) -> PoolBudget {
        match self {
            Self::ResearchThenImplement => PoolBudget {
                max_total_cost_usd: 5.0,
                max_concurrent_workers: 1, // Sequential
                ..PoolBudget::unlimited()
            },
            Self::ParallelTdd => PoolBudget {
                max_total_cost_usd: 10.0,
                max_concurrent_workers: 4,
                ..PoolBudget::unlimited()
            },
            Self::ReviewPair => PoolBudget {
                max_total_cost_usd: 3.0,
                max_concurrent_workers: 2,
                ..PoolBudget::unlimited()
            },
            Self::FullstackFeature => PoolBudget {
                max_total_cost_usd: 15.0,
                max_concurrent_workers: 4,
                ..PoolBudget::unlimited()
            },
            Self::ApiFirst => PoolBudget {
                max_total_cost_usd: 10.0,
                max_concurrent_workers: 1, // Sequential
                ..PoolBudget::unlimited()
            },
            Self::TestAcrossStacks => PoolBudget {
                max_total_cost_usd: 8.0,
                max_concurrent_workers: 4,
                ..PoolBudget::unlimited()
            },
        }
    }

    /// Get a human-readable description of this preset.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::ResearchThenImplement => {
                "Sequential: research first, then implement based on findings"
            }
            Self::ParallelTdd => "Parallel TDD: multiple workers implementing components with TDD",
            Self::ReviewPair => "Review pair: one implements, another reviews",
            Self::FullstackFeature => {
                "Fullstack: parallel backend + frontend agents with shared context"
            }
            Self::ApiFirst => "API-first: backend implements first, frontend consumes after",
            Self::TestAcrossStacks => "Test across stacks: per-archetype test agents in parallel",
        }
    }
}

impl std::fmt::Display for OrchestratorPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResearchThenImplement => write!(f, "research-then-implement"),
            Self::ParallelTdd => write!(f, "parallel-tdd"),
            Self::ReviewPair => write!(f, "review-pair"),
            Self::FullstackFeature => write!(f, "fullstack-feature"),
            Self::ApiFirst => write!(f, "api-first"),
            Self::TestAcrossStacks => write!(f, "test-across-stacks"),
        }
    }
}

impl std::str::FromStr for OrchestratorPreset {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "research-then-implement" | "research" => Ok(Self::ResearchThenImplement),
            "parallel-tdd" | "tdd" => Ok(Self::ParallelTdd),
            "review-pair" | "review" => Ok(Self::ReviewPair),
            "fullstack-feature" | "fullstack" => Ok(Self::FullstackFeature),
            "api-first" | "api" => Ok(Self::ApiFirst),
            "test-across-stacks" | "test-stacks" => Ok(Self::TestAcrossStacks),
            other => Err(format!(
                "unknown preset '{other}'. Available: research-then-implement, parallel-tdd, review-pair, fullstack-feature, api-first, test-across-stacks"
            )),
        }
    }
}

#[cfg(test)]
#[path = "preset.tests.rs"]
mod tests;
