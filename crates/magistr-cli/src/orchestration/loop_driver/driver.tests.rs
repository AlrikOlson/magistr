//! The loop driver end to end: a scripted terminal, an in-process ACP
//! agent behind the connector, a temp yaml project whose gates are `sh`
//! one-liners — the loop's decisions read off the model, the report and
//! what the agent saw.

use std::collections::VecDeque;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use magistr_acp::AcpTransport;
use magistr_cockpit::checkpoint::CheckpointPort;
use magistr_cockpit::cockpit::{DockItem, Model, Msg, Size};
use magistr_cockpit::input::{KeyCode, KeyEvent, Modifiers};
use magistr_cockpit::theme::Icons;
use magistr_cockpit::turn::RowKind;
use magistr_engine::tas::BlockerKind;
use magistr_engine::{ChunkStatus, NativeEngines, SignalKind, TasConfig};
use magistr_tui::graphics::Env;
use magistr_tui::harness::{AcpObserved, FakeAcpAgent, FakeTerminal, Script};
use magistr_tui::runtime::{Options, Runtime};

use super::*;
use crate::agent::Backend;
use crate::error::MagistrError;
use crate::orchestration::goal::{AgentGoal, GoalMode, Judge, JudgeInput, Verdict};
use crate::orchestration::ledger::{SpinePort, session_engine};
use crate::workflow::WorkflowConfig;
use crate::workflow::iteration::IterationLoop;

/// A judge answering from a script, remembering what it was asked.
struct Scripted {
    verdicts: Mutex<VecDeque<Verdict>>,
    asked: Mutex<Vec<JudgeInput>>,
}

impl Scripted {
    fn new(verdicts: impl IntoIterator<Item = Verdict>) -> Arc<Self> {
        Arc::new(Self {
            verdicts: Mutex::new(verdicts.into_iter().collect()),
            asked: Mutex::new(Vec::new()),
        })
    }

    fn asked(&self) -> Vec<JudgeInput> {
        self.asked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Judge for Scripted {
    fn judge<'a>(
        &'a self,
        input: &'a JudgeInput,
    ) -> Pin<Box<dyn Future<Output = Verdict> + Send + 'a>> {
        Box::pin(async move {
            self.asked
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(input.clone());
            self.verdicts
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pop_front()
                .unwrap_or_else(|| Verdict::not_yet("the script ran out"))
        })
    }
}

/// Engines on a temp store with one pending chunk `work`, and the same
/// engines by a second handle for the assertions.
fn engines_with_work(dir: &Path) -> NativeEngines {
    let engines = NativeEngines::open_for_project(&work_config(dir), "loop-goal".to_owned());
    engines
        .roadmap_add_chunk(
            "work".to_owned(),
            "Chunk work".to_owned(),
            "work".to_owned(),
            ChunkStatus::Pending,
            100,
            String::new(),
            vec!["it works".to_owned(), "it is tested".to_owned()],
            vec![],
            None,
            None,
        )
        .expect("add chunk");
    engines
}

/// The store [`engines_with_work`] opens — a second handle on it is what
/// the agent's own `magistr mcp-serve` process is to the loop.
fn work_config(dir: &Path) -> TasConfig {
    TasConfig::from_lookup(dir.to_path_buf(), |_| None).with_data_dir(dir.join("tas-data"))
}

/// The real ledger over `engines` and the project at `dir` — attached to
/// every native loop these tests run, as the entry points attach it, so
/// the boundary checkpoint's cost is paid exactly where the delegated
/// goal's deadlines are measured.
fn ledger(dir: &Path, engines: &NativeEngines) -> Box<dyn CheckpointPort> {
    Box::new(SpinePort::at(session_engine(dir, engines.clone()), dir))
}

/// Make the fixture a git repository with one commit and a clean tree —
/// the loop's own writings (`.magistr/`, the store) ignored — so a
/// boundary checkpoint is a state a fork could reproduce.
fn repository(root: &Path) {
    std::fs::write(root.join(".gitignore"), ".magistr/\ntas-data/\n").expect(".gitignore");
    for args in [
        &["init", "--initial-branch=main"][..],
        &["config", "user.email", "t@t.test"],
        &["config", "user.name", "Test"],
        &["add", "."],
        &["commit", "-m", "one"],
    ] {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Run the cockpit with the driver on it and quit the cockpit when the
/// driver's report is in — behind the driver's last event, since the
/// key goes down the same inbox and the channel is FIFO — so no test
/// carries a quit instant the loop has to beat. A script may still quit
/// earlier, as a user would; the key posted after that is dropped with
/// the loop gone. The ceiling is a hang guard, not a budget.
async fn run_cockpit(runtime: Runtime<FakeTerminal>, driver: LoopDriver) -> (Model, LoopReport) {
    let inbox = runtime.inbox();
    let task = driver.spawn();
    let quit = async move {
        let report = task.await;
        inbox.send(Msg::Key(
            KeyEvent::new(KeyCode::Char('d')).with(Modifiers::CTRL),
        ));
        report
    };
    let (model, report) = tokio::time::timeout(Duration::from_secs(120), async {
        tokio::join!(runtime.run(Model::new(SIZE)), quit)
    })
    .await
    .expect("the loop ends and the cockpit is quit on its report");
    (
        model.expect("the run succeeds"),
        report.expect("the driver did not panic"),
    )
}

const MS: fn(u64) -> Duration = Duration::from_millis;
const SIZE: Size = Size::new(80, 24);

/// A yaml project with one open spec item and the given gate commands.
fn project(check: &str, test: &str, lint: &str) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let magistr = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr).expect("mkdir .magistr");
    let config = serde_json::json!({
        "project": {"name": "fixture", "description": "", "type": "cli"},
        "stack": {
            "language": "rust", "framework": "none", "package_manager": "cargo",
            "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"
        },
        "quality_gate": {"check": check, "test": test, "lint": lint},
        "format": {"command": "true", "extensions": ["rs"]},
        "spec": {"file": "ROADMAP.md", "unchecked_pattern": "^\\s*- \\[ \\]"},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [],
        "state_prefix": "fixture"
    });
    std::fs::write(magistr.join("project.json"), config.to_string()).expect("project.json");
    std::fs::write(dir.path().join("ROADMAP.md"), "- [ ] make it work\n").expect("spec");
    dir
}

/// The connector that hands every connection to a fresh copy of `agent`
/// over a duplex channel, and what each of them observed.
fn fake_connector(agent: FakeAcpAgent) -> (Connector, Arc<Mutex<Vec<Arc<AcpObserved>>>>) {
    let seen: Arc<Mutex<Vec<Arc<AcpObserved>>>> = Arc::new(Mutex::new(Vec::new()));
    let connector = {
        let seen = Arc::clone(&seen);
        Connector::new(move |config| {
            let (client_end, observed, _agent_task) = agent.clone().spawn();
            seen.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(observed);
            async move {
                let spec = ResolvedSessionSpec::resolve_with_runtime(
                    config,
                    &crate::config::user_config::UserConfig::default(),
                    crate::agent::AgentRuntime {
                        path: PathBuf::from("fake-acp-agent"),
                        args: Vec::new(),
                        auth: crate::agent::AuthMethod::Implicit,
                        provider: "scripted".to_owned(),
                    },
                )?;
                let mut transport =
                    AcpTransport::new_with_args(PathBuf::from("fake-acp-agent"), Vec::new(), None)
                        .map_err(|e| MagistrError::Acp(e.to_string()))?
                        .session_model(spec.model().map(str::to_owned));
                transport
                    .connect_over(client_end)
                    .await
                    .map_err(|e| MagistrError::Acp(e.to_string()))?;
                Ok((transport, spec))
            }
        })
    };
    (connector, seen)
}

/// Run the loop over `script` on a cockpit, `max` iterations at most.
async fn run(
    dir: &Path,
    max: u32,
    script: Script,
    agent: FakeAcpAgent,
    auto_pause: bool,
) -> (Model, LoopReport, Vec<String>) {
    run_with(dir, max, script, agent, auto_pause, None, None).await
}

/// [`run_with`] under a delegated goal mode, the echo wait shortened so
/// a mute agent falls back within the test's budget; every agent the
/// connector handed out is returned, since a delegated iteration opens
/// a session per delivery attempt.
async fn run_delegated(
    dir: &Path,
    max: u32,
    script: Script,
    agent: FakeAcpAgent,
    native: NativeEngines,
    judge: Arc<dyn Judge>,
    mode: GoalMode,
) -> (Model, LoopReport, Vec<Arc<AcpObserved>>) {
    let wf = WorkflowConfig::load(dir).expect("the fixture loads");
    let port = ledger(dir, &native);
    let iter_loop = IterationLoop::open(dir, dir.join("ROADMAP.md"), max, Some(native))
        .expect("the loop opens");
    let (connector, seen) = fake_connector(agent);
    let terminal = FakeTerminal::new(SIZE, script);
    let options = Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500));
    let mut runtime = Runtime::new(terminal, options);
    let commands = runtime.commands().expect("the commands, once");
    let driver = LoopDriver::new(
        iter_loop,
        wf,
        Backend::Acp,
        runtime.inbox(),
        commands,
        connector,
    )
    .auto_pause(false)
    .goal(mode, Some(judge), None)
    .goal_echo_timeout(MS(400))
    .checkpoints(port);
    let (model, report) = run_cockpit(runtime, driver).await;
    let agents = seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    (model, report, agents)
}

/// [`run`], driven by think-and-ship engines when `native` is given, and
/// the native goal loop when `judge` is.
async fn run_with(
    dir: &Path,
    max: u32,
    script: Script,
    agent: FakeAcpAgent,
    auto_pause: bool,
    native: Option<NativeEngines>,
    judge: Option<Arc<dyn Judge>>,
) -> (Model, LoopReport, Vec<String>) {
    let (connector, seen) = fake_connector(agent);
    run_over(dir, max, script, connector, seen, auto_pause, native, judge).await
}

/// [`run_with`] over a connector the test built itself — around a
/// [`fake_connector`], with something to do at connect time.
#[allow(clippy::too_many_arguments)] // the test seam takes every collaborator by hand
async fn run_over(
    dir: &Path,
    max: u32,
    script: Script,
    connector: Connector,
    seen: Arc<Mutex<Vec<Arc<AcpObserved>>>>,
    auto_pause: bool,
    native: Option<NativeEngines>,
    judge: Option<Arc<dyn Judge>>,
) -> (Model, LoopReport, Vec<String>) {
    let wf = WorkflowConfig::load(dir).expect("the fixture loads");
    let port = native.as_ref().map(|engines| ledger(dir, engines));
    let iter_loop =
        IterationLoop::open(dir, dir.join("ROADMAP.md"), max, native).expect("the loop opens");
    let terminal = FakeTerminal::new(SIZE, script);
    let options = Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500));
    let mut runtime = Runtime::new(terminal, options);
    let commands = runtime.commands().expect("the commands, once");
    let mut driver = LoopDriver::new(
        iter_loop,
        wf,
        Backend::Acp,
        runtime.inbox(),
        commands,
        connector,
    )
    .auto_pause(auto_pause);
    if let Some(port) = port {
        driver = driver.checkpoints(port);
    }
    if judge.is_some() {
        driver = driver.goal(GoalMode::Native, judge, None);
    }
    let (model, report) = run_cockpit(runtime, driver).await;
    let prompts: Vec<String> = seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .flat_map(|o| o.prompts())
        .collect();
    (model, report, prompts)
}

/// The stage's notes and errors as one line — a long note wraps at the
/// stage's width, so a sentence is looked for here, not in one row.
fn stage_text(model: &Model) -> String {
    notes(model).join(" ")
}

fn notes(model: &Model) -> Vec<String> {
    model
        .turn()
        .rows()
        .iter()
        .filter(|r| matches!(r.kind, RowKind::Note | RowKind::Error))
        .map(|r| r.text.clone())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_policy_change_during_connection_only_applies_to_the_next_session() {
    use magistr_engine::policy::{Charter, PolicyCondition, PolicyEffect, PolicyRule};

    let dir = project("true", "true", "true");
    let (inner, seen) = fake_connector(FakeAcpAgent::new().asking_permission("Bash"));
    let inner = Arc::new(inner);
    let root = dir.path().to_path_buf();
    let connector = Connector::new(move |config| {
        let inner = Arc::clone(&inner);
        let root = root.clone();
        async move {
            let connected = inner.connect(config).await?;
            let mut charter = Charter::default();
            charter.delegate(PolicyRule::new(
                "refuse-bash",
                PolicyCondition::tool("Bash"),
                PolicyEffect::Deny,
            ));
            crate::permissions::CharterStore::at(&root).save(&charter)?;
            Ok(connected)
        }
    });
    let (_, report, _) = run_over(
        dir.path(),
        2,
        Script::new().startup_replies(MS(0)),
        connector,
        Arc::clone(&seen),
        false,
        None,
        None,
    )
    .await;
    assert_eq!(report.iterations(), 2);
    let sessions = seen.lock().unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(
        sessions[0].outcomes(),
        vec![Some("allow".to_owned())],
        "the first session retains its policy captured before connection"
    );
    assert_eq!(
        sessions[1].outcomes(),
        vec![Some("reject".to_owned())],
        "the next session resolves the newly saved charter"
    );
}

#[tokio::test]
async fn closing_sessions_drains_priced_usage_before_the_next_cost_baseline() {
    let dir = project("true", "true", "true");
    let spec = dir.path().join("ROADMAP.md");
    let wf = WorkflowConfig::load(dir.path()).unwrap();
    let mut iter_loop = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    iter_loop.set_budget_limit(Some(2.0)).unwrap();
    iter_loop.observe_budget(1.0);
    let terminal = FakeTerminal::new(SIZE, Script::new());
    let mut runtime = Runtime::new(terminal, Options::new(Env::new("xterm-kitty", false, None)));
    let (connector, _) = fake_connector(FakeAcpAgent::new());
    let mut driver = LoopDriver::new(
        iter_loop,
        wf,
        Backend::Acp,
        runtime.inbox(),
        runtime.commands().unwrap(),
        connector,
    );

    for (cents, expected_total) in [(25, 1.25), (75, 2.0)] {
        let (transport, spec) = driver
            .connector
            .connect(AgentConfig::new("", dir.path()).backend(Backend::Acp))
            .await
            .unwrap();
        let (session, session_tx, _) = driver.open_session(transport, spec, false).unwrap();
        let before = driver.iter_loop.run_state().budget_total_usd;
        // These final updates are queued after the turn consumer has stopped.
        // Repeated cumulative updates from one session must count only once.
        for _ in 0..2 {
            driver
                .tap()
                .send(Msg::Agent(AgentEvent::Usage {
                    used: 100,
                    size: 1000,
                    cost_cents: Some(cents),
                    elapsed_secs: None,
                }))
                .unwrap();
        }
        assert_eq!(driver.iter_loop.run_state().budget_total_usd, before);
        drop(session_tx);
        driver.close_session(session).await.unwrap();
        assert_eq!(
            driver.iter_loop.run_state().budget_total_usd,
            expected_total
        );
    }

    let recovered = IterationLoop::recover(dir.path(), &spec).unwrap();
    assert_eq!(recovered.run_state().budget_total_usd, 2.0);
    assert_eq!(
        recovered.run_state().stop_reason(),
        Some(magistr_engine::workflow::StopReason::BudgetExceeded)
    );
}

#[tokio::test]
async fn cancelled_and_incomplete_turns_cannot_start_verification() {
    for end in [
        TurnEnd::Cancelled,
        TurnEnd::MaxTokens,
        TurnEnd::MaxTurnRequests,
        TurnEnd::Refusal,
        TurnEnd::Other,
    ] {
        let dir = project("true", "true", "true");
        let wf = WorkflowConfig::load(dir.path()).unwrap();
        let iter_loop =
            IterationLoop::with_max(dir.path(), dir.path().join("ROADMAP.md"), 5).unwrap();
        let terminal = FakeTerminal::new(SIZE, Script::new());
        let mut runtime =
            Runtime::new(terminal, Options::new(Env::new("xterm-kitty", false, None)));
        let (connector, _) = fake_connector(FakeAcpAgent::new());
        let mut driver = LoopDriver::new(
            iter_loop,
            wf,
            Backend::Acp,
            runtime.inbox(),
            runtime.commands().unwrap(),
            connector,
        );
        driver
            .tap()
            .send(Msg::Agent(AgentEvent::TurnEnded(end)))
            .unwrap();
        let (session_tx, _session_rx) = unbounded_channel();
        let outcome = driver
            .turn(&session_tx, Instant::now(), &mut ticker())
            .await;
        assert!(
            !matches!(outcome, Turn::Ended),
            "{end:?} cannot certify an ended work attempt"
        );
        let expected = if end == TurnEnd::Cancelled {
            magistr_engine::workflow::StopReason::Cancelled
        } else {
            magistr_engine::workflow::StopReason::Failed
        };
        assert_eq!(driver.iter_loop.run_state().stop_reason(), Some(expected));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_iteration_runs_the_prompt_the_turn_and_the_gates_then_ends_on_the_budget() {
    let dir = project("true", "sh -c 'exit 0'", "false");
    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let (model, report, prompts) = run(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
    )
    .await;
    assert_eq!(report.iterations(), 1);
    assert_eq!(report.end(), &LoopEnd::MaxIterations(1));
    assert_eq!(
        report.to_string().lines().next(),
        Some("loop: 1 iteration · max iterations (1) reached")
    );
    assert_eq!(prompts.len(), 1, "one turn, on a fresh agent");
    assert!(
        prompts[0].contains("make it work"),
        "the prompt was built from the spec: {}",
        &prompts[0][..prompts[0].len().min(400)]
    );
    let status = model.loop_status().expect("the strip saw the loop");
    assert_eq!(status.iteration_text(), "iteration 1/1");
    let leds: Vec<String> = status
        .gates()
        .iter()
        .map(|gate| gate.text(&Icons::UNICODE))
        .collect();
    assert_eq!(leds, ["✓ check", "✓ test", "✗ lint"], "gates ran as jobs");
    assert_eq!(status.gates_passed(), Some(false));
    assert!(!status.is_paused(), "auto-pause was off");
    assert_eq!(status.ended(), Some("max iterations (1) reached"));
    assert_eq!(
        model.jobs_done(),
        4,
        "four job results: three gates, and the red `lint` run once more"
    );
    let notes = notes(&model);
    assert_eq!(notes[0], "iteration 1 of 1 — 0/1 done");
    assert!(
        notes
            .iter()
            .any(|n| n.starts_with("no commit this iteration")),
        "{notes:?}"
    );
    assert!(
        notes.contains(&"error: quality gates failed: lint failed".to_owned()),
        "{notes:?}"
    );
    assert_eq!(
        notes.last().map(String::as_str),
        Some("loop ended: max iterations (1) reached")
    );
    assert!(
        model
            .turn()
            .rows()
            .iter()
            .any(|r| r.text == "did the thing"),
        "the agent's reply is on the stage"
    );
    let mut restored = IterationLoop::recover(dir.path(), dir.path().join("ROADMAP.md"))
        .expect("the terminal state stays recoverable");
    assert_eq!(restored.iteration(), 1);
    assert_eq!(
        restored.pre_iteration_check().unwrap(),
        PreCheckResult::MaxIterations
    );
    assert!(model.quit_requested());
}

/// Every iteration boundary is a state on the spine: the driver posts
/// the boundary once the gates are in, the cockpit mints `cp-N · iter N`
/// wearing the verdicts, and the driver's own port takes it — verified,
/// because the tree is clean — before the next iteration begins, so the
/// answer is on the inbox ahead of the loop's report.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_iteration_boundary_puts_a_verified_checkpoint_on_the_spine() {
    let dir = project("true", "true", "true");
    repository(dir.path());
    let engines = engines_with_work(dir.path());
    let script = Script::new().startup_replies(MS(0));
    let (model, report, _) = run_with(
        dir.path(),
        2,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
        Some(engines),
        None,
    )
    .await;
    assert_eq!(report.iterations(), 2, "{report}");
    let spine = model.rail().spine();
    let rows: Vec<(&str, &str)> = spine
        .entries()
        .iter()
        .map(|cp| (cp.id(), cp.label()))
        .collect();
    assert_eq!(
        rows,
        [("cp-1", "iter 1"), ("cp-2", "iter 2")],
        "one row per boundary, in order"
    );
    for cp in spine.entries() {
        assert_eq!(
            cp.gates(),
            [
                ("check".to_owned(), true),
                ("test".to_owned(), true),
                ("lint".to_owned(), true),
            ],
            "the row wears the iteration's own verdicts"
        );
        assert_eq!(cp.gate_marks(&Icons::UNICODE).as_deref(), Some("✓✓✓"));
        assert!(
            cp.is_verified(),
            "a clean tree is a reproducible state: {:?}",
            cp.state()
        );
    }
    let notes = notes(&model);
    assert!(
        !notes.iter().any(|n| n.contains("checkpoint")),
        "the boundary is a row, never a complaint: {notes:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_report_measures_each_iteration_and_the_tokens_the_agent_put_in_context() {
    let dir = project("true", "sh -c 'sleep 0.2'", "true");
    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let (_, report, _) = run(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().reporting_usage(12_345, 200_000),
        false,
    )
    .await;
    assert_eq!(report.metrics().len(), 1, "one measure per iteration");
    let metrics = &report.metrics()[0];
    assert_eq!(
        metrics.tokens,
        Some(12_345),
        "the level the agent reported is what the turn put in context"
    );
    assert!(metrics.turn > Duration::ZERO, "{metrics}");
    assert!(metrics.gates >= MS(200), "the gates' own time: {metrics}");
    assert!(
        metrics.judge.is_none(),
        "no judge in the plain loop: {metrics}"
    );
    let text = report.to_string();
    assert!(
        text.lines()
            .nth(1)
            .is_some_and(|line| line.starts_with("  iteration 1: turn ")
                && line.ends_with("12.3k tokens in context")),
        "the measure prints under the headline: {text}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pause_holds_the_boundary_and_x_skips_the_next_iteration() {
    let dir = project("true", "true", "true");
    // The agent holds every turn on a permission request the workflow
    // policy cannot decide (an unknown tool — a Bash/Execute request
    // would be auto-approved), so the keys land at known points: Space
    // while the first turn waits, A answers it, x at the paused
    // boundary, A for the third turn.
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), " ")
        .text(MS(600), "a")
        .text(MS(1_200), "x")
        .text(MS(1_800), "a")
        .text(MS(3_000), "\x04");
    let (model, report, prompts) = run(
        dir.path(),
        3,
        script,
        FakeAcpAgent::new().asking_permission("Frobnicate"),
        true,
    )
    .await;
    assert_eq!(prompts.len(), 2, "iterations 1 and 3 ran, 2 was skipped");
    assert_eq!(report.iterations(), 2);
    assert_eq!(
        report.end(),
        &LoopEnd::NoCommits,
        "two iterations without a commit stop the loop"
    );
    let notes = notes(&model);
    assert!(
        notes.contains(&"pausing at the end of this iteration".to_owned()),
        "{notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|n| n.starts_with("paused — space resumes")),
        "the boundary note is on the stage (wrapped to the stage's width): {notes:?}"
    );
    assert!(
        notes.contains(&"skipped iteration 2".to_owned()),
        "{notes:?}"
    );
    let status = model.loop_status().expect("a loop");
    assert_eq!(status.iteration_text(), "iteration 3/3");
    assert_eq!(
        model.permissions_seen(),
        2,
        "one permission per turn reached the strip"
    );
    assert_eq!(model.permissions_pending(), 0);
}

/// A yaml project inside a fresh git repository whose `check` gate is an
/// empty commit: every iteration is a committed one, so the no-commit
/// rule (two in a row end the loop) cannot end a loop a test wants to
/// watch reach its next boundary.
fn committing_project() -> tempfile::TempDir {
    let dir = project(
        "git -c user.name=fixture -c user.email=fixture@magistr -c commit.gpgsign=false \
         commit -q --allow-empty -m tick",
        "true",
        "true",
    );
    let init = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .status()
        .expect("git init");
    assert!(init.success(), "git init the fixture");
    dir
}

/// `s` at a paused boundary steps: exactly one iteration runs, and the
/// driver pauses again at the boundary after it, saying so through the
/// same `Paused` the auto-pause uses — so the strip reads paused without
/// a second key from the user.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn s_while_paused_steps_one_iteration_then_pauses_again() {
    let dir = committing_project();
    // Space while the first turn waits on its permission, A answers it,
    // s at the paused boundary, A for the second turn; the cockpit is
    // quit while the loop holds the boundary before iteration 3.
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), " ")
        .text(MS(600), "a")
        .text(MS(1_200), "s")
        .text(MS(1_800), "a")
        .text(MS(4_000), "\x04");
    let (model, report, prompts) = run(
        dir.path(),
        3,
        script,
        FakeAcpAgent::new().asking_permission("Frobnicate"),
        true,
    )
    .await;
    assert_eq!(prompts.len(), 2, "iterations 1 and 2 ran; 3 never started");
    assert_eq!(report.iterations(), 2);
    assert_eq!(
        report.end(),
        &LoopEnd::CockpitClosed,
        "the loop was still holding the boundary when the cockpit quit"
    );
    let notes = notes(&model);
    assert!(
        notes.iter().any(|n| n.starts_with("committed ")),
        "the fixture's gate commits, so the no-commit rule stays out of it: {notes:?}"
    );
    assert!(
        notes.contains(&"stepping one iteration, then pausing".to_owned()),
        "{notes:?}"
    );
    assert!(
        notes.contains(&"paused: stepped iteration 2".to_owned()),
        "the driver said so at the boundary: {notes:?}"
    );
    assert_eq!(
        notes
            .iter()
            .filter(|n| n.starts_with("paused — space resumes"))
            .count(),
        2,
        "held before iteration 2 and again before iteration 3: {notes:?}"
    );
    let status = model.loop_status().expect("a loop");
    assert!(
        status.is_paused(),
        "paused again after the stepped iteration"
    );
    assert_eq!(status.iteration_text(), "iteration 2/3");
    assert_eq!(model.permissions_pending(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn esc_while_paused_aborts_the_loop() {
    let dir = project("true", "true", "true");
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), " ")
        .text(MS(600), "a")
        .bytes(MS(1_200), b"\x1b".as_slice())
        .text(MS(2_000), "\x04");
    let (model, report, prompts) = run(
        dir.path(),
        3,
        script,
        FakeAcpAgent::new().asking_permission("Frobnicate"),
        true,
    )
    .await;
    assert_eq!(prompts.len(), 1);
    assert_eq!(report.iterations(), 1);
    assert_eq!(report.end(), &LoopEnd::Aborted);
    let status = model.loop_status().expect("a loop");
    assert_eq!(status.ended(), Some("aborted"));
    assert!(!status.is_paused());
    assert!(
        notes(&model).contains(&"aborting the loop".to_owned()),
        "{:?}",
        notes(&model)
    );
}

/// Quitting the cockpit mid-gates ends the gate's process: the sleeping
/// test gate writes its own pid (the process-group leader under
/// `setsid`) and would outlive the whole test at 30s — after ctrl-D the
/// driver cancels the gate jobs and the process dies within the grace.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quitting_mid_gates_ends_the_gates_child_process() {
    let pids = tempfile::TempDir::new().expect("a temp dir");
    let pid_path = pids.path().join("gate.pid");
    let sleeper = format!("echo $$ > '{}' && sleep 30", pid_path.display());
    let dir = project("true", &sleeper, "true");
    let script = Script::new().startup_replies(MS(0)).text(MS(2_000), "\x04");
    let (_model, report, prompts) = run(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
    )
    .await;
    assert_eq!(prompts.len(), 1);
    assert_eq!(report.end(), &LoopEnd::CockpitClosed);
    let pid: i32 = std::fs::read_to_string(&pid_path)
        .expect("the sleeping gate had started and wrote its pid")
        .trim()
        .parse()
        .expect("a pid");
    // Gone within the session grace — a sleeping sh dies on the SIGTERM.
    let deadline = std::time::Instant::now() + Duration::from_secs(6);
    // SAFETY: signal 0 only probes for existence.
    while unsafe { libc::kill(pid, 0) } == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "the gate's child process outlived the quit"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The driver reaches no process and no file itself: the agent comes
/// through the connector, the gates run through `Jobs::spawn_job`.
#[test]
fn the_driver_runs_gates_only_through_jobs_and_spawns_nothing_itself() {
    let driver = include_str!("driver.rs");
    let gate = include_str!("gate_job.rs");
    let connector = include_str!("connector.rs");
    for (name, source) in [
        ("driver.rs", driver),
        ("gate_job.rs", gate),
        ("connector.rs", connector),
    ] {
        for forbidden in [
            "std::process",
            "Command::new",
            "tokio::process",
            "run_gates(",
            "std::fs::",
        ] {
            assert!(!source.contains(forbidden), "{name} reaches {forbidden}");
        }
    }
    assert!(driver.contains(".spawn_job("), "the gates go through Jobs");
    assert!(
        !driver.contains("run_command"),
        "the driver never runs a command on its own thread"
    );
    assert!(
        gate.contains("Handle::current().block_on("),
        "the gate job drives the shared subprocess helper from the blocking thread"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_native_iteration_re_seeds_the_signals_another_process_captured() {
    let dir = project("true", "true", "true");
    let config = TasConfig::from_lookup(dir.path().to_path_buf(), |_| None)
        .with_data_dir(dir.path().join("tas-data"));
    let engines = NativeEngines::open_for_project(&config, "loop-signal".to_owned());
    engines
        .roadmap_add_chunk(
            "work".to_owned(),
            "Chunk work".to_owned(),
            "work".to_owned(),
            ChunkStatus::Pending,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("add chunk");

    // The agent's `magistr mcp-serve` captures a signal after the loop's
    // engines were opened, with no broadcast feed attached — so only the
    // iteration's own re-seed can put it in front of the cockpit.
    let server = NativeEngines::open_for_project(&config, "loop-signal".to_owned());
    let captured = server.signal_capture(SignalKind::Idea, "alice".to_owned(), "faster".to_owned());

    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let (model, report, _) = run_with(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
        Some(engines),
        None,
    )
    .await;
    assert_eq!(report.iterations(), 1);

    let ids: Vec<&str> = model
        .roadmap()
        .signals()
        .iter()
        .map(|signal| signal.id.as_str())
        .collect();
    assert_eq!(ids, vec![captured.id.as_str()], "the inbox caught up");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn off_mode_runs_no_judge_and_the_stage_says_which_mode_drives() {
    let dir = project("true", "true", "true");
    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let (model, report, _) = run(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
    )
    .await;
    assert_eq!(report.end(), &LoopEnd::MaxIterations(1));
    let stage = stage_text(&model);
    assert!(
        stage.contains("goal loop: off — one iteration at a time, no judge"),
        "{stage}"
    );
    assert!(
        !stage.contains("judge,") && !stage.contains("asking the judge"),
        "no verdict, no judge line: {stage}"
    );
    let status = model.loop_status().expect("the strip saw the loop");
    assert_eq!(status.verdicts(), 0);
    assert_eq!(status.verdict(), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_not_yet_verdict_continues_the_loop_with_its_reason_on_the_stage() {
    // Two iterations need a commit each, or the no-commit budget ends
    // the loop first: the test gate makes one, as an agent would.
    let dir = project(
        "true",
        "git -c user.name=t -c user.email=t@t commit -q --allow-empty -m iteration",
        "true",
    );
    assert!(
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir.path())
            .status()
            .expect("git runs")
            .success()
    );
    let engines = engines_with_work(dir.path());
    let judge = Scripted::new([
        Verdict::not_yet("no test for the timeout"),
        Verdict::not_yet("still no test"),
    ]);
    let script = Script::new().startup_replies(MS(0)).text(MS(5_000), "\x04");
    let (model, report, prompts) = run_with(
        dir.path(),
        2,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
        Some(engines.clone()),
        Some(judge.clone()),
    )
    .await;
    assert_eq!(report.iterations(), 2, "a not-yet iterates again");
    assert_eq!(report.end(), &LoopEnd::MaxIterations(2));
    assert_eq!(prompts.len(), 2);

    // What the judge was asked: the chunk, its whole acceptance list,
    // the real gates, the agent's words.
    let asked = judge.asked();
    assert_eq!(asked.len(), 2);
    assert_eq!(asked[0].chunk, "work");
    assert_eq!(asked[0].title, "Chunk work");
    assert_eq!(asked[0].acceptance, ["it works", "it is tested"]);
    assert_eq!(asked[0].iteration, (1, 2));
    assert_eq!(asked[1].iteration, (2, 2));
    // The gates in the order their jobs returned — sorted to compare.
    let mut gates: Vec<(&str, bool)> = asked[0]
        .gates
        .iter()
        .map(|g| (g.name.as_str(), g.passed))
        .collect();
    gates.sort_unstable();
    assert_eq!(gates, [("check", true), ("lint", true), ("test", true)]);
    assert!(
        asked[0].transcript_tail.contains("did the thing"),
        "the agent's words reach the judge: {:?}",
        asked[0].transcript_tail
    );

    let stage = stage_text(&model);
    assert!(
        stage.contains("goal loop: native — magistr's own judge decides after the gates"),
        "{stage}"
    );
    assert!(
        stage.contains("judge, iteration 1: not yet — no test for the timeout"),
        "the reason is on the stage: {stage}"
    );
    assert!(
        stage.contains("judge: not yet — the next iteration takes work again"),
        "{stage}"
    );
    let status = model.loop_status().expect("the strip saw the loop");
    assert_eq!(status.verdicts(), 2, "the count is the loop's");
    assert_eq!(
        status.verdict().map(|v| v.reason.as_str()),
        Some("still no test")
    );
    assert_eq!(
        engines.roadmap_chunk("work").expect("the chunk").status,
        ChunkStatus::Pending,
        "a not-yet touches nothing"
    );
}

/// The agent finds its chunk blocked on something outside the repo and
/// records it through its own `magistr mcp-serve` — a second handle on
/// the store, written once the driver has picked the target and is
/// connecting for the turn. The judge is not asked about a chunk nobody
/// could have worked, and the stage does not promise it again: the next
/// iteration's target is `roadmap_next`'s answer, which skips it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_target_the_agent_blocked_is_not_sent_to_the_judge() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    let judge = Scripted::new([Verdict::met("never asked")]);
    let agent_side =
        NativeEngines::open_for_project(&work_config(dir.path()), "loop-goal".to_owned());
    let (inner, seen) =
        fake_connector(FakeAcpAgent::new().reply("blocked: the framework repo does not exist"));
    let inner = Arc::new(inner);
    let connector = Connector::new(move |config| {
        agent_side
            .roadmap_set_blocked_by(
                "work",
                BlockerKind::External,
                "the framework repo does not exist yet".to_owned(),
                None,
            )
            .expect("the agent blocks its chunk");
        let inner = Arc::clone(&inner);
        async move { inner.connect(config).await }
    });
    let script = Script::new().startup_replies(MS(0));
    let (model, report, prompts) = run_over(
        dir.path(),
        1,
        script,
        connector,
        seen,
        false,
        Some(engines.clone()),
        Some(judge.clone()),
    )
    .await;
    assert_eq!(report.iterations(), 1);
    assert_eq!(report.end(), &LoopEnd::MaxIterations(1));
    assert_eq!(prompts.len(), 1, "the turn ran");
    assert!(
        judge.asked().is_empty(),
        "a blocked chunk is not a question for the judge: {:?}",
        judge.asked()
    );
    let chunk = engines.roadmap_chunk("work").expect("the chunk");
    assert!(
        chunk.blocked_by.is_some(),
        "the driver re-read the store the agent wrote: {chunk:?}"
    );
    assert_eq!(
        chunk.status,
        ChunkStatus::Pending,
        "a blocker leaves the status where it was — which is why the driver reads blocked_by"
    );
    let stage = stage_text(&model);
    assert!(
        stage.contains(
            "judge not asked: work was blocked by the agent this iteration — the framework repo does not exist yet"
        ),
        "{stage}"
    );
    assert!(
        !stage.contains("takes work again"),
        "nothing promises a blocked chunk again: {stage}"
    );
    assert!(
        report.metrics()[0].judge.is_none(),
        "no judge time was spent: {}",
        report.metrics()[0]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ok_verdict_over_green_gates_completes_the_chunk_through_the_engines() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    let judge = Scripted::new([Verdict::met("both criteria hold")]);
    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let (model, report, _) = run_with(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
        Some(engines.clone()),
        Some(judge),
    )
    .await;
    assert_eq!(report.iterations(), 1);
    assert!(
        report.metrics()[0].judge.is_some(),
        "the judge's ask is measured: {}",
        report.metrics()[0]
    );
    assert_eq!(
        engines.roadmap_chunk("work").expect("the chunk").status,
        ChunkStatus::Done,
        "the chunk the agent never even started is started and completed"
    );
    let notes = notes(&model);
    assert!(
        notes.contains(&"judge: met — both criteria hold; work completed".to_owned()),
        "{notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|n| n.starts_with("judge, iteration 1: met — both criteria hold")),
        "{notes:?}"
    );
    let status = model.loop_status().expect("the strip saw the loop");
    assert_eq!(status.verdicts(), 1);
    assert_eq!(
        status.verdict_text(&Icons::UNICODE).as_deref(),
        Some("✓ judge")
    );
    // The completion frame reached the cockpit's roadmap view.
    assert_eq!(
        model.roadmap().chunk("work").map(|c| c.status.as_str()),
        Some("done"),
        "the Roadmap tab folded the completion"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ok_verdict_over_a_red_gate_is_a_disagreement_and_the_chunk_stays() {
    let dir = project("true", "true", "false");
    let engines = engines_with_work(dir.path());
    let judge = Scripted::new([Verdict::met("looks done to me")]);
    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let (model, report, _) = run_with(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
        Some(engines.clone()),
        Some(judge),
    )
    .await;
    assert_eq!(report.end(), &LoopEnd::MaxIterations(1), "the loop goes on");
    assert_ne!(
        engines.roadmap_chunk("work").expect("the chunk").status,
        ChunkStatus::Done,
        "a red gate cannot be judged green"
    );
    let stage = stage_text(&model);
    assert!(
        stage.contains(
            "error: the judge says work is met (looks done to me) but the gates disagree: lint failed — the chunk stays in progress"
        ),
        "{stage}"
    );
    let status = model.loop_status().expect("the strip saw the loop");
    assert_eq!(status.verdicts(), 1);
    assert_eq!(status.gates_passed(), Some(false));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_impossible_verdict_blocks_the_chunk_on_a_human_and_ends_the_loop() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    let judge = Scripted::new([Verdict::impossible(
        "the two criteria contradict each other",
    )]);
    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let (model, report, prompts) = run_with(
        dir.path(),
        3,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
        Some(engines.clone()),
        Some(judge),
    )
    .await;
    assert_eq!(prompts.len(), 1, "the loop stops, budget or not");
    assert_eq!(
        report.end(),
        &LoopEnd::NeedsHuman("the two criteria contradict each other".to_owned())
    );
    assert_eq!(
        report.to_string().lines().next(),
        Some("loop: 1 iteration · the judge needs a human: the two criteria contradict each other")
    );
    let chunk = engines.roadmap_chunk("work").expect("the chunk");
    let blocker = chunk.blocked_by.expect("blocked on a human");
    assert_eq!(blocker.kind, BlockerKind::AwaitingHuman);
    assert!(
        blocker.reason.contains("contradict each other"),
        "{}",
        blocker.reason
    );
    // The dock raised the Needs-you card from the driver's own frame.
    match model.dock().front() {
        Some(DockItem::Blocker { chunk, reason }) => {
            assert_eq!(chunk, "work");
            assert!(reason.contains("contradict each other"), "{reason}");
        }
        other => panic!("a blocker card is on the dock: {other:?}"),
    }
    let stage = stage_text(&model);
    assert!(
        stage.ends_with(
            "loop ended: the judge needs a human: the two criteria contradict each other"
        ),
        "{stage}"
    );
    let status = model.loop_status().expect("the strip saw the loop");
    assert_eq!(status.verdicts(), 1);
    assert_eq!(status.verdict().map(|v| v.impossible), Some(true));
}

/// Every goal every agent of the run saw, in order.
fn goals_seen(agents: &[Arc<AcpObserved>]) -> Vec<(String, Option<String>)> {
    agents.iter().flat_map(|a| a.goals()).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn extension_mode_sets_the_goal_through_the_advertised_method_and_skips_the_judge() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    let judge = Scripted::new([Verdict::met("never asked")]);
    let script = Script::new().startup_replies(MS(0));
    let (mode, note) = GoalMode::from_value(Some("extension"));
    assert_eq!((mode, note), (GoalMode::Extension, None));
    let (model, report, agents) = run_delegated(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().advertising_goal_extension(),
        engines,
        judge.clone(),
        mode,
    )
    .await;
    assert_eq!(report.iterations(), 1);
    assert_eq!(report.end(), &LoopEnd::MaxIterations(1));
    assert_eq!(report.goal(), Some("delegated (extension)"));
    assert!(
        report
            .to_string()
            .lines()
            .next()
            .is_some_and(|line| line.ends_with("· goal: delegated (extension)")),
        "{report}"
    );
    assert!(
        report.metrics()[0].judge.is_none(),
        "a delegated iteration asks no judge: {}",
        report.metrics()[0]
    );
    assert_eq!(
        agents.len(),
        1,
        "one session, one delivery — stage: {}",
        stage_text(&model)
    );
    let goals = goals_seen(&agents);
    assert_eq!(goals.len(), 1, "{goals:?}");
    assert_eq!(goals[0].0, "set");
    let objective = goals[0].1.as_deref().expect("an objective");
    assert!(
        objective.starts_with("Done means: roadmap chunk `work` is complete (Chunk work)"),
        "{objective}"
    );
    assert!(objective.contains("print its verdict line"), "{objective}");
    assert!(
        agents[0].prompts().is_empty(),
        "the extension is the turn — no session/prompt was sent"
    );
    assert!(
        judge.asked().is_empty(),
        "magistr's judge is not asked inside a delegated loop"
    );
    let stage = stage_text(&model);
    assert!(
        stage.contains("goal loop: extension — delegating to the agent's own /goal"),
        "{stage}"
    );
    assert!(stage.contains("goal (_session/goal) →"), "{stage}");
    assert!(
        stage.contains("goal loop delegated (extension) — confirmed by the agent's echo (delivery and recognition, not progress)"),
        "{stage}"
    );
    assert!(
        stage.contains("delegated iteration settled — the agent's own evaluator judged it; magistr saw no verdict"),
        "{stage}"
    );
    // This rests on the inbox's FIFO order and on nothing else: the fake
    // streams these words BEFORE the echo the driver treats as delivery
    // confirmed, so they are ahead of every event the driver posts for the
    // rest of the iteration, and ahead of the quit key the harness sends
    // behind the report. Sent after the echo — as they were until this was
    // fixed — they raced the whole rest of the loop, and lost it under
    // compilation contention.
    assert!(
        model
            .turn()
            .rows()
            .iter()
            .any(|r| r.text.starts_with("working toward:")),
        "the agent's goal-loop words stream onto the stage: {stage}"
    );
    let status = model.loop_status().expect("the strip saw the loop");
    assert_eq!(status.verdicts(), 0, "no verdict crosses the wire");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn command_mode_composes_the_discovered_turn_and_is_confirmed_by_the_echo() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    let judge = Scripted::new([Verdict::met("never asked")]);
    let script = Script::new().startup_replies(MS(0));
    let (model, report, agents) = run_delegated(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new()
            .advertising_goal_command()
            .reply("on it"),
        engines,
        judge.clone(),
        GoalMode::Command,
    )
    .await;
    assert_eq!(report.iterations(), 1);
    assert_eq!(report.goal(), Some("delegated (command)"));
    assert_eq!(agents.len(), 1, "{}", stage_text(&model));
    let prompts = agents[0].prompts();
    assert_eq!(
        prompts.len(),
        1,
        "{prompts:?} — stage: {}",
        stage_text(&model)
    );
    assert!(
        prompts[0].starts_with("/goal Done means: roadmap chunk `work` is complete"),
        "the turn is the discovered name plus the objective: {}",
        prompts[0]
    );
    let goals = goals_seen(&agents);
    assert_eq!(goals.len(), 1);
    assert_eq!(goals[0].0, "set");
    assert!(judge.asked().is_empty());
    let stage = stage_text(&model);
    assert!(stage.contains("goal → /goal Done means:"), "{stage}");
    assert!(
        stage.contains("goal loop delegated (command) — confirmed by the agent's echo"),
        "{stage}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_falls_from_an_unadvertised_extension_to_the_listed_command() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    let judge = Scripted::new([Verdict::met("never asked")]);
    let script = Script::new().startup_replies(MS(0));
    let (model, report, agents) = run_delegated(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().advertising_goal_command(),
        engines,
        judge.clone(),
        GoalMode::Auto,
    )
    .await;
    assert_eq!(report.goal(), Some("delegated (command)"));
    assert_eq!(
        agents.len(),
        2,
        "the extension attempt had its own session before the command's"
    );
    assert!(
        agents[0].goals().is_empty(),
        "nothing was sent to a session with no extension"
    );
    assert_eq!(agents[1].goals().len(), 1);
    assert!(judge.asked().is_empty());
    let stage = stage_text(&model);
    assert!(
        stage.contains("goal extension: the agent advertises no usable goal extension"),
        "{stage}"
    );
    assert!(
        stage.contains("goal loop delegated (command) — confirmed by the agent's echo"),
        "{stage}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mute_agent_is_reported_cleared_and_the_native_judge_takes_over() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    let judge = Scripted::new([Verdict::not_yet("the fallback judge was asked")]);
    let script = Script::new().startup_replies(MS(0));
    let (model, report, agents) = run_delegated(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new()
            .advertising_goal_extension()
            .mute_goal()
            .reply("did the thing"),
        engines,
        judge.clone(),
        GoalMode::Extension,
    )
    .await;
    assert_eq!(report.iterations(), 1, "the iteration still ran, natively");
    assert_eq!(report.goal(), Some("extension → native (no echo)"));
    assert_eq!(
        agents.len(),
        2,
        "the delegated session was abandoned and a native one opened"
    );
    assert_eq!(
        agents[0].goals(),
        [
            (
                "set".to_owned(),
                Some(AgentGoal::compile_objective(Some(("work", "Chunk work"))))
            ),
            ("clear".to_owned(), None),
        ],
        "the unconfirmed set was cleared before the session was dropped"
    );
    assert_eq!(
        agents[1].prompts().len(),
        1,
        "the native turn ran on the second session"
    );
    assert!(
        !agents[1].prompts()[0].starts_with("/goal") && agents[1].prompts()[0].contains("work"),
        "with the iteration prompt, not the objective: {}",
        agents[1].prompts()[0]
    );
    let asked = judge.asked();
    assert_eq!(asked.len(), 1, "the native judge decided the iteration");
    assert_eq!(asked[0].chunk, "work");
    let stage = stage_text(&model);
    assert!(
        stage.contains(
            "error: goal extension: no echo within 0.4 s — the delivery was cleared and abandoned"
        ),
        "{stage}"
    );
    assert!(
        stage.contains("error: MAGISTR_GOAL=extension asked to delegate the loop, but no delivery was confirmed by the agent's echo — running magistr's own loop instead"),
        "the fallback is a visible red line: {stage}"
    );
    assert!(
        stage.contains("judge, iteration 1: not yet — the fallback judge was asked"),
        "{stage}"
    );
}

/// The driver reads nothing off a goal snapshot beyond the echo's
/// objective: the only vocabulary it uses is the vendored file's, and
/// that file models one field.
#[test]
fn the_driver_relies_on_nothing_beyond_the_optimistic_echo() {
    let driver = include_str!("driver.rs");
    for runtime_field in [
        "iterations",
        "lastReason",
        "last_reason",
        "tokensUsed",
        "createdAt",
    ] {
        assert!(
            !driver.contains(&format!("\"{runtime_field}\"")),
            "the driver must not read the runtime field {runtime_field} — it never crosses the wire"
        );
    }
    assert!(
        driver.contains("AgentGoal::echo("),
        "confirmation is the echo"
    );
    assert!(
        !driver.contains("_session/goal"),
        "the method comes from the advertisement"
    );
    assert!(
        !driver.contains("\"/goal"),
        "the command comes from the advertised list"
    );
}

/// The three cargos contend for one build lock on one target directory,
/// so racing them buys no wall clock and costs the ability to say which
/// gate spent which second. Each gate here writes an `s` when it starts
/// and an `e` when it ends: serialized that is `sesese`, and any overlap
/// shows up as two `s` in a row.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_gates_run_one_at_a_time_not_three_at_once() {
    let mark = "printf s >> gates.log; sleep 0.15; printf e >> gates.log";
    let dir = project(mark, mark, mark);
    let script = Script::new().startup_replies(MS(0)).text(MS(4_000), "\x04");
    let (model, report, _) = run(dir.path(), 1, script, FakeAcpAgent::new(), false).await;

    assert_eq!(report.end(), &LoopEnd::MaxIterations(1));
    let log = std::fs::read_to_string(dir.path().join("gates.log")).expect("the gates ran");
    assert_eq!(log, "sesese", "the gates overlapped: {log}");
    let status = model.loop_status().expect("the strip saw the loop");
    assert_eq!(status.gates().len(), 3);
    assert_eq!(status.gates_passed(), Some(true));
}

/// A red required gate is run once more before the loop believes it. The
/// strip still shows one LED per gate, not one per attempt.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_red_required_gate_is_run_once_more_before_the_loop_believes_it() {
    let dir = project("true", "true", "exit 3");
    let script = Script::new().startup_replies(MS(0)).text(MS(3_000), "\x04");
    let (model, _, _) = run(dir.path(), 1, script, FakeAcpAgent::new(), true).await;

    let status = model.loop_status().expect("the strip saw the loop");
    let leds: Vec<String> = status
        .gates()
        .iter()
        .map(|gate| gate.text(&Icons::UNICODE))
        .collect();
    assert_eq!(leds, ["\u{2713} check", "\u{2713} test", "\u{2717} lint"]);
    assert_eq!(model.jobs_done(), 4, "the red gate ran twice");
    let notes = notes(&model);
    assert!(
        notes
            .iter()
            .any(|n| n.contains("lint failed (exit 3)") && n.contains("running it once more")),
        "the retry is announced with the exit code it failed with: {notes:?}"
    );
    assert!(
        notes.contains(&"error: quality gates failed: lint failed".to_owned()),
        "red twice is real: {notes:?}"
    );
    assert!(
        status.is_paused(),
        "auto-pause was on and the gate is really red"
    );
}

/// Green on the retry is a flake: named rather than hidden, and not a
/// reason to pause the loop. 2026 flaky-test guidance is that an
/// always-fails verdict is a real failure retries were masking, while the
/// 1-99% band is what classification is for.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_gate_that_fails_then_passes_is_a_flake_and_does_not_pause_the_loop() {
    // Fails the first time it is run in this directory, passes after.
    let flake = "if [ -e flake.marker ]; then exit 0; else touch flake.marker; exit 9; fi";
    let dir = project("true", flake, "true");
    let script = Script::new().startup_replies(MS(0)).text(MS(3_000), "\x04");
    let (model, _, _) = run(dir.path(), 1, script, FakeAcpAgent::new(), true).await;

    let status = model.loop_status().expect("the strip saw the loop");
    assert_eq!(
        status.gates_passed(),
        Some(true),
        "a flake is not a red run"
    );
    assert!(!status.is_paused(), "a flake does not pause the loop");
    assert_eq!(model.jobs_done(), 4, "the flaky gate ran twice");
    let notes = notes(&model);
    assert!(
        notes
            .iter()
            .any(|n| n.contains("flaky, passed on the retry: test")),
        "the flake is named, not hidden: {notes:?}"
    );
    assert!(
        !notes.iter().any(|n| n.contains("quality gates failed")),
        "{notes:?}"
    );
}

/// The loop's gates are the project's top-level `quality_gate`. Sharing
/// the *runner* with `magistr_gate` must not silently move the loop onto
/// `resolve_gates`' per-archetype specs, which would change which
/// commands run on a polyglot project.
#[test]
fn the_loop_resolves_its_gates_from_the_top_level_quality_gate() {
    let driver = include_str!("driver.rs");
    assert!(
        driver.contains("gate_command(&quality_gate, gate)"),
        "the loop reads the top-level quality_gate"
    );
    assert!(
        !driver.contains("resolve_gates("),
        "the loop does not call the per-archetype gate resolver"
    );
}

/// A ship trace with one active task, the way the agent's own
/// `magistr mcp-serve` leaves one behind for the loop's gates to record
/// into.
fn engines_with_active_task(dir: &Path) -> NativeEngines {
    let engines = engines_with_work(dir);
    engines.ship_set_objective("do the work".into(), vec![], vec![], "chunk:work".into());
    engines.ship_add_task(
        "impl".into(),
        "implement".into(),
        think_and_ship::ship::domain::task::TaskType::Implement,
        None,
    );
    engines.ship_start_task("impl").expect("start the task");
    engines
}

fn recorded(engines: &NativeEngines) -> Vec<think_and_ship::ship::domain::check::Check> {
    engines
        .ship_tasks()
        .into_iter()
        .flat_map(|t| t.checks)
        .collect()
}

/// The gates the loop runs between iterations used to write nothing at
/// all, while the agent's own `magistr_gate` wrote verified records
/// beside them. Each one is now a verified check with its real exit code,
/// its command and its output.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_gate_the_loop_runs_reaches_the_trace_as_a_verified_check() {
    let dir = project("true", "echo ran-the-tests", "exit 4");
    let engines = engines_with_active_task(dir.path());
    let script = Script::new().startup_replies(MS(0)).text(MS(4_000), "\x04");
    let (model, _, _) = run_with(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new(),
        false,
        Some(engines.clone()),
        None,
    )
    .await;

    engines.reload_ship();
    let checks = recorded(&engines);
    let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["check", "test", "lint"], "{names:?}");
    for check in &checks {
        assert!(check.verified, "never self-reported: {}", check.name);
        assert!(check.required);
        assert!(check.command.is_some(), "the command that produced it");
    }
    let lint = checks.iter().find(|c| c.name == "lint").expect("lint");
    assert_eq!(lint.exit_code, Some(4), "the real code, not a bool");
    assert!(!lint.passed);
    assert_eq!(lint.command.as_deref(), Some("exit 4"));
    let test = checks.iter().find(|c| c.name == "test").expect("test");
    assert!(test.passed);
    assert!(
        test.details.contains("ran-the-tests"),
        "the output is on the record: {}",
        test.details
    );
    assert!(
        !notes(&model).iter().any(|n| n.contains("not recorded")),
        "nothing said it could not record"
    );
}

/// The record is where the flake stops being invisible: it is stored as
/// passed with the retry's exit code, so both codes have to be on it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_flake_is_recorded_as_passed_with_both_exit_codes_on_the_record() {
    let flake = "if [ -e flake.marker ]; then exit 0; else touch flake.marker; exit 9; fi";
    let dir = project("true", flake, "true");
    let engines = engines_with_active_task(dir.path());
    let script = Script::new().startup_replies(MS(0)).text(MS(4_000), "\x04");
    let (_, _, _) = run_with(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new(),
        true,
        Some(engines.clone()),
        None,
    )
    .await;

    engines.reload_ship();
    let checks = recorded(&engines);
    assert_eq!(checks.len(), 3, "one record per gate, not per attempt");
    let test = checks.iter().find(|c| c.name == "test").expect("test");
    assert!(test.passed, "green on the retry is green");
    assert_eq!(test.exit_code, Some(0));
    assert!(
        test.details.contains("exit 9") && test.details.contains("retry"),
        "the failing attempt is on the record: {}",
        test.details
    );
}

/// A green `cargo nextest` on a large workspace emits hundreds of KB of
/// PASS lines, and the store is re-read and merge-written whole on every
/// mutation. A passing gate keeps its tail; a red one keeps everything.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_passing_gate_does_not_persist_its_whole_output_and_a_red_one_does() {
    let noisy = "for i in $(seq 1 4000); do echo 'PASS some::test::name'; done";
    let dir = project(
        "true",
        &format!("{noisy}; exit 0"),
        &format!("{noisy}; exit 1"),
    );
    let engines = engines_with_active_task(dir.path());
    let script = Script::new().startup_replies(MS(0)).text(MS(6_000), "\x04");
    let (_, _, _) = run_with(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new(),
        false,
        Some(engines.clone()),
        None,
    )
    .await;

    engines.reload_ship();
    let checks = recorded(&engines);
    let green = checks.iter().find(|c| c.name == "test").expect("test");
    let red = checks.iter().find(|c| c.name == "lint").expect("lint");
    assert!(
        green.details.len() < 4_000,
        "a green gate keeps its tail, not 4000 PASS lines: {} bytes",
        green.details.len()
    );
    assert!(
        green.details.contains("tail only"),
        "and says so: {}",
        green.details
    );
    assert!(
        red.details.len() > 50_000,
        "a red gate keeps everything: {} bytes",
        red.details.len()
    );
}

/// A loop with no think-and-ship engines cannot record, and says so once
/// rather than silently recording nothing — the same honesty
/// `magistr_gate`'s own `recorded: false` note carries.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_loop_without_the_engines_says_the_gates_were_not_recorded() {
    let dir = project("true", "true", "true");
    let script = Script::new().startup_replies(MS(0)).text(MS(3_000), "\x04");
    let (model, _, _) = run(dir.path(), 1, script, FakeAcpAgent::new(), false).await;

    let notes = notes(&model);
    let said: Vec<&String> = notes
        .iter()
        .filter(|n| n.contains("gates not recorded"))
        .collect();
    assert_eq!(
        said.len(),
        1,
        "once for the set, not once per gate: {said:?}"
    );
    assert!(
        said[0].contains("no think-and-ship engines"),
        "the whole reason fits on one row: {said:?}"
    );
}

/// The engines are there but nobody opened a task to record against: the
/// refusal reaches the stage instead of being swallowed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_store_with_no_active_task_says_why_the_gates_were_not_recorded() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    let script = Script::new().startup_replies(MS(0)).text(MS(3_000), "\x04");
    let (model, _, _) = run_with(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new(),
        false,
        Some(engines.clone()),
        None,
    )
    .await;

    engines.reload_ship();
    assert!(recorded(&engines).is_empty());
    let notes = notes(&model);
    let said: Vec<&String> = notes
        .iter()
        .filter(|n| n.contains("gates not recorded"))
        .collect();
    assert_eq!(said.len(), 1, "once for the set: {said:?}");
    assert!(
        !said[0].contains("no think-and-ship engines"),
        "it carries the engine's own reason, not the no-engines one: {said:?}"
    );
}

/// The gates run after the agent has already completed its chunk and
/// committed, so they can prevent none of it. Their authority is over the
/// roadmap: a chunk the agent completed itself while a required gate is
/// red does not stay done.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_red_gate_reopens_the_chunk_the_agent_completed_itself() {
    let dir = project("true", "true", "exit 3");
    let engines = engines_with_work(dir.path());
    // The agent's own process, completing the chunk inside its turn the
    // way the /next workflow mandates.
    let agent_side =
        NativeEngines::open_for_project(&work_config(dir.path()), "loop-goal".to_owned());
    let (inner, seen) = fake_connector(FakeAcpAgent::new().reply("shipped it"));
    let inner = Arc::new(inner);
    let connector = Connector::new(move |config| {
        agent_side.roadmap_start_chunk("work").expect("start");
        agent_side
            .roadmap_complete_chunk("work", None)
            .expect("the agent completes its own chunk");
        let inner = Arc::clone(&inner);
        async move { inner.connect(config).await }
    });
    let script = Script::new().startup_replies(MS(0)).text(MS(4_000), "\x04");
    let (model, _, _) = run_over(
        dir.path(),
        1,
        script,
        connector,
        seen,
        false,
        Some(engines.clone()),
        None,
    )
    .await;

    engines.reload_roadmap();
    assert_eq!(
        engines.roadmap_chunk("work").expect("the chunk").status,
        ChunkStatus::InProgress,
        "the chunk does not stay done over a red gate"
    );
    let notes = notes(&model);
    assert!(
        notes.iter().any(|n| n.contains("work reopened")),
        "the stage names the reopen: {notes:?}"
    );
    assert!(
        notes.iter().any(|n| n.contains("lint")),
        "and the gate that caused it: {notes:?}"
    );
}

/// Green gates leave a self-completed chunk exactly as the agent left it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn green_gates_leave_a_self_completed_chunk_done() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    let agent_side =
        NativeEngines::open_for_project(&work_config(dir.path()), "loop-goal".to_owned());
    let (inner, seen) = fake_connector(FakeAcpAgent::new().reply("shipped it"));
    let inner = Arc::new(inner);
    let connector = Connector::new(move |config| {
        agent_side.roadmap_start_chunk("work").expect("start");
        agent_side
            .roadmap_complete_chunk("work", None)
            .expect("complete");
        let inner = Arc::clone(&inner);
        async move { inner.connect(config).await }
    });
    let script = Script::new().startup_replies(MS(0)).text(MS(4_000), "\x04");
    let (model, _, _) = run_over(
        dir.path(),
        1,
        script,
        connector,
        seen,
        false,
        Some(engines.clone()),
        None,
    )
    .await;

    engines.reload_roadmap();
    assert_eq!(
        engines.roadmap_chunk("work").expect("the chunk").status,
        ChunkStatus::Done
    );
    assert!(
        !notes(&model).iter().any(|n| n.contains("reopened")),
        "{:?}",
        notes(&model)
    );
}

/// A flake passed, so it is not evidence of anything to act on.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_flake_never_reopens_a_chunk() {
    let flake = "if [ -e flake.marker ]; then exit 0; else touch flake.marker; exit 9; fi";
    let dir = project("true", flake, "true");
    let engines = engines_with_work(dir.path());
    let agent_side =
        NativeEngines::open_for_project(&work_config(dir.path()), "loop-goal".to_owned());
    let (inner, seen) = fake_connector(FakeAcpAgent::new());
    let inner = Arc::new(inner);
    let connector = Connector::new(move |config| {
        agent_side.roadmap_start_chunk("work").expect("start");
        agent_side
            .roadmap_complete_chunk("work", None)
            .expect("complete");
        let inner = Arc::clone(&inner);
        async move { inner.connect(config).await }
    });
    let script = Script::new().startup_replies(MS(0)).text(MS(4_000), "\x04");
    let (model, _, _) = run_over(
        dir.path(),
        1,
        script,
        connector,
        seen,
        true,
        Some(engines.clone()),
        None,
    )
    .await;

    engines.reload_roadmap();
    assert_eq!(
        engines.roadmap_chunk("work").expect("the chunk").status,
        ChunkStatus::Done,
        "a flake is not a reason to reopen"
    );
    assert!(
        !notes(&model).iter().any(|n| n.contains("reopened")),
        "{:?}",
        notes(&model)
    );
}

/// The next iteration's prompt carries the failure rows the gate named,
/// not a sentence about them — which is only possible because the gates
/// keep their output now.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_next_iterations_prompt_carries_the_rows_the_red_gate_named() {
    let lint = "echo 'error[E0308]: mismatched types'; echo '  --> src/late.rs:7:3'; exit 1";
    let dir = project("true", "true", lint);
    let script = Script::new().startup_replies(MS(0)).text(MS(8_000), "\x04");
    let (_, report, prompts) = run(dir.path(), 2, script, FakeAcpAgent::new(), false).await;

    assert_eq!(report.iterations(), 2);
    assert_eq!(prompts.len(), 2, "two turns");
    assert!(
        !prompts[0].contains("quality gates failed"),
        "the first iteration carries nothing"
    );
    let carried = &prompts[1];
    assert!(
        carried.contains("The last iteration's quality gates failed"),
        "{carried}"
    );
    assert!(
        carried.contains("`lint`") && carried.contains("exit 1"),
        "{carried}"
    );
    assert!(
        carried.contains("src/late.rs:7:3") && carried.contains("mismatched types"),
        "the rows themselves, not a sentence: {carried}"
    );
}

/// The carry is appended once, not accumulated across a red streak. It is
/// taken rather than read, so a prompt built after two red iterations
/// still carries exactly one brief.
///
/// The stronger claim — that a green iteration after a red one starts
/// clean — is not asserted here, and deliberately: `project()` builds no
/// git repository, so every iteration is a no-commit one and the loop
/// stops at MAX_NO_COMMIT_RETRIES (2) before a third prompt is ever built.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_carry_is_appended_once_and_never_accumulated() {
    let lint = "echo 'error[E0433]: failed to resolve'; echo '  --> src/a.rs:1:1'; exit 1";
    let dir = project("true", "true", lint);
    let script = Script::new().startup_replies(MS(0)).text(MS(8_000), "\x04");
    let (_, report, prompts) = run(dir.path(), 2, script, FakeAcpAgent::new(), false).await;

    assert_eq!(report.iterations(), 2);
    assert_eq!(
        prompts[1]
            .matches("The last iteration's quality gates failed")
            .count(),
        1,
        "one brief, taken not read: {}",
        &prompts[1][prompts[1].len().saturating_sub(600)..]
    );
}

/// The authority rule finally runs on the real path. `judged` skips a
/// chunk that is `Done`, which is exactly what the `/next` workflow tells
/// the agent to leave behind — so before the reopen, a self-completed
/// chunk over a red gate never reached `Decision::reconcile` at all. It
/// does now, and the judge's "met" over a red gate is a disagreement.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_self_completed_chunk_over_a_red_gate_reaches_the_authority_rule() {
    let dir = project("true", "true", "exit 3");
    let engines = engines_with_work(dir.path());
    let agent_side =
        NativeEngines::open_for_project(&work_config(dir.path()), "loop-goal".to_owned());
    let judge = Scripted::new([Verdict::met("I shipped it")]);
    let (inner, seen) = fake_connector(FakeAcpAgent::new().reply("shipped it"));
    let inner = Arc::new(inner);
    let connector = Connector::new(move |config| {
        agent_side.roadmap_start_chunk("work").expect("start");
        agent_side
            .roadmap_complete_chunk("work", None)
            .expect("complete");
        let inner = Arc::clone(&inner);
        async move { inner.connect(config).await }
    });
    let script = Script::new().startup_replies(MS(0)).text(MS(5_000), "\x04");
    let (model, _, _) = run_over(
        dir.path(),
        1,
        script,
        connector,
        seen,
        false,
        Some(engines.clone()),
        Some(judge.clone()),
    )
    .await;

    assert_eq!(
        judge.asked().len(),
        1,
        "the judge is asked about a chunk the reopen put back in progress"
    );
    let notes = notes(&model);
    assert!(
        notes.iter().any(|n| n.contains("the gates disagree")),
        "met over a red gate is a disagreement, never a completion: {notes:?}"
    );
    engines.reload_roadmap();
    assert_eq!(
        engines.roadmap_chunk("work").expect("the chunk").status,
        ChunkStatus::InProgress,
        "and the chunk stays in progress"
    );
}
