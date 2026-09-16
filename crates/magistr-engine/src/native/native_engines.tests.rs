use super::*;
use crate::native::{
    EvidencePort, GateContract, NativeConfig, NativeReader, NativeSnapshot, ProjectAuthority,
    SourceFreshness, SourceObservation, SourceWitness, WriterRole,
};
use magistr_workflow::roadmap::domain::{BlockerKind, ChunkStatus};
use magistr_workflow::ship::domain::action::ActionType;
use magistr_workflow::ship::domain::check::{Check, CheckType};
use magistr_workflow::ship::domain::task::TaskType;
use magistr_workflow::signal::domain::SignalKind;
use magistr_workflow::think::domain::ThinkStep;

/// A source that never moves and a project that configures no gates: the
/// evidence capability every writer here is admitted with, so a recorded
/// gate certifies a source and a chunk can complete on it.
struct Fixed;

impl SourceWitness for Fixed {
    fn observation(&self) -> SourceObservation {
        SourceObservation {
            workspace_root: "/w".into(),
            workspace_generation: None,
            scheme: "fake/1".into(),
            policy_digest: "p".into(),
            revision: "r1".into(),
            consistency: "optimistic".into(),
            captured_at_ms: 0,
        }
    }
    fn validate(&self) -> Result<SourceFreshness, String> {
        Ok(SourceFreshness::Unchanged)
    }
}

struct Evidence;

impl EvidencePort for Evidence {
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        Ok(Box::new(Fixed))
    }
    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        Ok(Vec::new())
    }
}

fn temp_config(tmp: &tempfile::TempDir) -> NativeConfig {
    NativeConfig::from_lookup(tmp.path().join("proj"), |_| None)
        .with_data_dir(tmp.path().join("data"))
        .with_evidence(Arc::new(Evidence))
}

/// The host's admitted writer on `cfg`.
fn host(cfg: &NativeConfig, project_id: &str) -> NativeEngines {
    ProjectAuthority::admit_for_project(cfg, project_id.into(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

/// The agent's `magistr mcp-serve` on the same store: the other admitted
/// writer role, holding its own cached engines.
fn service(cfg: &NativeConfig, project_id: &str) -> NativeEngines {
    ProjectAuthority::admit_for_project(cfg, project_id.into(), WriterRole::Service)
        .expect("service admitted")
        .open()
}

/// An observer on the store, admitted by nobody.
fn reader(cfg: &NativeConfig, project_id: &str) -> NativeReader {
    NativeReader::open_for_project(cfg, project_id.into())
}

fn add_chunk(engines: &NativeEngines, id: &str, priority: u32, deps: &[&str]) {
    engines
        .roadmap_add_chunk(
            id.into(),
            format!("Chunk {id}"),
            String::new(),
            ChunkStatus::Pending,
            priority,
            String::new(),
            vec![],
            deps.iter().map(|d| (*d).to_string()).collect(),
            None,
            None,
        )
        .expect("add chunk");
}

fn step(n: u32, purpose: &str) -> ThinkStep {
    serde_json::from_value(serde_json::json!({
        "step_number": n,
        "estimated_total": 3,
        "purpose": purpose,
        "context": "in-process test",
        "thought": "Reasoning body.",
        "outcome": "Recorded.",
        "next_action": "Continue.",
        "rationale": "Because the test says so.",
    }))
    .expect("step")
}

#[test]
fn branch_lifecycle_and_exact_merge_destination_survive_reader_restart() {
    use crate::native::BranchStatus;

    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let project = "reasoning-restart";
    let observer = reader(&cfg, project);
    assert!(observer.snapshot().branches.is_empty());
    {
        let engines = host(&cfg, project);
        engines
            .think_record_step(step(1, "Keep recovery isolated"))
            .expect("origin");
        let mut branch = step(2, "Can each tenant keep its own receipt?");
        branch.branch_id = Some("receipt".into());
        branch.branch_name = Some("Can each tenant keep its own receipt?".into());
        branch.branch_from = Some(1);
        branch.outcome = "Keep one receipt per tenant.".into();
        engines.think_record_step(branch).expect("branch");
        engines
            .think_record_step(step(3, "Check the log format"))
            .expect("unrelated step");
        engines
            .think_record_step(step(4, "Use tenant receipts during recovery"))
            .expect("synthesis");
        engines
            .services
            .think
            .set_branch_status("receipt", "merged", Some(4))
            .expect("merge");
        let snapshot = observer.snapshot();
        let receipt = snapshot
            .branches
            .iter()
            .find(|branch| branch.id == "receipt")
            .expect("persisted branch");
        assert_eq!(receipt.name, "Can each tenant keep its own receipt?");
        assert_eq!(receipt.from_step, Some(1));
        assert_eq!(receipt.status, BranchStatus::Merged);
        assert_eq!(receipt.merged_into, Some(4));
        assert_eq!(snapshot.steps[1].branch_id.as_deref(), Some("receipt"));
        assert_eq!(snapshot.steps[1].outcome, "Keep one receipt per tenant.");
    }

    let restarted = reader(&cfg, project);
    let persisted = restarted.snapshot();
    let receipt = persisted
        .branches
        .iter()
        .find(|branch| branch.id == "receipt")
        .expect("branch after restart");
    assert_eq!(receipt.status, BranchStatus::Merged);
    assert_eq!(
        receipt.merged_into,
        Some(4),
        "do not substitute the first later main-line step"
    );

    {
        let engines = host(&cfg, project);
        engines
            .services
            .think
            .set_branch_status("receipt", "abandoned", None)
            .expect("stop investigation after restart");
    }
    let stopped = restarted.snapshot();
    let receipt = stopped
        .branches
        .iter()
        .find(|branch| branch.id == "receipt")
        .expect("stopped branch");
    assert_eq!(receipt.status, BranchStatus::Abandoned);
    assert_eq!(
        receipt.merged_into, None,
        "reseed clears obsolete merge provenance"
    );
}

#[test]
fn round_trip_through_a_temp_store() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);

    {
        let engines = host(&cfg, "round-trip");
        assert_eq!(engines.project_id(), "round-trip");
        add_chunk(&engines, "base", 100, &[]);
        add_chunk(&engines, "next", 200, &["base"]);
        let started = engines.roadmap_start_chunk("base").expect("start");
        assert_eq!(started.status, ChunkStatus::InProgress);

        assert!(engines.think_record_step(step(1, "open")).is_ok());
        // Returns the prior pinned state; a fresh step was not pinned.
        assert_eq!(engines.think_pin_step(1, true), Ok(false));

        assert_eq!(
            engines
                .ship_set_objective("obj".into(), vec![], vec![], "chunk:base".into())
                .expect("objective"),
            0
        );
        engines
            .ship_add_task("t1".into(), "Task one".into(), TaskType::Implement, None)
            .expect("add task");
        let t = engines.ship_start_task("t1").expect("start task");
        assert_eq!(t.id, "t1");
        let action = engines
            .ship_record_action(
                None,
                ActionType::Code,
                "wrote code".into(),
                vec!["a.rs".into()],
                vec![],
                Some(1),
            )
            .expect("action");
        assert_eq!(action.think_step, Some(1));
        let check = engines
            .ship_record_gate(
                None,
                "test",
                "cargo nextest".into(),
                0,
                String::new(),
                true,
                None,
            )
            .expect("gate");
        assert!(check.passed && check.verified);
        assert_eq!(check.exit_code, Some(0));
        let failed = engines
            .ship_record_gate(None, "lint", "clippy".into(), 1, String::new(), true, None)
            .expect("gate");
        assert!(!failed.passed && failed.verified);
        // The red lint is current evidence against completion until it
        // is re-run green; then it is a recovered failure, not an
        // outstanding one.
        let refused = engines
            .roadmap_complete_chunk("base", Some("task:t1"))
            .expect_err("a failed required gate refuses completion");
        assert!(
            refused.contains("lint: the latest attempt did not pass (exit 1)"),
            "{refused}"
        );
        engines
            .ship_record_gate(None, "lint", "clippy".into(), 0, String::new(), true, None)
            .expect("gate");
        engines.ship_complete_task("t1", vec![]).expect("complete");

        let sig = engines
            .signal_capture(SignalKind::Idea, "alice".into(), "make it faster".into())
            .expect("capture");
        assert_eq!(sig.body, "make it faster");
        let done = engines
            .roadmap_complete_chunk("base", Some("task:t1"))
            .expect("complete chunk");
        assert_eq!(done.status, ChunkStatus::Done);
        let report = engines
            .ship_finalize(vec![], Some("done".into()))
            .expect("finalize");
        assert_eq!(report.status, "shipped");
        assert_eq!(report.recovered_failures, vec!["lint (task: t1)"]);
    }

    // A second process on the same store sees everything.
    let reopened = reader(&cfg, "round-trip");
    let snap = reopened.roadmap_snapshot();
    assert_eq!(snap.count("done"), 1);
    assert_eq!(snap.next_ready.as_deref(), Some("next"));
    assert_eq!(reopened.roadmap_progress().checked_tasks, 1);
    assert_eq!(
        reopened.roadmap_chunk("base").map(|c| c.cross_refs),
        Some(vec!["task:t1".to_string()])
    );
    let pinned = reopened.think_pinned_steps().len();
    assert_eq!(pinned, 1, "think trace persisted with its pin");
    assert_eq!(reopened.ship_status()["objective"]["status"], "completed");
    assert_eq!(reopened.signal_status()["counts"]["new"], 1);

    // Every family wrote under its own partition of the one data dir.
    for family in ["roadmap", "think", "ship", "signal"] {
        let dir = tmp.path().join("data").join(family).join("sessions");
        assert!(
            dir.join("round-trip.json").exists(),
            "{family} store missing"
        );
    }
}

#[test]
fn two_writers_merge_without_data_loss() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);

    // A = Magistr in-process, B = an external think-and-ship server, both
    // holding the same (empty) store in memory.
    let a = host(&cfg, "shared");
    let b = service(&cfg, "shared");

    add_chunk(&a, "from-a", 100, &[]);
    add_chunk(&b, "from-b", 200, &[]);
    // B wrote last from stale memory (it never saw from-a). A plain
    // overwrite would have erased from-a; the locked merge keeps both.
    let c = reader(&cfg, "shared");
    let ids: Vec<String> = c.roadmap_chunks().into_iter().map(|c| c.id).collect();
    assert!(
        ids.contains(&"from-a".to_string()),
        "A's chunk survived: {ids:?}"
    );
    assert!(
        ids.contains(&"from-b".to_string()),
        "B's chunk survived: {ids:?}"
    );

    // The merge lands on disk and in the writer's memory: B's own save
    // handed it the merged file, so B answers A's chunk without a reload,
    // and A's next query observes B's chunk the same way.
    assert!(
        b.roadmap_chunk("from-a").is_some(),
        "B's memory is the merged file after its own write"
    );
    assert!(
        a.roadmap_chunk("from-b").is_some(),
        "A's query re-read the moved store"
    );
    b.roadmap_start_chunk("from-b").expect("start");
    assert_eq!(
        a.roadmap_chunk("from-b").map(|c| c.status),
        Some(ChunkStatus::InProgress),
        "A observes B's transition on its next query"
    );
    let d = reader(&cfg, "shared");
    assert_eq!(d.roadmap_snapshot().count("in_progress"), 1);
    assert_eq!(d.roadmap_snapshot().count("pending"), 1);
    // A snapshot read on the writer that never reloaded is the current
    // store too, family by family.
    let snapshot = a.snapshot();
    assert_eq!(snapshot.chunks.len(), 2);
}

#[test]
fn open_resolves_identity_from_the_project_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("ident");
    std::fs::create_dir_all(&root).expect("mkdir");
    let cfg = NativeConfig::from_lookup(&root, |_| None).with_data_dir(tmp.path().join("data"));
    let declared = super::super::identity::ensure_project_file(&root).expect("identity");
    let engines = ProjectAuthority::admit(&cfg, WriterRole::Host)
        .expect("admitted")
        .open();
    assert_eq!(engines.project_id(), declared);
    assert_eq!(engines.authority().project_id(), declared);
    assert_eq!(NativeReader::open(&cfg).project_id(), declared);
}

#[test]
fn persistence_off_writes_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp).persist(false);
    let engines = host(&cfg, "ephemeral");
    add_chunk(&engines, "a", 1, &[]);
    assert!(!tmp.path().join("data").join("roadmap").exists());
    let again = reader(&cfg, "ephemeral");
    assert!(again.roadmap_chunks().is_empty());
}

#[test]
fn roadmap_mutation_wrappers_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "mutations");

    let added = engines
        .roadmap_add_chunk(
            "auth".into(),
            "Authenticate users".into(),
            String::new(),
            ChunkStatus::Pending,
            200,
            "desc".into(),
            vec!["logs in".into()],
            vec![],
            None,
            Some("Identity".into()),
        )
        .expect("add");
    assert_eq!(added.group.as_deref(), Some("Identity"));
    assert!(!added.name.is_empty(), "name is derived from the id");

    let updated = engines
        .roadmap_update_chunk(
            "auth",
            Some("Authenticate every user".into()),
            Some(100),
            None,
            Some(vec!["logs in".into(), "logs out".into()]),
            None,
            None,
        )
        .expect("update");
    assert_eq!(updated.title, "Authenticate every user");
    assert_eq!(updated.priority, 100);
    assert_eq!(updated.acceptance.len(), 2);

    let proposed = engines
        .roadmap_reprioritize("auth", 300, "less urgent".into())
        .expect("propose");
    assert_eq!(proposed.priority, 100, "a proposal never reorders");
    assert_eq!(
        proposed.reprioritize.as_ref().map(|p| p.suggested_priority),
        Some(300)
    );

    let linked = engines.roadmap_link("auth", "think:7").expect("link");
    assert!(linked.cross_refs.contains(&"think:7".to_string()));

    let regrouped = engines
        .roadmap_set_group("auth", Some("Accounts".into()))
        .expect("set group");
    assert_eq!(regrouped.group.as_deref(), Some("Accounts"));

    let backlogged = engines
        .roadmap_set_status("auth", ChunkStatus::Backlog)
        .expect("set status");
    assert_eq!(backlogged.status, ChunkStatus::Backlog);
    let refused = engines
        .roadmap_set_status("auth", ChunkStatus::Done)
        .expect_err("done is never set by name");
    assert!(refused.contains("roadmap_complete_chunk"), "{refused}");

    let gone = engines
        .roadmap_obsolete_chunk("auth", "superseded".into())
        .expect("obsolete");
    assert_eq!(gone.status, ChunkStatus::Obsoleted);
    assert_eq!(gone.obsoleted_reason.as_deref(), Some("superseded"));

    assert!(engines.roadmap_link("missing", "think:1").is_err());
}

#[test]
fn reload_picks_up_another_writers_changes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let host = reader(&cfg, "reload");
    let server = service(&cfg, "reload");
    assert_eq!(host.config(), &cfg);

    add_chunk(&server, "only-on-disk", 100, &[]);
    server
        .ship_set_objective("obj".into(), vec![], vec![], "chunk:only-on-disk".into())
        .expect("objective");
    server
        .ship_add_task("t1".into(), "task".into(), TaskType::Implement, None)
        .expect("add task");

    // The host's in-memory copies predate those writes; its queries read
    // through, and an explicit reload is the same current answer.
    assert_eq!(
        host.roadmap_next().map(|c| c.id).as_deref(),
        Some("only-on-disk")
    );
    assert_eq!(host.ship_status()["tasks"]["total"], 1);

    host.reload_roadmap();
    host.reload_ship();
    assert_eq!(
        host.roadmap_next().map(|c| c.id).as_deref(),
        Some("only-on-disk")
    );
    assert_eq!(host.ship_status()["tasks"]["total"], 1);
    assert_eq!(
        host.ship_status()["objective"]["scope"],
        "chunk:only-on-disk"
    );
}

#[test]
fn think_steps_from_store_sees_another_writers_steps_without_reopening() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let host = reader(&cfg, "think-store");
    assert!(host.think_steps_from_store().is_empty());

    // The agent's server records after the host opened.
    let server = service(&cfg, "think-store");
    assert!(server.think_record_step(step(1, "open")).is_ok());
    assert!(server.think_record_step(step(2, "ground")).is_ok());

    let numbers: Vec<u32> = host
        .think_steps_from_store()
        .iter()
        .map(|s| s.step_number)
        .collect();
    assert_eq!(numbers, vec![1, 2]);
}

#[test]
fn think_steps_from_store_is_empty_without_persistence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp).persist(false);
    let engines = host(&cfg, "ephemeral-think");
    assert!(engines.think_record_step(step(1, "open")).is_ok());
    assert!(engines.think_steps_from_store().is_empty());
}

#[test]
fn signal_triage_wrappers_round_trip() {
    use magistr_workflow::signal::domain::SignalStatus;
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "triage");

    let a = engines
        .signal_capture(SignalKind::Idea, "alice".into(), "faster".into())
        .expect("capture");
    let b = engines
        .signal_capture(SignalKind::Bug, "bob".into(), "broken".into())
        .expect("capture");
    let c = engines
        .signal_capture(SignalKind::Question, "cara".into(), "why".into())
        .expect("capture");
    let ids: Vec<String> = engines.signal_list().into_iter().map(|s| s.id).collect();
    assert!(ids.contains(&a.id) && ids.contains(&b.id) && ids.contains(&c.id));

    // Promote needs a researched signal; a fresh one is refused with a reason.
    assert!(engines.signal_promote(&a.id).is_err());
    let researched = engines
        .signal_research(&a.id, "validated".into(), 0.9, vec![], None)
        .expect("research");
    assert_eq!(researched.status, SignalStatus::Researched);
    assert!(
        engines
            .signal_research("missing", "x".into(), 0.5, vec![], None)
            .is_err()
    );
    let promoted = engines.signal_promote(&a.id).expect("promote");
    assert_eq!(promoted.status, SignalStatus::Promoted);
    // The reciprocal half the MCP verb has always done: promotion creates
    // the backlog chunk and links the two both ways.
    let chunk_id = format!("signal-{}", a.id);
    assert!(
        promoted.cross_refs.contains(&format!("chunk:{chunk_id}")),
        "{:?}",
        promoted.cross_refs
    );
    let chunk = engines
        .roadmap_chunk(&chunk_id)
        .expect("promotion created the backlog chunk");
    assert_eq!(chunk.status, ChunkStatus::Backlog);
    assert_eq!(chunk.title, "[idea] faster");
    assert!(chunk.cross_refs.contains(&format!("signal:{}", a.id)));
    // Promoting again is idempotent: the same chunk, still one of it.
    let again = engines.signal_promote(&a.id).expect("promote again");
    assert_eq!(again.status, SignalStatus::Promoted);
    assert_eq!(
        engines
            .roadmap_chunks()
            .iter()
            .filter(|c| c.id == chunk_id)
            .count(),
        1
    );

    let dismissed = engines.signal_ignore(&b.id).expect("ignore");
    assert_eq!(dismissed.status, SignalStatus::Dismissed);

    let snoozed = engines.signal_snooze(&c.id, 30).expect("snooze");
    assert!(snoozed.snooze_until.is_some(), "snooze stamped");
    assert_eq!(snoozed.status, SignalStatus::New, "snooze keeps status");

    assert!(engines.signal_promote("missing").is_err());
}

#[test]
fn proposal_resolution_wrappers_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "proposals");
    add_chunk(&engines, "a", 100, &[]);

    // Title: accept adopts the suggestion, reject keeps the plan's title.
    let proposed = engines
        .roadmap_propose_title("a", "Better title".into(), "why".into(), "tracker".into())
        .expect("propose title");
    assert_eq!(proposed.title, "Chunk a", "a proposal never renames");
    assert!(proposed.title_proposal.is_some());
    let rejected = engines
        .roadmap_resolve_title_proposal("a", false)
        .expect("reject title");
    assert_eq!(rejected.title, "Chunk a");
    assert!(rejected.title_proposal.is_none());
    engines
        .roadmap_propose_title("a", "Better title".into(), "why".into(), "tracker".into())
        .expect("propose title again");
    let accepted = engines
        .roadmap_resolve_title_proposal("a", true)
        .expect("accept title");
    assert_eq!(accepted.title, "Better title");
    assert!(engines.roadmap_resolve_title_proposal("a", true).is_err());

    // Status: accept takes the suggested transition, reject keeps the
    // current status — either act disposes the proposal.
    let proposed = engines
        .roadmap_propose_status(
            "a",
            ChunkStatus::InProgress,
            "started remotely".into(),
            "tracker".into(),
        )
        .expect("propose status");
    assert_eq!(
        proposed.status,
        ChunkStatus::Pending,
        "a proposal never moves"
    );
    let kept = engines
        .roadmap_resolve_status_proposal("a", false)
        .expect("reject status");
    assert_eq!(kept.status, ChunkStatus::Pending);
    assert!(kept.status_proposal.is_none());
    engines
        .roadmap_propose_status(
            "a",
            ChunkStatus::InProgress,
            "started remotely".into(),
            "tracker".into(),
        )
        .expect("propose status again");
    let moved = engines
        .roadmap_resolve_status_proposal("a", true)
        .expect("accept status");
    assert_eq!(moved.status, ChunkStatus::InProgress);
    assert!(moved.status_proposal.is_none());
    assert!(engines.roadmap_resolve_status_proposal("a", true).is_err());

    // Reprioritize: accept takes the suggested priority, reject keeps the
    // current one; the proposal is disposed either way — including when
    // the suggestion equals the current priority.
    engines
        .roadmap_reprioritize("a", 300, "later".into())
        .expect("propose reprioritize");
    let kept = engines
        .roadmap_resolve_reprioritize("a", false)
        .expect("reject reprioritize");
    assert_eq!(kept.priority, 100);
    assert!(kept.reprioritize.is_none());
    engines
        .roadmap_reprioritize("a", 300, "later".into())
        .expect("propose reprioritize again");
    let moved = engines
        .roadmap_resolve_reprioritize("a", true)
        .expect("accept reprioritize");
    assert_eq!(moved.priority, 300);
    assert!(moved.reprioritize.is_none());
    engines
        .roadmap_reprioritize("a", 300, "no-op suggestion".into())
        .expect("propose the current priority");
    let same = engines
        .roadmap_resolve_reprioritize("a", true)
        .expect("accept the no-op");
    assert_eq!(same.priority, 300);
    assert!(same.reprioritize.is_none(), "disposed even when identical");
    assert!(engines.roadmap_resolve_reprioritize("a", true).is_err());
    assert!(
        engines
            .roadmap_resolve_reprioritize("missing", true)
            .is_err()
    );
}

#[test]
fn think_pin_and_checkpoint_act_on_the_live_trace() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp).persist(false);
    let engines = host(&cfg, "pin-checkpoint");
    assert!(engines.think_record_step(step(1, "hypothesis")).is_ok());

    assert_eq!(
        engines.think_pin_step(1, true),
        Ok(false),
        "the previous flag comes back"
    );
    let pinned = engines
        .think_step(1)
        .and_then(|step| step.pinned)
        .unwrap_or(false);
    assert!(pinned, "the flag stuck");
    assert!(engines.think_pin_step(9, true).is_err(), "unknown step");

    let report = engines.think_checkpoint().expect("checkpoint");
    assert!(
        report.get("open_hypotheses").is_some(),
        "the checkpoint report has its sections: {report}"
    );
}

#[test]
fn the_snapshot_cursor_is_none_without_persistence_and_moves_when_a_writer_lands() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);

    let engines = host(&cfg, "stamped");
    // Nothing written yet: no file, no cursor.
    assert_eq!(engines.snapshot_cursor(), None);

    add_chunk(&engines, "base", 100, &[]);
    let first = engines
        .snapshot_cursor()
        .expect("the roadmap file exists now");

    // A second process writing the same store moves the cursor.
    let other = service(&cfg, "stamped");
    add_chunk(&other, "next", 200, &["base"]);
    assert_ne!(engines.snapshot_cursor(), Some(first), "the cursor moved");

    // And a store with persistence off never has one at all.
    let off = reader(
        &NativeConfig::from_lookup(tmp.path().join("proj"), |_| None)
            .with_data_dir(tmp.path().join("data"))
            .persist(false),
        "quiet",
    );
    assert_eq!(off.snapshot_cursor(), None);
}

/// The snapshot is the store whole, typed: what the cockpit seeds from
/// and what a re-read after a gap restores. Each family comes back as the
/// engine holds it — the ship half with actions and checks on their
/// tasks — and the cursor is the store's, taken first.
#[test]
fn a_snapshot_reads_every_family_whole_with_its_cursor() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "snapshotted");

    let empty = engines.snapshot();
    assert_eq!(
        empty,
        NativeSnapshot::default(),
        "an empty store is an empty snapshot"
    );

    add_chunk(&engines, "base", 100, &[]);
    add_chunk(&engines, "next", 200, &["base"]);
    engines.roadmap_start_chunk("base").expect("start");
    engines
        .ship_set_objective(
            "ship the snapshot".into(),
            vec![],
            vec![],
            "chunk:base".into(),
        )
        .expect("objective");
    engines
        .ship_add_task("t1".into(), "read it".into(), TaskType::Implement, None)
        .expect("task");
    engines.ship_start_task("t1").expect("start task");
    engines
        .ship_record_action(
            Some("t1"),
            ActionType::Code,
            "wrote the seed".into(),
            vec![],
            vec![],
            None,
        )
        .expect("action");
    engines
        .ship_record_gate(
            Some("t1"),
            "cargo nextest",
            "true".into(),
            0,
            String::new(),
            true,
            None,
        )
        .expect("gate");
    engines
        .signal_capture(SignalKind::Idea, "alice".into(), "faster".into())
        .expect("signal");
    assert!(engines.think_record_step(step(1, "open")).is_ok());

    let snapshot = engines.snapshot();
    assert_eq!(
        snapshot.cursor,
        engines.snapshot_cursor(),
        "the cursor is the store's stamp"
    );
    assert_eq!(
        snapshot.objective.as_ref().map(|o| o.scope.as_str()),
        Some("chunk:base")
    );
    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(
        snapshot.tasks[0].actions.len(),
        1,
        "actions ride their task"
    );
    assert_eq!(snapshot.tasks[0].checks.len(), 1, "checks ride their task");
    assert_eq!(
        snapshot.tasks[0].checks[0].exit_code,
        Some(0),
        "the exit code ship_status would have dropped"
    );
    let ids: Vec<&str> = snapshot.chunks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["base", "next"], "stored order");
    assert_eq!(snapshot.chunks[0].status, ChunkStatus::InProgress);
    assert_eq!(snapshot.progress.total_tasks, 2);
    assert_eq!(snapshot.signals.len(), 1);
    assert_eq!(snapshot.steps.len(), 1, "the persisted step");
    assert_eq!(snapshot.steps[0].step_number, 1);

    // A reader opened beside the writer reads the same store to the same
    // snapshot: the projection is a function of the files, not of the
    // process that wrote them.
    let observer = reader(&cfg, "snapshotted");
    assert_eq!(observer.snapshot(), snapshot);
}

#[test]
fn a_blocker_is_cleared_through_the_wrapper_and_erring_is_loud() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "blockers");
    add_chunk(&engines, "stuck", 100, &[]);

    // Nothing to clear yet: the library errs rather than answer a no-op,
    // and the wrapper passes that through — a caller who believes there
    // is a blocker is working from a stale picture.
    assert!(engines.roadmap_clear_blocked_by("stuck").is_err());

    // A blocker the dock would raise a card for, set the way the MCP
    // verb sets it (validate, then store).
    engines
        .roadmap_set_blocked_by(
            "stuck",
            BlockerKind::AwaitingHuman,
            "needs a person to pick the palette".into(),
            None,
        )
        .expect("set");
    assert!(
        engines
            .roadmap_chunk("stuck")
            .expect("chunk")
            .blocked_by
            .is_some()
    );

    let cleared = engines.roadmap_clear_blocked_by("stuck").expect("clear");
    assert!(cleared.blocked_by.is_none(), "the card's blocker is lifted");
    assert!(engines.roadmap_clear_blocked_by("stuck").is_err());
    assert!(engines.roadmap_clear_blocked_by("missing").is_err());
}

#[test]
fn a_blocker_is_set_through_the_wrapper_the_way_the_verb_sets_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "blockers-set");
    add_chunk(&engines, "stuck", 100, &[]);

    let blocked = engines
        .roadmap_set_blocked_by(
            "stuck",
            BlockerKind::AwaitingHuman,
            "the judge says the acceptance cannot be met as written".into(),
            Some("think:345".into()),
        )
        .expect("set");
    let blocker = blocked.blocked_by.expect("the blocker is on the chunk");
    assert_eq!(blocker.kind, BlockerKind::AwaitingHuman);
    assert!(blocker.reason.contains("cannot be met"));
    assert_eq!(blocker.evidence.as_deref(), Some("think:345"));
    // Stored, not only answered: a fresh read sees it, and the clear
    // wrapper lifts exactly this.
    assert!(
        engines
            .roadmap_chunk("stuck")
            .expect("chunk")
            .blocked_by
            .is_some()
    );
    assert!(engines.roadmap_clear_blocked_by("stuck").is_ok());

    // An unknown chunk and an empty reason are refused, not stored.
    assert!(
        engines
            .roadmap_set_blocked_by("missing", BlockerKind::AwaitingHuman, "why".into(), None)
            .is_err()
    );
    assert!(
        engines
            .roadmap_set_blocked_by("stuck", BlockerKind::AwaitingHuman, String::new(), None)
            .is_err(),
        "a blocker needs a reason"
    );
}

#[test]
fn reload_signal_picks_up_another_writers_capture() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let host = reader(&cfg, "reload-signal");
    let server = service(&cfg, "reload-signal");

    // The agent's `magistr mcp-serve` captures while the host is open.
    let captured = server
        .signal_capture(SignalKind::Idea, "alice".into(), "faster".into())
        .expect("capture");

    // The host's in-memory copy predates it; its queries read through.
    assert_eq!(host.signal_list().len(), 1);
    assert_eq!(host.signal_status()["counts"]["total"], 1);

    host.reload_signal();
    let ids: Vec<String> = host.signal_list().into_iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![captured.id]);
    assert_eq!(host.signal_status()["counts"]["total"], 1);
}

/// A ship trace with one active task, ready to take a gate record.
fn engines_with_active_task(tmp: &tempfile::TempDir) -> NativeEngines {
    let engines = host(&temp_config(tmp), "gate-rec");
    engines
        .ship_set_objective("record the gates".into(), vec![], vec![], String::new())
        .expect("objective");
    engines
        .ship_add_task("t1".into(), "run gates".into(), TaskType::Implement, None)
        .expect("add task");
    engines.ship_start_task("t1").expect("start");
    engines
}

fn checks(engines: &NativeEngines) -> Vec<Check> {
    engines
        .ship_tasks()
        .into_iter()
        .flat_map(|t| t.checks)
        .collect()
}

#[test]
fn a_recorded_gate_is_verified_and_carries_the_command_and_the_real_exit_code() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines_with_active_task(&tmp);

    engines
        .ship_record_gate(
            None,
            "lint",
            "cargo clippy --workspace".to_owned(),
            3,
            "error: something".to_owned(),
            true,
            None,
        )
        .expect("record");

    let checks = checks(&engines);
    assert_eq!(checks.len(), 1);
    let check = &checks[0];
    assert!(check.verified, "a gate the loop ran is not self-reported");
    assert!(!check.passed);
    assert_eq!(check.exit_code, Some(3));
    assert_eq!(check.command.as_deref(), Some("cargo clippy --workspace"));
    assert!(check.required);
    assert_eq!(check.details, "error: something");
}

#[test]
fn the_check_type_is_derived_from_the_gate_name_archetype_suffix_and_all() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines_with_active_task(&tmp);
    for name in ["check", "test:app-typescript", "lint", "structure", "test"] {
        engines
            .ship_record_gate(None, name, "true".to_owned(), 0, String::new(), true, None)
            .expect("record");
    }
    let kinds: Vec<CheckType> = checks(&engines)
        .iter()
        .map(|c| c.check_type.clone())
        .collect();
    assert_eq!(
        kinds,
        vec![
            CheckType::Typecheck,
            CheckType::Test,
            CheckType::Lint,
            // A custom gate is a lint until it says otherwise — the same
            // fallback `magistr_gate`'s own mapping uses.
            CheckType::Lint,
            CheckType::Test,
        ]
    );
}

#[test]
fn a_junit_report_is_parsed_into_the_record() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines_with_active_task(&tmp);
    let report = tmp.path().join("junit.xml");
    std::fs::write(
        &report,
        r#"<?xml version="1.0"?>
<testsuites tests="3" failures="1" skipped="1">
  <testsuite name="s" tests="3" failures="1" skipped="1">
    <testcase classname="s" name="ok"/>
    <testcase classname="s" name="bad"><failure message="boom"/></testcase>
    <testcase classname="s" name="gone"><skipped/></testcase>
  </testsuite>
</testsuites>"#,
    )
    .expect("write junit");

    engines
        .ship_record_gate(
            None,
            "test",
            "cargo nextest run".to_owned(),
            100,
            "…".to_owned(),
            true,
            Some(("junit", &report.to_string_lossy())),
        )
        .expect("record");

    let checks = checks(&engines);
    let results = checks[0].results.as_ref().expect("results");
    assert_eq!(results.total, 3);
    assert_eq!(results.failed, 1);
    assert_eq!(results.skipped, 1);
    assert!(
        results.failures.iter().any(|f| f.name.contains("bad")),
        "{:?}",
        results.failures
    );
    let record = checks[0].report.as_ref().expect("report record");
    assert!(record.parsed, "{record:?}");
}

#[test]
fn a_missing_report_is_not_parsed_and_never_a_failure() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines_with_active_task(&tmp);
    let absent = tmp.path().join("never-written.xml");

    let check = engines
        .ship_record_gate(
            None,
            "test",
            "cargo nextest run".to_owned(),
            0,
            String::new(),
            true,
            Some(("junit", &absent.to_string_lossy())),
        )
        .expect("a missing report does not fail the record");

    assert!(check.passed, "the exit code decides, never the report");
    assert!(check.results.is_none());
    assert_eq!(check.report.as_ref().map(|r| r.parsed), Some(false));
}

#[test]
fn a_gate_cannot_be_recorded_with_no_task_to_record_it_against() {
    // The loop needs the refusal to be legible: it prints it on the stage
    // rather than silently recording nothing.
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = host(&temp_config(&tmp), "no-task");
    let refusal = engines
        .ship_record_gate(
            None,
            "check",
            "true".to_owned(),
            0,
            String::new(),
            true,
            None,
        )
        .expect_err("no active task");
    assert!(!refusal.is_empty(), "the refusal says something");
}

/// The verb's own refusals reach the caller in the handler's words: what
/// the wire refuses, the in-process writer refuses identically, because it
/// is the same handler.
#[test]
fn the_handlers_refusals_are_the_writers_refusals() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp).persist(false);
    let engines = host(&cfg, "refusals");

    let empty = engines
        .signal_capture(SignalKind::Idea, "alice".into(), "   ".into())
        .expect_err("an empty body is refused as the verb refuses it");
    assert_eq!(empty, "signal body is empty");

    let captured = engines
        .signal_capture(SignalKind::Idea, "alice".into(), "faster".into())
        .expect("capture");
    let blank = engines
        .signal_research(&captured.id, "  ".into(), 0.5, vec![], None)
        .expect_err("a blank summary is refused");
    assert_eq!(blank, "research summary is empty");

    add_chunk(&engines, "c", 100, &[]);
    let done = engines
        .roadmap_complete_chunk("c", None)
        .expect_err("no objective certifies the chunk");
    assert!(
        done.starts_with("completion refused: no ship objective is set"),
        "{done}"
    );

    let no_task = engines
        .ship_start_task("nothing")
        .expect_err("an unknown task is refused");
    assert!(no_task.contains("nothing"), "{no_task}");
}

/// A fresh objective after a shipped one clears the stale plan — the
/// count the handler reports comes back as the typed answer.
#[test]
fn a_fresh_cycle_reports_how_many_stale_tasks_it_cleared() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp).persist(false);
    let engines = host(&cfg, "fresh-cycle");
    assert_eq!(
        engines
            .ship_set_objective("first".into(), vec![], vec![], String::new())
            .expect("objective"),
        0
    );
    engines
        .ship_add_task("t1".into(), "one".into(), TaskType::Implement, None)
        .expect("add task");
    engines
        .ship_add_task("t2".into(), "two".into(), TaskType::Test, None)
        .expect("add task");
    engines.ship_finalize(vec![], None).expect("finalize");
    assert_eq!(
        engines
            .ship_set_objective("second".into(), vec![], vec![], String::new())
            .expect("objective"),
        2,
        "the finished cycle's two tasks were cleared"
    );
    assert!(engines.ship_tasks().is_empty());
}
