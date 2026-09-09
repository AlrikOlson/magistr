//! Integration tests for multi-agent orchestration.
//!
//! Tests the full interaction between topology, pool, conflict detection,
//! shared context, budget enforcement, partitioning, and presets.
//! Does NOT spawn real ACP sessions — tests use direct struct manipulation.

use std::path::PathBuf;

use magistr::agent::multi::budget::{AgentBudget, PoolBudget};
use magistr::agent::multi::conflict::{ConflictDetector, ConflictResolution};
use magistr::agent::multi::context::SharedContext;
use magistr::agent::multi::partition::{self, PartitionStrategy};
use magistr::agent::multi::preset::OrchestratorPreset;
use magistr::agent::multi::role::SpecializedRole;
use magistr::agent::multi::role_dispatch::{classify_subtask_role, dispatch_to_role};
use magistr::agent::multi::role_recommend::recommend_roles_for_task;
use magistr::agent::multi::supervisor::SupervisorConfig;
use magistr::agent::multi::tool_policy::{apply_tool_policy, is_tool_allowed};
use magistr::agent::multi::topology::{AgentId, AgentMsg, AgentRole, AgentTopology};

// ── Topology + Pool Integration ──────────────────────────────────────

#[test]
fn topology_tracks_pool_lifecycle() {
    let mut topo = AgentTopology::new();

    // Add supervisor + 3 workers
    let sup = AgentId::new(0);
    topo.add(sup, AgentRole::Supervisor);
    for i in 1..=3 {
        topo.add(
            AgentId::new(i),
            AgentRole::Worker {
                task_id: format!("task-{i}"),
            },
        );
    }
    assert_eq!(topo.len(), 4);
    assert_eq!(topo.worker_count(), 3);

    // Remove a worker
    topo.remove(AgentId::new(2));
    assert_eq!(topo.worker_count(), 2);
    assert!(topo.get(AgentId::new(2)).is_none());

    // Supervisor still present
    assert!(topo.get(sup).is_some());
}

// ── Conflict Detection + Resolution ─────────────────────────────────

#[test]
fn conflict_detection_across_multiple_agents() {
    let mut detector = ConflictDetector::with_default_resolution(ConflictResolution::Serialize);

    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);
    let a3 = AgentId::new(3);

    // Agent 1 claims files A, B
    detector.claim_file(a1, PathBuf::from("src/main.rs"));
    detector.claim_file(a1, PathBuf::from("src/lib.rs"));

    // Agent 2 claims files B, C (conflict on B)
    detector.claim_file(a2, PathBuf::from("src/lib.rs"));
    detector.claim_file(a2, PathBuf::from("src/utils.rs"));

    // Agent 3 claims file A (conflict on A)
    detector.claim_file(a3, PathBuf::from("src/main.rs"));

    let conflicts = detector.detect_conflicts();
    assert_eq!(conflicts.len(), 2); // main.rs and lib.rs

    // Release agent 1's claims
    detector.release_all(a1);
    let conflicts = detector.detect_conflicts();
    assert!(conflicts.is_empty()); // No more conflicts

    assert_eq!(
        detector.default_resolution(),
        Some(ConflictResolution::Serialize)
    );
}

// ── Shared Context Knowledge Sharing ────────────────────────────────

#[test]
fn shared_context_cross_agent_knowledge_flow() {
    let ctx = SharedContext::new();

    let researcher = AgentId::new(1);
    let implementer = AgentId::new(2);
    let tester = AgentId::new(3);

    // Researcher publishes findings
    ctx.publish(researcher, "api_pattern", "Use REST with pagination");
    ctx.publish(researcher, "api_pattern", "Auth via Bearer tokens");
    ctx.publish(researcher, "error_handling", "Use Result<T, AppError>");

    // Implementer reads research findings
    let patterns = ctx.query("api_pattern");
    assert_eq!(patterns.len(), 2);
    assert!(patterns[0].value.contains("REST"));

    // Implementer publishes implementation details
    ctx.publish(
        implementer,
        "implemented_endpoints",
        "/api/users, /api/items",
    );

    // Tester reads both research and implementation findings
    let error_approach = ctx.query("error_handling");
    assert_eq!(error_approach.len(), 1);

    let endpoints = ctx.query("implemented_endpoints");
    assert_eq!(endpoints.len(), 1);

    // All keys visible
    let mut keys = ctx.keys();
    keys.sort();
    assert_eq!(
        keys,
        vec!["api_pattern", "error_handling", "implemented_endpoints"]
    );

    assert_eq!(ctx.total_findings(), 4);

    // Per-agent filtering
    assert_eq!(ctx.findings_by_agent(researcher).len(), 3);
    assert_eq!(ctx.findings_by_agent(implementer).len(), 1);
    assert_eq!(ctx.findings_by_agent(tester).len(), 0);
}

// ── Budget Enforcement ──────────────────────────────────────────────

#[test]
fn budget_enforcement_per_agent_and_pool() {
    let agent_budget = AgentBudget {
        max_tokens: 50_000,
        max_cost_usd: 1.0,
        max_turns: 10,
    };

    // Under budget
    assert!(!agent_budget.is_exceeded(49_000, 0.9, 9));
    assert!(agent_budget.exceeded_reason(49_000, 0.9, 9).is_none());

    // Over token budget
    assert!(agent_budget.is_exceeded(51_000, 0.5, 5));
    let reason = agent_budget.exceeded_reason(51_000, 0.5, 5).unwrap();
    assert!(reason.contains("token"));

    // Pool budget
    let pool_budget = PoolBudget {
        max_total_tokens: 200_000,
        max_total_cost_usd: 5.0,
        max_concurrent_workers: 4,
        per_agent_default: agent_budget,
    };

    assert!(!pool_budget.is_exceeded(4.0, 150_000));
    assert!(pool_budget.is_exceeded(5.01, 150_000));
    assert!(pool_budget.is_exceeded(4.0, 200_001));

    assert!(pool_budget.can_add_worker(3));
    assert!(!pool_budget.can_add_worker(4));
}

// ── Partitioning ────────────────────────────────────────────────────

#[test]
fn partition_strategies_produce_correct_structure() {
    let files: Vec<PathBuf> = (0..8)
        .map(|i| PathBuf::from(format!("src/file{i}.rs")))
        .collect();
    let components = vec!["engine".into(), "tui".into(), "agent".into()];

    // Split by file: 8 files / 5 per worker = 2 chunks
    let tasks = partition::partition("implement", &files, &[], &PartitionStrategy::SplitByFile);
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].scope.len(), 5);
    assert_eq!(tasks[1].scope.len(), 3);

    // Split by phase: always 3 (research, implement, test)
    let tasks = partition::partition("build feature", &[], &[], &PartitionStrategy::SplitByPhase);
    assert_eq!(tasks.len(), 3);
    assert!(tasks[0].prompt.contains("RESEARCH"));
    assert!(tasks[1].prompt.contains("IMPLEMENT"));
    assert!(tasks[2].prompt.contains("TESTS"));

    // Split by component: 3 components = 3 tasks
    let tasks = partition::partition(
        "refactor",
        &[],
        &components,
        &PartitionStrategy::SplitByComponent,
    );
    assert_eq!(tasks.len(), 3);
    assert!(tasks[0].prompt.contains("engine"));
}

// ── Presets ─────────────────────────────────────────────────────────

#[test]
fn presets_produce_valid_configurations() {
    for preset in OrchestratorPreset::all() {
        let strategy = preset.partition_strategy();
        let resolution = preset.conflict_resolution();
        let budget = preset.default_budget();

        // Every preset has a cost limit
        assert!(
            budget.max_total_cost_usd > 0.0,
            "{preset} should have a cost limit"
        );

        // Description is non-empty
        assert!(!preset.description().is_empty());

        // Round-trip through Display + FromStr
        let name = preset.to_string();
        let parsed: OrchestratorPreset = name.parse().expect("preset should parse");
        assert_eq!(parsed, preset);

        // Strategy and resolution are consistent
        match preset {
            OrchestratorPreset::ResearchThenImplement => {
                assert_eq!(strategy, PartitionStrategy::SplitByPhase);
                assert_eq!(resolution, ConflictResolution::Serialize);
                assert_eq!(budget.max_concurrent_workers, 1);
            }
            OrchestratorPreset::ParallelTdd => {
                assert_eq!(strategy, PartitionStrategy::SplitByComponent);
                assert_eq!(resolution, ConflictResolution::Merge);
                assert!(budget.max_concurrent_workers > 1);
            }
            OrchestratorPreset::ReviewPair => {
                assert_eq!(strategy, PartitionStrategy::SplitByFile);
                assert_eq!(resolution, ConflictResolution::LastWriteWins);
                assert_eq!(budget.max_concurrent_workers, 2);
            }
            OrchestratorPreset::FullstackFeature
            | OrchestratorPreset::ApiFirst
            | OrchestratorPreset::TestAcrossStacks => {
                assert_eq!(strategy, PartitionStrategy::SplitByArchetype);
            }
        }
    }
}

// ── Message Protocol ────────────────────────────────────────────────

#[test]
fn agent_messages_have_correct_routing() {
    let assignment = AgentMsg::TaskAssignment {
        target: AgentId::new(1),
        prompt: "do work".into(),
        scope: vec![PathBuf::from("src/")],
    };
    assert!(assignment.source().is_none());
    assert_eq!(assignment.target(), Some(AgentId::new(1)));

    let result = AgentMsg::TaskResult {
        source: AgentId::new(1),
        success: true,
        summary: "done".into(),
        files_modified: vec![PathBuf::from("src/main.rs")],
        cost_usd: Some(0.05),
    };
    assert_eq!(result.source(), Some(AgentId::new(1)));
    assert!(result.target().is_none());

    let context_share = AgentMsg::ContextShare {
        source: AgentId::new(2),
        key: "finding".into(),
        value: "important".into(),
    };
    assert_eq!(context_share.source(), Some(AgentId::new(2)));
    assert!(context_share.target().is_none());

    let cancel = AgentMsg::CancelRequest {
        target: AgentId::new(3),
        reason: "budget exceeded".into(),
    };
    assert!(cancel.source().is_none());
    assert_eq!(cancel.target(), Some(AgentId::new(3)));
}

// ── Supervisor Config ───────────────────────────────────────────────

#[test]
fn supervisor_config_from_preset() {
    let budget = OrchestratorPreset::ParallelTdd.default_budget();
    let config = SupervisorConfig::new("/tmp/project")
        .max_retries(3)
        .budget(budget.clone());

    assert_eq!(config.max_retries, 3);
    assert_eq!(config.budget.max_total_cost_usd, budget.max_total_cost_usd);
    assert_eq!(
        config.budget.max_concurrent_workers,
        budget.max_concurrent_workers
    );
}

// ── End-to-End: Partition → Conflict Check → Context Share ──────────

#[test]
fn end_to_end_partition_conflict_context_flow() {
    // 1. Partition a task
    let files = vec![
        PathBuf::from("src/a.rs"),
        PathBuf::from("src/b.rs"),
        PathBuf::from("src/c.rs"),
    ];
    let subtasks = partition::partition(
        "implement feature",
        &files,
        &[],
        &PartitionStrategy::SplitByFile,
    );
    assert_eq!(subtasks.len(), 1); // 3 files < 5 max per worker

    // 2. Assign to agents and check for conflicts
    let mut detector = ConflictDetector::new();
    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);

    // Both agents claim overlapping files
    for f in &files[..2] {
        detector.claim_file(a1, f.clone());
    }
    for f in &files[1..] {
        detector.claim_file(a2, f.clone());
    }

    let conflicts = detector.detect_conflicts();
    assert_eq!(conflicts.len(), 1); // src/b.rs
    assert_eq!(conflicts[0].file, PathBuf::from("src/b.rs"));

    // 3. Resolve by releasing one agent's claim
    detector.release_file(a2, &PathBuf::from("src/b.rs"));
    assert!(detector.detect_conflicts().is_empty());

    // 4. Agents share findings via context
    let ctx = SharedContext::new();
    ctx.publish(a1, "approach", "use trait objects for extensibility");
    ctx.publish(a2, "approach", "confirmed — trait objects work well here");

    let approaches = ctx.query("approach");
    assert_eq!(approaches.len(), 2);
}

// ── Role System Integration ─────────────────────────────────────────

#[test]
fn role_loading_and_built_in_archetypes() {
    let roles = SpecializedRole::built_in_roles();
    assert_eq!(roles.len(), 5);

    let ids: Vec<&str> = roles.iter().map(|r| r.role_id.as_str()).collect();
    assert!(ids.contains(&"researcher"));
    assert!(ids.contains(&"coder"));
    assert!(ids.contains(&"reviewer"));
    assert!(ids.contains(&"tester"));
    assert!(ids.contains(&"documenter"));

    // Each role has a valid prompt template
    for role in &roles {
        assert!(role.system_prompt_template.contains("{task}"));
        let rendered = role.render_prompt("test task");
        assert!(rendered.contains("test task"));
        assert!(!rendered.contains("{task}"));
    }
}

#[test]
fn tool_policy_enforces_read_only_roles() {
    let base_tools: Vec<String> = vec![
        "Read".into(),
        "Edit".into(),
        "Write".into(),
        "Bash".into(),
        "mcp__workbench-mcp__search".into(),
    ];

    // Researcher: read-only, Edit and Write denied
    let researcher = SpecializedRole::researcher();
    let filtered = apply_tool_policy(&researcher, &base_tools);
    assert!(!filtered.contains(&"Edit".to_string()));
    assert!(!filtered.contains(&"Write".to_string()));
    assert!(is_tool_allowed(&researcher, "Read"));
    assert!(!is_tool_allowed(&researcher, "Edit"));

    // Coder: all tools allowed
    let coder = SpecializedRole::coder();
    let filtered = apply_tool_policy(&coder, &base_tools);
    assert_eq!(filtered.len(), 5);

    // Reviewer: read-only, same as researcher
    let reviewer = SpecializedRole::reviewer();
    assert!(!is_tool_allowed(&reviewer, "Write"));
}

#[test]
fn role_dispatch_matches_subtask_types() {
    // Phase-based subtasks from partition_by_phase
    assert_eq!(
        classify_subtask_role("phase-research", "Research phase"),
        "researcher"
    );
    assert_eq!(
        classify_subtask_role("phase-implement", "Implementation phase"),
        "coder"
    );
    assert_eq!(
        classify_subtask_role("phase-test", "Testing phase"),
        "tester"
    );

    // With available roles
    let roles = SpecializedRole::built_in_roles();
    let role = dispatch_to_role("phase-research", "Research the codebase", &roles);
    assert_eq!(role.unwrap().role_id, "researcher");
}

#[test]
fn role_model_routing() {
    let researcher = SpecializedRole::researcher();
    assert_eq!(
        researcher.effective_model(),
        Some("claude-haiku-4-5-20251001")
    );

    let coder = SpecializedRole::coder();
    assert_eq!(coder.effective_model(), Some("claude-opus-4-6"));

    let reviewer = SpecializedRole::reviewer();
    assert_eq!(reviewer.effective_model(), Some("claude-sonnet-4-6"));
}

#[test]
fn role_recommend_end_to_end() {
    // Bug fix → coder + tester
    let roles = recommend_roles_for_task("fix the authentication bug");
    assert!(roles.contains(&"coder"));
    assert!(roles.contains(&"tester"));

    // New feature → researcher + coder + tester
    let roles = recommend_roles_for_task("implement new payment system");
    assert!(roles.contains(&"researcher"));
    assert!(roles.contains(&"coder"));
    assert!(roles.contains(&"tester"));

    // Refactor → coder + reviewer + tester
    let roles = recommend_roles_for_task("refactor the database layer");
    assert!(roles.contains(&"coder"));
    assert!(roles.contains(&"reviewer"));
    assert!(roles.contains(&"tester"));
}

#[test]
fn supervisor_with_custom_roles() {
    let mut roles = SpecializedRole::built_in_roles();
    // Add a custom role
    roles.push(SpecializedRole {
        role_id: "security-auditor".into(),
        display_name: "Security Auditor".into(),
        system_prompt_template: "Audit for security issues.\n\nTask: {task}".into(),
        allowed_tools: vec!["Read".into()],
        denied_tools: vec!["Edit".into(), "Write".into()],
        model_preference: Some("claude-opus-4-6".into()),
        model_fallback: vec![],
        constraints: magistr::agent::multi::role::RoleConstraints {
            read_only: true,
            max_iterations: 5,
        },
        icon: '\u{1f6e1}',
        color_index: 5,
        archetype_id: None,
    });

    let config = SupervisorConfig::new("/tmp/project");
    let supervisor = magistr::agent::multi::supervisor::Supervisor::with_roles(config, roles);
    assert_eq!(supervisor.available_roles().len(), 6);
    assert!(
        supervisor
            .available_roles()
            .iter()
            .any(|r| r.role_id == "security-auditor")
    );
}

#[test]
fn topology_tracks_specialized_roles() {
    let mut topo = AgentTopology::new();

    // Add regular worker
    topo.add(
        AgentId::new(1),
        AgentRole::Worker {
            task_id: "t1".into(),
        },
    );
    assert!(
        topo.get(AgentId::new(1))
            .unwrap()
            .specialized_role
            .is_none()
    );

    // Add specialized worker
    topo.add_with_specialization(
        AgentId::new(2),
        AgentRole::Worker {
            task_id: "t2".into(),
        },
        "researcher".into(),
    );
    assert_eq!(
        topo.get(AgentId::new(2))
            .unwrap()
            .specialized_role
            .as_deref(),
        Some("researcher"),
    );
}

#[test]
fn custom_role_from_toml_loading() {
    use std::io::Write;
    let dir = tempfile::TempDir::new().unwrap();
    let agents_dir = dir.path().join(".magistr").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    let mut f = std::fs::File::create(agents_dir.join("perf-analyst.toml")).unwrap();
    f.write_all(
        b"role_id = \"perf-analyst\"\n\
display_name = \"Performance Analyst\"\n\
system_prompt_template = \"Analyze performance.\\n\\nTask: {task}\"\n\
denied_tools = [\"Edit\", \"Write\"]\n\
icon = \"P\"\n\
\n\
[constraints]\n\
read_only = true\n\
\n\
[model]\n\
preference = \"claude-sonnet-4-6\"\n",
    )
    .unwrap();

    let all = magistr::agent::multi::role_loader::load_all_roles(dir.path());
    assert_eq!(all.len(), 6); // 5 built-in + 1 custom
    let perf = all.iter().find(|r| r.role_id == "perf-analyst").unwrap();
    assert!(perf.constraints.read_only);
    assert_eq!(perf.display_name, "Performance Analyst");
}
