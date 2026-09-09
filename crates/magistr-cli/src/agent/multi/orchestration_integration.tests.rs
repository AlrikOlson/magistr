//! MG2 integration tests — orchestration patterns, conflict detection,
//! context bridging, budget enforcement, and canvas event emission.
//!
//! These tests verify the full orchestration pipeline without launching
//! real ACP sessions. They exercise partitioning → conflict detection →
//! context sharing → budget distribution → canvas event wiring.

use std::path::PathBuf;

use tokio::sync::mpsc;

use super::budget::{AgentBudget, PoolBudget, distribute_budget};
use super::canvas::{CanvasEvent, EdgeMessageType, TokenUsage};
use super::conflict::{ConflictDetector, ConflictResolution};
use super::context::SharedContext;
use super::partition::{PartitionStrategy, auto_partition, auto_select_strategy, partition};
use super::preset::OrchestratorPreset;
use super::supervisor::SupervisorConfig;
use super::topology::AgentId;

// ─── Research-Then-Implement Pattern ──────────────────────────────────────

#[test]
fn research_then_implement_produces_phase_subtasks() {
    let subtasks = partition(
        "Build auth module",
        &[],
        &[],
        &PartitionStrategy::SplitByPhase,
    );
    assert_eq!(subtasks.len(), 3);
    assert_eq!(subtasks[0].task_id, "phase-research");
    assert_eq!(subtasks[1].task_id, "phase-implement");
    assert_eq!(subtasks[2].task_id, "phase-test");
}

#[test]
fn research_then_implement_context_bridging() {
    let ctx = SharedContext::new();
    let researcher = AgentId::new(1);

    // Researcher publishes findings.
    ctx.extract_and_publish(researcher, "researcher", "Found REST pattern with JWT");

    // Coder's injection should include research findings.
    let injection = ctx.build_injection("coder", "implement auth");
    assert!(injection.contains("Found REST pattern with JWT"));
    assert!(injection.contains("research_findings"));
}

#[test]
fn research_then_implement_preset_config() {
    let preset = OrchestratorPreset::ResearchThenImplement;
    assert_eq!(preset.partition_strategy(), PartitionStrategy::SplitByPhase);
    assert_eq!(preset.conflict_resolution(), ConflictResolution::Serialize);
    assert_eq!(preset.default_budget().max_concurrent_workers, 1);
}

// ─── Review-Pair Pattern ──────────────────────────────────────────────────

#[test]
fn review_pair_alternating_rounds_context_flow() {
    let ctx = SharedContext::new();
    let coder = AgentId::new(1);
    let reviewer = AgentId::new(2);

    // Round 1: Coder publishes output.
    ctx.extract_and_publish(coder, "coder", "Added login endpoint");

    // Reviewer should see coder output.
    let injection = ctx.build_injection("reviewer", "review code");
    assert!(injection.contains("Added login endpoint"));

    // Round 1: Reviewer publishes feedback.
    ctx.extract_and_publish(reviewer, "reviewer", "REVISIONS NEEDED: add validation");

    // Researcher (or any role with full context access) sees all findings.
    let all_injection = ctx.build_injection("researcher", "check everything");
    assert!(all_injection.contains("add validation"));

    // The ReviewPair executor injects critique directly into the coder's
    // prompt (not via shared context), so the coder's filtered context
    // won't contain review_feedback — this is by design.
    assert_eq!(ctx.total_findings(), 2);
}

#[test]
fn review_pair_preset_config() {
    let preset = OrchestratorPreset::ReviewPair;
    assert_eq!(preset.partition_strategy(), PartitionStrategy::SplitByFile);
    assert_eq!(
        preset.conflict_resolution(),
        ConflictResolution::LastWriteWins
    );
    assert_eq!(preset.default_budget().max_concurrent_workers, 2);
}

// ─── Parallel-TDD Pattern with Conflict Detection ─────────────────────────

#[test]
fn parallel_tdd_conflict_detection() {
    let mut detector = ConflictDetector::with_default_resolution(ConflictResolution::Merge);
    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);
    let shared_file = PathBuf::from("src/shared.rs");

    // Both agents claim the same file.
    assert!(detector.record_edit(a1, shared_file.clone()).is_none());
    let conflict = detector.record_edit(a2, shared_file.clone());
    assert!(conflict.is_some());

    let c = conflict.expect("conflict expected");
    assert_eq!(c.file, shared_file);
    assert!(c.agents.contains(&a1));
    assert!(c.agents.contains(&a2));
    assert_eq!(
        detector.default_resolution(),
        Some(ConflictResolution::Merge)
    );
}

#[test]
fn parallel_tdd_no_conflict_separate_files() {
    let mut detector = ConflictDetector::new();
    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);

    assert!(
        detector
            .record_edit(a1, PathBuf::from("src/a.rs"))
            .is_none()
    );
    assert!(
        detector
            .record_edit(a2, PathBuf::from("src/b.rs"))
            .is_none()
    );
    assert!(detector.detect_conflicts().is_empty());
}

#[test]
fn parallel_tdd_preset_config() {
    let preset = OrchestratorPreset::ParallelTdd;
    assert_eq!(
        preset.partition_strategy(),
        PartitionStrategy::SplitByComponent
    );
    assert_eq!(preset.conflict_resolution(), ConflictResolution::Merge);
    assert_eq!(preset.default_budget().max_concurrent_workers, 4);
}

#[test]
fn parallel_tdd_component_partitioning() {
    let components = vec!["engine".into(), "tui".into(), "agent".into()];
    let subtasks = partition(
        "refactor",
        &[],
        &components,
        &PartitionStrategy::SplitByComponent,
    );
    assert_eq!(subtasks.len(), 3);

    for (i, task) in subtasks.iter().enumerate() {
        assert_eq!(task.strategy, PartitionStrategy::SplitByComponent);
        assert_eq!(task.scope.len(), 1);
        assert_eq!(task.scope[0], PathBuf::from(&components[i]));
    }
}

// ─── Budget Enforcement ───────────────────────────────────────────────────

#[test]
fn budget_enforcement_per_agent() {
    let budget = AgentBudget {
        max_tokens: 10_000,
        max_cost_usd: 1.0,
        max_turns: 0,
    };

    // Under budget.
    assert!(!budget.is_exceeded(5_000, 0.5, 0));

    // Token exceeded.
    assert!(budget.is_exceeded(10_001, 0.5, 0));
    let reason = budget.exceeded_reason(10_001, 0.5, 0).unwrap();
    assert!(reason.contains("token limit"));

    // Cost exceeded.
    assert!(budget.is_exceeded(5_000, 1.01, 0));
}

#[test]
fn budget_enforcement_pool_level() {
    let budget = PoolBudget {
        max_total_cost_usd: 10.0,
        max_total_tokens: 100_000,
        max_concurrent_workers: 4,
        per_agent_default: AgentBudget::unlimited(),
    };

    assert!(!budget.is_exceeded(9.0, 90_000));
    assert!(budget.is_exceeded(10.01, 0));
    assert!(budget.is_exceeded(0.0, 100_001));
    assert!(budget.can_add_worker(3));
    assert!(!budget.can_add_worker(4));
}

#[test]
fn budget_distribution_preserves_total() {
    let roles = &["coder", "researcher", "reviewer"];
    let total_cost = 10.0;
    let budgets = distribute_budget(total_cost, 0, roles);
    assert_eq!(budgets.len(), 3);

    let sum: f64 = budgets.iter().map(|b| b.max_cost_usd).sum();
    assert!((sum - total_cost).abs() < 0.01);
}

#[test]
fn budget_distribution_respects_weights() {
    let roles = &["coder", "reviewer"];
    let budgets = distribute_budget(13.0, 0, roles);
    // coder=1.0, reviewer=0.3, total=1.3
    // coder share = 1.0/1.3 * 13 = 10.0
    // reviewer share = 0.3/1.3 * 13 = 3.0
    assert!((budgets[0].max_cost_usd - 10.0).abs() < 0.01);
    assert!((budgets[1].max_cost_usd - 3.0).abs() < 0.01);
}

// ─── Canvas Event Emission ────────────────────────────────────────────────

#[test]
fn canvas_event_message_sent_serialization() {
    let event = CanvasEvent::MessageSent {
        source: AgentId::new(0),
        target: AgentId::new(1),
        message_type: EdgeMessageType::TaskDelegation,
    };

    let json = serde_json::to_string(&event).expect("serialize");
    assert!(json.contains("MessageSent"));
    assert!(json.contains("TaskDelegation"));
}

#[test]
fn canvas_event_conflict_detected_serialization() {
    let event = CanvasEvent::ConflictDetected {
        file: PathBuf::from("src/shared.rs"),
        agents: vec![AgentId::new(1), AgentId::new(2)],
    };

    let json = serde_json::to_string(&event).expect("serialize");
    assert!(json.contains("ConflictDetected"));
    assert!(json.contains("src/shared.rs"));
}

#[test]
fn canvas_event_token_usage_updated() {
    let event = CanvasEvent::TokenUsageUpdated {
        agent_id: AgentId::new(1),
        usage: TokenUsage::new(5000, 10000),
    };

    let json = serde_json::to_string(&event).expect("serialize");
    assert!(json.contains("TokenUsageUpdated"));
    assert!(json.contains("5000"));
    assert!(json.contains("10000"));
}

#[test]
fn canvas_event_delegation_result_context_types() {
    // All three message types serialize correctly.
    for (msg_type, expected) in [
        (EdgeMessageType::TaskDelegation, "TaskDelegation"),
        (EdgeMessageType::Result, "Result"),
        (EdgeMessageType::ContextUpdate, "ContextUpdate"),
        (EdgeMessageType::ConflictSignal, "ConflictSignal"),
    ] {
        let event = CanvasEvent::MessageSent {
            source: AgentId::new(0),
            target: AgentId::new(1),
            message_type: msg_type,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains(expected), "missing {expected} in {json}");
    }
}

// ─── Auto-Strategy Selection Integration ──────────────────────────────────

#[test]
fn auto_strategy_selects_component_for_multi_crate_project() {
    let components = vec!["core".into(), "gui".into(), "cli".into()];
    let strategy = auto_select_strategy("refactor module boundaries", &[], &components);
    assert_eq!(strategy, PartitionStrategy::SplitByComponent);

    let tasks = auto_partition("refactor module boundaries", &[], &components);
    assert_eq!(tasks.len(), 3);
}

#[test]
fn auto_strategy_selects_file_for_many_files() {
    let files: Vec<PathBuf> = (0..15)
        .map(|i| PathBuf::from(format!("src/f{i}.rs")))
        .collect();
    let strategy = auto_select_strategy("fix formatting", &files, &[]);
    assert_eq!(strategy, PartitionStrategy::SplitByFile);
}

#[test]
fn auto_strategy_selects_phase_for_greenfield() {
    let strategy = auto_select_strategy("build a new auth system", &[], &[]);
    assert_eq!(strategy, PartitionStrategy::SplitByPhase);
}

// ─── Context + Conflict + Budget Full Pipeline ────────────────────────────

#[test]
fn full_pipeline_research_then_implement() {
    // 1. Select strategy
    let strategy = OrchestratorPreset::ResearchThenImplement.partition_strategy();
    assert_eq!(strategy, PartitionStrategy::SplitByPhase);

    // 2. Partition
    let subtasks = partition("build auth", &[], &[], &strategy);
    assert_eq!(subtasks.len(), 3);

    // 3. Allocate budgets
    let roles = &["researcher", "coder", "tester"];
    let budgets = distribute_budget(5.0, 0, roles);
    assert_eq!(budgets.len(), 3);

    // Researcher gets least, coder gets most
    assert!(budgets[1].max_cost_usd > budgets[0].max_cost_usd);
    assert!(budgets[1].max_cost_usd > budgets[2].max_cost_usd);

    // 4. Context flows
    let ctx = SharedContext::new();
    ctx.extract_and_publish(AgentId::new(1), "researcher", "Use JWT + refresh tokens");
    let coder_injection = ctx.build_injection("coder", "implement auth");
    assert!(coder_injection.contains("JWT"));

    ctx.extract_and_publish(AgentId::new(2), "coder", "Added /auth/login endpoint");
    let tester_injection = ctx.build_injection("tester", "test auth");
    assert!(tester_injection.contains("/auth/login"));
}

#[test]
fn full_pipeline_parallel_tdd_with_conflicts() {
    // 1. Partition by component
    let components = vec!["auth".into(), "db".into()];
    let subtasks = partition(
        "add feature",
        &[],
        &components,
        &PartitionStrategy::SplitByComponent,
    );
    assert_eq!(subtasks.len(), 2);

    // 2. Budget
    let budgets = distribute_budget(10.0, 0, &["coder", "coder"]);
    assert!((budgets[0].max_cost_usd - 5.0).abs() < 0.01);

    // 3. Conflict detection
    let mut detector = ConflictDetector::with_default_resolution(ConflictResolution::Merge);
    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);

    detector.record_edit(a1, PathBuf::from("src/auth/mod.rs"));
    detector.record_edit(a2, PathBuf::from("src/db/mod.rs"));
    assert!(detector.detect_conflicts().is_empty());

    // Both touch shared config
    detector.record_edit(a1, PathBuf::from("config.toml"));
    let conflict = detector.record_edit(a2, PathBuf::from("config.toml"));
    assert!(conflict.is_some());

    // 4. Context sharing
    let ctx = SharedContext::new();
    ctx.extract_and_publish(a1, "coder", "Added auth middleware");
    ctx.extract_and_publish(a2, "coder", "Added DB pool");
    assert_eq!(ctx.total_findings(), 2);
}

// ─── Supervisor Config ────────────────────────────────────────────────────

#[test]
fn supervisor_config_with_preset() {
    let config = SupervisorConfig::new("/tmp")
        .max_retries(3)
        .budget(OrchestratorPreset::ParallelTdd.default_budget())
        .preset(OrchestratorPreset::ParallelTdd);

    assert_eq!(config.max_retries, 3);
    assert_eq!(config.budget.max_concurrent_workers, 4);
    assert_eq!(config.preset, Some(OrchestratorPreset::ParallelTdd));
}

// ─── Canvas Event Channel ─────────────────────────────────────────────────

#[test]
fn canvas_tx_receives_events() {
    let (tx, mut rx) = mpsc::unbounded_channel::<CanvasEvent>();

    // Simulate emit_canvas from pattern executor.
    tx.send(CanvasEvent::MessageSent {
        source: AgentId::new(0),
        target: AgentId::new(1),
        message_type: EdgeMessageType::TaskDelegation,
    })
    .expect("send");

    tx.send(CanvasEvent::ConflictDetected {
        file: PathBuf::from("shared.rs"),
        agents: vec![AgentId::new(1), AgentId::new(2)],
    })
    .expect("send");

    // Verify both events received.
    let e1 = rx.try_recv().expect("event 1");
    assert!(matches!(e1, CanvasEvent::MessageSent { .. }));

    let e2 = rx.try_recv().expect("event 2");
    assert!(matches!(e2, CanvasEvent::ConflictDetected { .. }));
}
