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
use magistr_cockpit::turn::LineKind;
use magistr_engine::native::BlockerKind;
use magistr_engine::{
    ChunkStatus, NativeConfig, NativeEngines, ProjectAuthority, SignalKind, WriterRole,
};
use magistr_tui::graphics::Env;
use magistr_tui::harness::{AcpObserved, FakeAcpAgent, FakeTerminal, Script};
use magistr_tui::runtime::{Options, Runtime};

use super::*;

#[tokio::test]
async fn absent_provider_reports_every_failed_control_and_keeps_local_controls_available() {
    let dir = tempfile::tempdir().unwrap();
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let session = crate::orchestration::interactive_session::interactive_session(
        AgentConfig::new("", dir.path()),
        Connector::new(|_| async { panic!("controls must not launch an agent") }),
        inbox,
        None,
    )
    .spawn(receiver);
    for command in [
        Cmd::Cancel,
        Cmd::Permission {
            id: 9,
            option_id: "allow".into(),
        },
        Cmd::Constraint {
            id: 9,
            condition: "only this file".into(),
        },
        Cmd::Delegate("read".into()),
        Cmd::Reconnect,
        Cmd::CancelRoadmapPrompt,
        Cmd::Loop(magistr_cockpit::cockpit::LoopCmd::Pause),
    ] {
        commands.send(command.clone()).unwrap();
        let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(event, Msg::CommandFailed { cmd, error } if cmd == command && !error.is_empty())
        );
    }
    commands
        .send(Cmd::Engine(magistr_cockpit::cockpit::EngineCmd::Checkpoint))
        .unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        event,
        Msg::Engine(magistr_cockpit::engine::EngineEvent::PortFailed { .. })
    ));
    drop(commands);
    session.await.unwrap().unwrap();
}

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

/// The loop's admitted host writer on a temp store with one pending chunk
/// `work`, and the same engines by a second handle for the assertions.
fn engines_with_work(dir: &Path) -> NativeEngines {
    let engines = host(&work_config(dir), "loop-goal");
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

/// The store [`engines_with_work`] opens — on the writer config production
/// admits the loop on, carrying the fixture's evidence capability, so a
/// gate the loop or the agent records certifies the tree and a judged
/// completion has receipts to stand on.
fn work_config(dir: &Path) -> NativeConfig {
    magistr_core::evidence::writer_config(dir, |_| None).with_data_dir(dir.join("tas-data"))
}

/// The agent's own process completing `work` inside its turn the only way
/// a chunk completes: an objective scoped to it, a verification task, every
/// configured gate recorded on that task with the outcome the agent claims,
/// then `roadmap_complete_chunk` naming the task. The loop's own gates land
/// on the same task afterwards — it is the one active in the trace.
fn agent_completes_work(agent_side: &NativeEngines, root: &Path, gates: [(&str, &str); 3]) {
    agent_side.roadmap_start_chunk("work").expect("start");
    agent_side
        .ship_set_objective("do the work".into(), vec![], vec![], "chunk:work".into())
        .expect("objective");
    agent_side
        .ship_add_task(
            "verify".into(),
            "verify".into(),
            magistr_workflow::ship::domain::task::TaskType::Test,
            None,
        )
        .expect("task");
    agent_side.ship_start_task("verify").expect("active");
    for (gate, command) in gates {
        crate::application::AdmittedCheck::admit(
            agent_side,
            root,
            Some("verify"),
            gate,
            command.into(),
            true,
        )
        .expect("admitted")
        .settle(
            agent_side,
            magistr_engine::native::CheckExecution::Exited { code: 0 },
            String::new(),
        )
        .expect("settled");
    }
    agent_side
        .roadmap_complete_chunk("work", Some("task:verify"))
        .expect("the agent completes its own chunk on its receipts");
}

/// The loop's own writer role on `config`.
fn host(config: &NativeConfig, project_id: &str) -> NativeEngines {
    ProjectAuthority::admit_for_project(config, project_id.to_owned(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

/// A second writer on the same store, in the role the agent's own
/// `magistr mcp-serve` process holds beside the loop.
fn agent_side(config: &NativeConfig, project_id: &str) -> NativeEngines {
    ProjectAuthority::admit_for_project(config, project_id.to_owned(), WriterRole::Service)
        .expect("service admitted")
        .open()
}

/// The real ledger over `engines` and the project at `dir` — attached to
/// every native loop these tests run, as the entry points attach it, so
/// the boundary checkpoint's cost is paid exactly where the delegated
/// goal's deadlines are measured.
fn ledger(dir: &Path, engines: &NativeEngines) -> Box<dyn CheckpointPort> {
    Box::new(SpinePort::at(session_engine(dir, engines.reader()), dir))
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
        // The run is finished. Ctrl-C also exits when a rejected message has
        // restored a nonempty draft; Ctrl-D would edit that draft instead.
        inbox.send(Msg::Key(
            KeyEvent::new(KeyCode::Char('c')).with(Modifiers::CTRL),
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

async fn next_matching(
    events: &mut tokio::sync::mpsc::UnboundedReceiver<Msg>,
    predicate: impl Fn(&Msg) -> bool,
) -> Msg {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = events.recv().await.expect("live inbox");
            if predicate(&event) {
                return event;
            }
        }
    })
    .await
    .expect("the expected session event arrives")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_finished_loop_keeps_delivering_roadmap_and_conversation_requests() {
    let dir = project("true", "true", "true");
    repository(dir.path());
    let engines = host(&work_config(dir.path()), "finished-followup");
    engines
        .roadmap_add_chunk(
            "later".into(),
            "Future work".into(),
            "Future work".into(),
            ChunkStatus::Backlog,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .unwrap();
    let mut port = ledger(dir.path(), &engines);
    port.save(
        "before-followup",
        magistr_engine::CheckpointReason::Manual,
        "Before request",
    )
    .unwrap();
    let iteration =
        IterationLoop::open(dir.path(), dir.path().join("ROADMAP.md"), 2, Some(engines)).unwrap();
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (connector, seen) = fake_connector(FakeAcpAgent::new().reply("The plan is updated."));
    let captured = Arc::new(Mutex::new(Vec::new()));
    let inputs = Arc::clone(&captured);
    let connector = Connector::new(move |config: AgentConfig| {
        inputs.lock().unwrap().push(config.clone());
        let connector = connector.clone();
        async move { connector.connect(config).await }
    });
    let driver = LoopDriver::new(
        iteration,
        WorkflowConfig::load(dir.path()).unwrap(),
        Backend::Acp,
        inbox,
        receiver,
        connector,
    )
    .persisting(false)
    .checkpoints(port)
    .keep_open(true);
    let task = driver.spawn();
    next_matching(&mut events, |event| {
        matches!(event, Msg::Loop(LoopEvent::Ended(_)))
    })
    .await;
    assert!(
        !task.is_finished(),
        "the completed loop still owns message delivery"
    );
    commands
        .send(Cmd::Checkpoint(CheckpointCmd::Restore {
            id: "before-followup".into(),
        }))
        .unwrap();
    let restored = next_matching(&mut events, |event| matches!(event, Msg::Checkpoint(_))).await;
    assert!(
        matches!(
            restored,
            Msg::Checkpoint(magistr_cockpit::cockpit::CheckpointEvent::Restored { .. })
        ),
        "post-loop local controls share the loop's existing checkpoint history: {restored:?}"
    );
    for text in ["Add more ready work", "Include accessibility"] {
        commands.send(Cmd::RoadmapPrompt(text.to_owned())).unwrap();
        next_matching(&mut events, |event| {
            matches!(event, Msg::RoadmapRequestStarted)
        })
        .await;
        next_matching(&mut events, |event| {
            matches!(event, Msg::Agent(AgentEvent::Text(_)))
        })
        .await;
        next_matching(&mut events, |event| {
            matches!(event, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)))
        })
        .await;
    }
    assert_eq!(
        seen.lock().unwrap().len(),
        1,
        "planning follow-ups reuse their context"
    );
    for command in [
        Cmd::Prompt("Explain the last result".to_owned()),
        Cmd::RoadmapPrompt("Keep the earlier testing request".to_owned()),
        Cmd::Prompt("Continue your explanation".to_owned()),
    ] {
        commands.send(command).unwrap();
        next_matching(&mut events, |event| {
            matches!(event, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)))
        })
        .await;
    }
    // Quit must close both retained connections even while its sender lives.
    commands.send(Cmd::Quit).unwrap();
    let report = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        report.iterations(),
        0,
        "messages never start another work iteration"
    );
    assert_eq!(report.end(), &LoopEnd::NoReadyChunk);
    let observed = seen.lock().unwrap();
    assert_eq!(
        observed.len(),
        2,
        "changing purpose opens a separate conversation"
    );
    assert_eq!(
        observed[0].prompts(),
        [
            "Add more ready work",
            "Include accessibility",
            "Keep the earlier testing request",
        ]
    );
    assert_eq!(
        observed[1].prompts(),
        ["Explain the last result", "Continue your explanation"]
    );
    let configs = captured.lock().unwrap();
    assert!(configs[0].planning_only && !configs[0].workflow_permissions);
    assert!(!configs[1].planning_only);
    assert!(
        !configs[0]
            .allowed_tools
            .iter()
            .any(|name| name == "Bash" || name == "ship_start")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_messages_during_checks_are_rejected_with_the_original_draft() {
    let dir = project("sleep 0.2", "true", "true");
    let iteration = IterationLoop::with_max(dir.path(), dir.path().join("ROADMAP.md"), 1).unwrap();
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (connector, seen) = fake_connector(FakeAcpAgent::new().reply("Done."));
    let task = LoopDriver::new(
        iteration,
        WorkflowConfig::load(dir.path()).unwrap(),
        Backend::Acp,
        inbox,
        receiver,
        connector,
    )
    .persisting(false)
    .auto_pause(false)
    .spawn();
    next_matching(&mut events, |event| {
        matches!(event, Msg::Loop(LoopEvent::Stage(LoopStage::Gates)))
    })
    .await;
    commands
        .send(Cmd::Prompt("Keep this question".into()))
        .unwrap();
    let rejected = next_matching(&mut events, |event| {
        matches!(event, Msg::Agent(AgentEvent::PromptRejected { .. }))
    })
    .await;
    assert!(
        matches!(rejected, Msg::Agent(AgentEvent::PromptRejected { text, error })
        if text == "Keep this question" && error.contains("draft is kept"))
    );
    let report = tokio::time::timeout(Duration::from_secs(20), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(report.iterations(), 1);
    let observed = seen.lock().unwrap();
    assert_eq!(observed.len(), 1);
    assert_eq!(
        observed[0].prompts().len(),
        1,
        "the finished agent receives no lost follow-up"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn roadmap_requests_wait_for_work_verification_and_can_be_cancelled_while_queued() {
    for cancel in [false, true] {
        let dir = project("sleep 0.05", "true", "true");
        let iteration =
            IterationLoop::with_max(dir.path(), dir.path().join("ROADMAP.md"), 1).unwrap();
        let (inbox, mut events) = Inbox::channel();
        let (commands, receiver) = Commands::channel();
        let (connector, seen) = fake_connector(FakeAcpAgent::new().reply("Done."));
        let driver = LoopDriver::new(
            iteration,
            WorkflowConfig::load(dir.path()).unwrap(),
            Backend::Acp,
            inbox,
            receiver,
            connector,
        )
        .persisting(false)
        .auto_pause(false);
        commands
            .send(Cmd::RoadmapPrompt("Add testing work".into()))
            .unwrap();
        let task = driver.spawn();
        next_matching(&mut events, |event| {
            matches!(event, Msg::RoadmapRequestQueued)
        })
        .await;
        if cancel {
            commands.send(Cmd::CancelRoadmapPrompt).unwrap();
            next_matching(&mut events, |event| {
                matches!(event, Msg::RoadmapRequestCancelled)
            })
            .await;
        } else {
            next_matching(&mut events, |event| {
                matches!(event, Msg::Loop(LoopEvent::Stage(LoopStage::Gates)))
            })
            .await;
            next_matching(&mut events, |event| {
                matches!(event, Msg::RoadmapRequestStarted)
            })
            .await;
        }
        let report = tokio::time::timeout(Duration::from_secs(20), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(report.iterations(), 1);
        let observed = seen.lock().unwrap();
        assert_eq!(observed.len(), if cancel { 1 } else { 2 });
        if !cancel {
            assert_eq!(observed[1].prompts(), ["Add testing work"]);
        }
    }
}

#[tokio::test]
async fn interactive_roadmap_startup_failure_preserves_request_and_does_not_claim_started() {
    let dir = tempfile::tempdir().unwrap();
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let session = crate::orchestration::interactive_session::interactive_session(
        AgentConfig::new("", dir.path()),
        Connector::new(|_| async { Err(MagistrError::Acp("offline".into())) }),
        inbox,
        None,
    )
    .spawn(receiver);
    commands
        .send(Cmd::RoadmapPrompt("Add more work".into()))
        .unwrap();
    let event = next_matching(&mut events, |event| {
        matches!(
            event,
            Msg::RoadmapRequestStarted | Msg::Agent(AgentEvent::PromptRejected { .. })
        )
    })
    .await;
    assert!(
        matches!(event, Msg::Agent(AgentEvent::PromptRejected { text, error })
        if text == "Add more work" && error.contains("offline"))
    );
    drop(commands);
    session.await.unwrap().unwrap();
}

#[tokio::test]
async fn active_roadmap_request_stop_reaches_the_provider_without_a_work_iteration() {
    let dir = tempfile::tempdir().unwrap();
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (connector, seen) = fake_connector(FakeAcpAgent::new().stalling());
    let session = crate::orchestration::interactive_session::interactive_session(
        AgentConfig::new("", dir.path()),
        connector,
        inbox,
        None,
    )
    .spawn(receiver);
    commands
        .send(Cmd::RoadmapPrompt("Review the plan".into()))
        .unwrap();
    next_matching(&mut events, |event| {
        matches!(event, Msg::RoadmapRequestStarted)
    })
    .await;
    commands.send(Cmd::CancelRoadmapPrompt).unwrap();
    next_matching(&mut events, |event| {
        matches!(event, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::Cancelled)))
    })
    .await;
    drop(commands);
    session.await.unwrap().unwrap();
    assert_eq!(seen.lock().unwrap()[0].prompts(), ["Review the plan"]);
}

#[tokio::test]
async fn roadmap_connection_can_be_cancelled_or_closed_before_handshake_finishes() {
    for quit in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (inbox, mut events) = Inbox::channel();
        let (commands, receiver) = Commands::channel();
        let started = Arc::new(tokio::sync::Notify::new());
        let signal = Arc::clone(&started);
        let connector = Connector::new(move |_| {
            signal.notify_one();
            std::future::pending()
        });
        let session = crate::orchestration::interactive_session::interactive_session(
            AgentConfig::new("", dir.path()),
            connector,
            inbox,
            None,
        )
        .spawn(receiver);
        commands
            .send(Cmd::RoadmapPrompt("More ready work".into()))
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), started.notified())
            .await
            .unwrap();
        if quit {
            drop(commands);
        } else {
            commands.send(Cmd::CancelRoadmapPrompt).unwrap();
            let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(matches!(event, Msg::RoadmapRequestCancelled));
            drop(commands);
        }
        tokio::time::timeout(Duration::from_secs(1), session)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn local_project_controls_work_before_any_agent_is_connected() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let session = crate::orchestration::interactive_session::interactive_session(
        AgentConfig::new("", dir.path()),
        Connector::new(|_| async { panic!("local controls must not launch an agent") }),
        inbox,
        Some(crate::application::workflow_client::EngineClient::attach(
            engines.clone(),
        )),
    )
    .spawn(receiver);
    commands
        .send(Cmd::Engine(
            magistr_cockpit::cockpit::EngineCmd::StartChunk { id: "work".into() },
        ))
        .unwrap();
    next_matching(&mut events, |event| matches!(event, Msg::Engine(_))).await;
    assert_eq!(engines.roadmap_chunks()[0].status, ChunkStatus::InProgress);
    drop(commands);
    session.await.unwrap().unwrap();
}

#[tokio::test]
async fn plain_roadmap_keyboard_request_reaches_its_planning_session_and_stays_on_roadmap() {
    use magistr_cockpit::cockpit::{RequestPhase, StageTab};
    let dir = tempfile::tempdir().unwrap();
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(200), "Add testing work\r")
        .close(MS(700));
    let terminal = FakeTerminal::new(SIZE, script);
    let mut runtime = Runtime::new(
        terminal,
        Options::new(Env::new("xterm-kitty", false, None).with_screen_reader(true))
            .probe_timeout(MS(500)),
    );
    let commands = runtime.commands().unwrap();
    let (connector, seen) = fake_connector(FakeAcpAgent::new().reply("Testing work is ready."));
    let session = crate::orchestration::interactive_session::interactive_session(
        AgentConfig::new("", dir.path()),
        connector,
        runtime.inbox(),
        None,
    )
    .spawn(commands);
    let model = runtime
        .run(Model::new(SIZE).with_tab(StageTab::Roadmap))
        .await
        .unwrap();
    session.await.unwrap().unwrap();
    assert_eq!(seen.lock().unwrap()[0].prompts(), ["Add testing work"]);
    assert_eq!(model.tab(), StageTab::Roadmap);
    assert_eq!(model.roadmap_requests().len(), 1);
    assert_eq!(model.roadmap_requests()[0].phase, RequestPhase::Complete);
    assert!(
        model
            .turn()
            .rows()
            .iter()
            .any(|line| line.text.contains("Testing work is ready."))
    );
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
    // The loop's own writings are not source: the evidence capability
    // hashes the tree under the fixture's ignore file, as it would in a
    // real repository, and reads project.json as a config input regardless.
    std::fs::write(dir.path().join(".gitignore"), ".magistr/\ntas-data/\n").expect(".gitignore");
    dir
}

/// A two-archetype project: `api/` and `web/`, each with a test command
/// that passes only in its own directory (`test -f <marker>` against a
/// marker that exists there and nowhere else), both naming the project's
/// own `check` and `lint` (one run each, not one per archetype), a
/// required custom gate `smoke` and an optional custom gate `never` that
/// `all` does not run. The top-level `test` is green on purpose: a loop
/// resolving only the top level would never see `web_test` fail.
fn polyglot_project(web_test: &str, smoke: &str) -> tempfile::TempDir {
    let dir = project("true", "true", "true");
    for arch in ["api", "web"] {
        let root = dir.path().join(arch);
        std::fs::create_dir_all(&root).expect("archetype root");
        std::fs::write(root.join(format!("{arch}.marker")), "").expect("marker");
    }
    let archetype = |id: &str, language: &str, role: &str, test: &str| {
        serde_json::json!({
            "id": id, "name": id, "language": language, "root": id, "role": role,
            "quality_gate": {"check": "true", "test": test, "lint": "true"},
            "format": {"command": "true", "extensions": []},
            "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"}
        })
    };
    let path = dir.path().join(".magistr/project.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("project.json")).expect("json");
    config["archetypes"] = serde_json::json!([
        archetype("api", "rust", "backend", "test -f api.marker"),
        archetype("web", "typescript", "frontend", web_test),
    ]);
    config["custom_gates"] = serde_json::json!([
        {"name": "smoke", "command": smoke, "required": true},
        {"name": "never", "command": "exit 99", "required": false},
    ]);
    std::fs::write(&path, config.to_string()).expect("project.json");
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

/// The turn's notes and errors as the stream received them — source
/// lines, not the rows they wrap to, so a claim about what the loop
/// said does not depend on the stage's width.
fn notes(model: &Model) -> Vec<String> {
    model
        .turn()
        .lines()
        .iter()
        .filter(|l| matches!(l.kind(), LineKind::Note | LineKind::Error))
        .map(|l| l.text().to_owned())
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
        Some("loop: 1 iteration · Reached the limit of 1 attempt.")
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
    assert_eq!(status.ended(), Some("Reached the limit of 1 attempt."));
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
        Some("Work stopped: Reached the limit of 1 attempt.")
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
    assert_eq!(status.ended(), Some("Cancelled."));
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
    let config = NativeConfig::from_lookup(dir.path().to_path_buf(), |_| None)
        .with_data_dir(dir.path().join("tas-data"));
    let engines = host(&config, "loop-signal");
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
    let server = agent_side(&config, "loop-signal");
    let captured = server
        .signal_capture(SignalKind::Idea, "alice".to_owned(), "faster".to_owned())
        .expect("capture");

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
    let agent_side = agent_side(&work_config(dir.path()), "loop-goal");
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
    // The objective and its task are the agent's to set; the chunk itself
    // is never started by anyone but the driver.
    let engines = engines_with_active_task(dir.path());
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
        model.roadmap().chunk("work").map(|c| c.state),
        Some(magistr_cockpit::cockpit::ChunkState::Done),
        "the Roadmap tab folded the completion"
    );
    // A completion always names the task its evidence sits on: the one
    // the loop's gates were recorded on.
    assert_eq!(
        engines.roadmap_chunk("work").expect("the chunk").cross_refs,
        vec!["task:impl".to_owned()]
    );
}

/// With a task active in the trace, the loop's gates are recorded on that
/// task by name, and the judged completion names the same task as its
/// proof-of-ship reference — the evidence and the completion point at
/// each other.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_judged_completion_names_the_task_its_gates_were_recorded_on() {
    let dir = project("true", "true", "true");
    let engines = engines_with_active_task(dir.path());
    let judge = Scripted::new([Verdict::met("both criteria hold")]);
    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let (_, report, _) = run_with(
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
    engines.reload_ship();
    let checks = recorded(&engines);
    assert_eq!(checks.len(), 3, "check, test, lint: {checks:?}");
    assert!(
        engines
            .ship_tasks()
            .iter()
            .find(|t| t.id == "impl")
            .is_some_and(|t| t.checks.len() == 3),
        "every gate landed on the named task"
    );
    let done = engines.roadmap_chunk("work").expect("the chunk");
    assert_eq!(done.status, ChunkStatus::Done);
    assert_eq!(done.cross_refs, vec!["task:impl".to_owned()]);
}

/// On a polyglot project the loop runs the set completion verifies: one
/// spec per archetype command in the archetype's directory, the project's
/// own command once however many archetypes name it, and the required
/// custom gates — under the names and commands the contracts carry, so
/// the judged completion stands on the loop's own receipts.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_two_archetype_project_completes_through_the_loops_own_receipts() {
    let dir = polyglot_project("test -f web.marker", "true");
    let engines = engines_with_active_task(dir.path());
    let judge = Scripted::new([Verdict::met("both criteria hold")]);
    let script = Script::new().startup_replies(MS(0)).text(MS(4_000), "\x04");
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
    engines.reload_ship();
    let checks = recorded(&engines);
    let names: Vec<&str> = checks.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        ["check", "test:api", "test:web", "lint", "smoke"],
        "the contracts, in their order, deduplicated, without the optional gate: {names:?}"
    );
    for check in &checks {
        assert!(check.passed, "{} passed: {}", check.name, check.details);
        assert!(check.verified && check.required, "{}", check.name);
    }
    let api = checks
        .iter()
        .find(|c| c.name == "test:api")
        .expect("test:api");
    assert_eq!(
        api.command.as_deref(),
        Some("cd api && test -f api.marker"),
        "the archetype's command carries its directory, as the contract spells it"
    );
    let jobs_done = model.jobs_done();
    assert_eq!(jobs_done, 5, "five gates ran, none twice");
    engines.reload_roadmap();
    let done = engines.roadmap_chunk("work").expect("the chunk");
    assert_eq!(
        done.status,
        ChunkStatus::Done,
        "completion accepted the loop's receipts as its contracts"
    );
    assert_eq!(done.cross_refs, vec!["task:impl".to_owned()]);
    assert!(
        !notes(&model).iter().any(|n| n.contains("not recorded")),
        "nothing said it could not record"
    );
}

/// A required custom gate is part of the loop's set: red, it is a
/// disagreement with the judge and the chunk stays where it is.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_red_required_custom_gate_refuses_the_judged_completion() {
    let dir = polyglot_project("test -f web.marker", "exit 3");
    let engines = engines_with_active_task(dir.path());
    let judge = Scripted::new([Verdict::met("looks done to me")]);
    let script = Script::new().startup_replies(MS(0)).text(MS(4_000), "\x04");
    let (model, _, _) = run_with(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
        Some(engines.clone()),
        Some(judge),
    )
    .await;
    engines.reload_roadmap();
    assert_ne!(
        engines.roadmap_chunk("work").expect("the chunk").status,
        ChunkStatus::Done,
        "a red required custom gate cannot be judged green"
    );
    let stage = stage_text(&model);
    assert!(
        stage.contains("but the gates disagree: smoke failed — the chunk stays in progress"),
        "{stage}"
    );
    engines.reload_ship();
    let smoke = recorded(&engines)
        .into_iter()
        .find(|c| c.name == "smoke")
        .expect("the custom gate is on the record");
    assert!(!smoke.passed);
    assert_eq!(smoke.exit_code, Some(3));
    assert_eq!(smoke.command.as_deref(), Some("exit 3"));
}

/// A gate configuration that cannot be resolved when the set is due —
/// the agent's turn broke project.json — is a failed iteration, not a
/// green one with "no quality gates configured": completion would refuse
/// the same configuration, so the loop says so and pauses on it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_set_that_cannot_be_resolved_fails_the_iteration_instead_of_passing_empty() {
    let dir = project("true", "true", "true");
    let engines = engines_with_active_task(dir.path());
    let (inner, seen) = fake_connector(FakeAcpAgent::new().reply("broke the config"));
    let inner = Arc::new(inner);
    let config = dir.path().join(".magistr/project.json");
    let connector = Connector::new(move |cfg| {
        std::fs::write(&config, "{ not json").expect("break project.json");
        let inner = Arc::clone(&inner);
        async move { inner.connect(cfg).await }
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
    let status = model.loop_status().expect("the strip saw the loop");
    assert!(
        status.is_paused(),
        "an unresolved set pauses like a red one"
    );
    let stage = stage_text(&model);
    assert!(
        stage.contains("quality gates not resolved — nothing ran: parse gate configuration"),
        "{stage}"
    );
    assert!(
        !stage.contains("no quality gates configured") && !stage.contains("gates passed"),
        "never read as green or unconfigured: {stage}"
    );
    assert_eq!(model.jobs_done(), 0, "nothing ran");
}

/// A required archetype gate cannot disappear behind a green top-level
/// command: the top-level `test` is `true`, the web archetype's is red,
/// and the loop runs the archetype's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_required_archetype_gate_cannot_disappear_behind_a_green_top_level_command() {
    let dir = polyglot_project("exit 5", "true");
    let engines = engines_with_active_task(dir.path());
    let judge = Scripted::new([Verdict::met("looks done to me")]);
    let script = Script::new().startup_replies(MS(0)).text(MS(4_000), "\x04");
    let (model, _, _) = run_with(
        dir.path(),
        1,
        script,
        FakeAcpAgent::new().reply("did the thing"),
        false,
        Some(engines.clone()),
        Some(judge),
    )
    .await;
    let status = model.loop_status().expect("the strip saw the loop");
    assert_eq!(status.gates_passed(), Some(false));
    engines.reload_roadmap();
    assert_ne!(
        engines.roadmap_chunk("work").expect("the chunk").status,
        ChunkStatus::Done
    );
    engines.reload_ship();
    let web = recorded(&engines)
        .into_iter()
        .find(|c| c.name == "test:web")
        .expect("the archetype's gate is on the record under the contract's name");
    assert!(!web.passed);
    assert_eq!(web.exit_code, Some(5));
    assert_eq!(web.command.as_deref(), Some("cd web && exit 5"));
    assert!(
        stage_text(&model).contains("the gates disagree: test:web failed"),
        "{}",
        stage_text(&model)
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
        Some("loop: 1 iteration · Your input is needed: the two criteria contradict each other")
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
            "Work stopped: Your input is needed: the two criteria contradict each other"
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
    // Completion crosses the owner's event stream, and the driver waits
    // for the cockpit's terminal event before closing the relay. Text on
    // either side of the echo therefore arrives before loop cleanup.
    assert!(
        model
            .turn()
            .rows()
            .iter()
            .any(|r| r.text.starts_with("working toward:")),
        "the agent's goal-loop words stream onto the stage: {stage}"
    );
    assert!(
        model
            .turn()
            .rows()
            .iter()
            .any(|r| r.text.contains("goal work finished")),
        "post-echo text preceding completion also reaches the stage: {stage}"
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

/// The loop's gates are the contracts native completion verifies,
/// resolved through the same evidence capability completion asks —
/// never the top-level `quality_gate` alone, and never a second
/// resolution beside the capability's.
#[test]
fn the_loop_resolves_its_gates_through_the_evidence_capability() {
    let driver = include_str!("driver.rs");
    assert!(
        driver.contains(".config().evidence())")
            && driver.contains(".required_gates()")
            && driver.contains("WorkspaceEvidence::gate_contracts("),
        "the loop asks the admitted writer's evidence capability for the project's required gates, and the one resolver behind it without a writer"
    );
    assert!(
        !driver.contains("gate_command("),
        "the loop does not read the top-level quality_gate on its own"
    );
    assert!(
        !driver.contains("resolve_gates("),
        "the loop does not resolve gates beside the capability"
    );
}

/// A ship trace with one active task, the way the agent's own
/// `magistr mcp-serve` leaves one behind for the loop's gates to record
/// into.
fn engines_with_active_task(dir: &Path) -> NativeEngines {
    let engines = engines_with_work(dir);
    engines
        .ship_set_objective("do the work".into(), vec![], vec![], "chunk:work".into())
        .expect("objective");
    engines
        .ship_add_task(
            "impl".into(),
            "implement".into(),
            magistr_workflow::ship::domain::task::TaskType::Implement,
            None,
        )
        .expect("add task");
    engines.ship_start_task("impl").expect("start the task");
    engines
}

fn recorded(engines: &NativeEngines) -> Vec<magistr_workflow::ship::domain::check::Check> {
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
        said[0].contains("no workflow engines"),
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
        !said[0].contains("no workflow engines"),
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
    let agent_side = agent_side(&work_config(dir.path()), "loop-goal");
    let (inner, seen) = fake_connector(FakeAcpAgent::new().reply("shipped it"));
    let inner = Arc::new(inner);
    let root = dir.path().to_path_buf();
    let connector = Connector::new(move |config| {
        // The agent's receipts claim lint green; the loop's own run of it
        // is what finds it red.
        agent_completes_work(
            &agent_side,
            &root,
            [("check", "true"), ("test", "true"), ("lint", "exit 3")],
        );
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
    let agent_side = agent_side(&work_config(dir.path()), "loop-goal");
    let (inner, seen) = fake_connector(FakeAcpAgent::new().reply("shipped it"));
    let inner = Arc::new(inner);
    let root = dir.path().to_path_buf();
    let connector = Connector::new(move |config| {
        agent_completes_work(
            &agent_side,
            &root,
            [("check", "true"), ("test", "true"), ("lint", "true")],
        );
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recorded_verification_survives_finalization_without_running_unrelated_gates() {
    use magistr_workflow::ship::domain::objective::VerificationMode;
    use magistr_workflow::ship::domain::task::TaskType;

    for case in [
        "focused-completed",
        "full-completed",
        "focused-active",
        "completed-then-committed",
        "stale",
        "missing",
        "foreign",
        "failed-check",
    ] {
        let full = case == "full-completed";
        let valid = matches!(
            case,
            "focused-completed" | "full-completed" | "focused-active" | "completed-then-committed"
        );
        let command = if full {
            "true"
        } else {
            "echo unexpected > unexpected-gate; exit 9"
        };
        let dir = project(command, command, command);
        std::fs::write(dir.path().join("work.txt"), "verified work").unwrap();
        let engines = engines_with_work(dir.path());
        let agent_side = agent_side(&work_config(dir.path()), "loop-goal");
        let (inner, seen) = fake_connector(FakeAcpAgent::new().reply("Work finished."));
        let root = dir.path().to_path_buf();
        let connector = Connector::new(move |config| {
            agent_side.roadmap_start_chunk("work").unwrap();
            agent_side
                .ship_set_objective_with_verification(
                    "do the work".into(),
                    vec![],
                    vec![],
                    if case == "foreign" {
                        "chunk:other"
                    } else {
                        "chunk:work"
                    }
                    .into(),
                    if full {
                        VerificationMode::Full
                    } else {
                        VerificationMode::Focused
                    },
                )
                .unwrap();
            agent_side
                .ship_add_task("verify".into(), "verify".into(), TaskType::Test, None)
                .unwrap();
            agent_side.ship_start_task("verify").unwrap();
            let names = if case == "missing" {
                vec![]
            } else if full {
                vec!["check", "test", "lint"]
            } else {
                vec!["document links"]
            };
            for name in names {
                crate::application::AdmittedCheck::admit(
                    &agent_side,
                    &root,
                    Some("verify"),
                    name,
                    "true".into(),
                    true,
                )
                .unwrap()
                .settle(
                    &agent_side,
                    magistr_engine::native::CheckExecution::Exited {
                        code: if case == "failed-check" { 1 } else { 0 },
                    },
                    String::new(),
                )
                .unwrap();
            }
            if case != "focused-active" {
                agent_side.ship_complete_task("verify", vec![]).unwrap();
                agent_side.ship_finalize(vec![], None).unwrap();
                if valid {
                    agent_side
                        .roadmap_complete_chunk("work", Some("task:verify"))
                        .unwrap();
                }
            }
            // Stale: the source moved before anything accepted the evidence.
            // Committed: the item was completed on current evidence, and then
            // the commit's own hooks regenerated tracked files.
            if matches!(case, "stale" | "completed-then-committed") {
                std::fs::write(root.join("work.txt"), "changed after verification").unwrap();
            }
            let inner = inner.clone();
            async move { inner.connect(config).await }
        });
        let judge = Scripted::new([Verdict::met("acceptance satisfied")]);
        let script = Script::new().startup_replies(MS(0));
        let (model, report, _) = run_over(
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
        let stage = stage_text(&model);
        assert_eq!(model.jobs_done(), 0, "{case}: no gate command should run");
        assert!(!dir.path().join("unexpected-gate").exists(), "{case}");
        assert!(!stage.contains("gates not recorded"), "{case}: {stage}");
        assert!(model.loop_status().unwrap().ended().is_some(), "{case}");
        if valid {
            assert!(
                !matches!(report.end(), LoopEnd::Failed(_)),
                "{case}: {report}"
            );
            assert!(
                // Work still open is verified against the tree as it is now;
                // work the agent completed was accepted at its completion.
                stage.contains(if case == "focused-active" {
                    "Recorded checks still verify this work"
                } else {
                    "Completed on its recorded checks"
                }),
                "{case}: {stage}"
            );
            assert!(
                !stage.contains("no quality gates configured"),
                "{case}: {stage}"
            );
            assert_eq!(
                engines.roadmap_chunk("work").unwrap().status,
                ChunkStatus::Done,
                "{case}"
            );
            let expected_checks = if full { 3 } else { 1 };
            assert_eq!(
                recorded(&engines).len(),
                expected_checks,
                "{case}: no duplicate receipts"
            );
            if case == "focused-active" {
                let asked = judge.asked();
                assert_eq!(asked.len(), 1);
                assert_eq!(asked[0].recorded_checks, ["document links"]);
                assert!(asked[0].prompt().contains("recorded check; current source"));
                assert!(!asked[0].prompt().contains("no gates are configured"));
            } else {
                assert!(
                    judge.asked().is_empty(),
                    "{case}: completed work is not judged twice"
                );
                assert!(engines.ship_active_task_id().is_none(), "{case}");
            }
        } else {
            assert!(
                matches!(report.end(), LoopEnd::Failed(_)),
                "{case}: {report}"
            );
            assert!(
                judge.asked().is_empty(),
                "{case}: missing evidence never reaches the judge"
            );
            assert_ne!(
                engines.roadmap_chunk("work").unwrap().status,
                ChunkStatus::Done,
                "{case}"
            );
            assert!(
                !stage.contains("Recorded checks still verify this work"),
                "{case}: {stage}"
            );
            if case == "stale" {
                // Said once, for a person: which checks and what to do, with
                // no receipt, run or task identifier on the screen.
                let said = "The project's files changed after these checks ran, so they no longer prove this work: document links. Run them again, then continue.";
                assert_eq!(stage.matches("error:").count(), 1, "{stage}");
                assert!(stage.contains(&format!("error: {said}")), "{stage}");
                assert_eq!(stage.matches(said).count(), 1, "said once: {stage}");
                assert!(stage.ends_with("Work stopped."), "{stage}");
                for internal in ["receipt", "(run ", "task `"] {
                    assert!(!stage.contains(internal), "{internal}: {stage}");
                }
            }
        }
    }
}

/// An agent may split the item it was given and ship one part (`/next`
/// tells it to when the item is too large). The iteration's work is then
/// the part the feed saw completed: its recorded checks verify it, and the
/// judge is not asked whether the remaining item is met on the part's
/// evidence.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_item_the_agent_split_is_verified_by_the_part_it_completed() {
    use magistr_workflow::ship::domain::objective::VerificationMode;
    use magistr_workflow::ship::domain::task::TaskType;

    let unexpected = "echo unexpected > unexpected-gate; exit 9";
    let dir = project(unexpected, unexpected, unexpected);
    let engines = engines_with_work(dir.path());
    let agent_side = agent_side(&work_config(dir.path()), "loop-goal");
    let (inner, seen) = fake_connector(FakeAcpAgent::new().reply("Shipped the first part."));
    let root = dir.path().to_path_buf();
    let connector = Connector::new(move |config| {
        agent_side
            .roadmap_add_chunk(
                "work-part".to_owned(),
                "The first part of the work".to_owned(),
                "work part".to_owned(),
                ChunkStatus::Pending,
                90,
                String::new(),
                vec!["the part works".to_owned()],
                vec![],
                None,
                None,
            )
            .unwrap();
        agent_side.roadmap_start_chunk("work-part").unwrap();
        agent_side
            .ship_set_objective_with_verification(
                "do the first part".into(),
                vec![],
                vec![],
                "chunk:work-part".into(),
                VerificationMode::Focused,
            )
            .unwrap();
        agent_side
            .ship_add_task("verify".into(), "verify".into(), TaskType::Test, None)
            .unwrap();
        agent_side.ship_start_task("verify").unwrap();
        crate::application::AdmittedCheck::admit(
            &agent_side,
            &root,
            Some("verify"),
            "part tests",
            "true".into(),
            true,
        )
        .unwrap()
        .settle(
            &agent_side,
            magistr_engine::native::CheckExecution::Exited { code: 0 },
            String::new(),
        )
        .unwrap();
        agent_side.ship_complete_task("verify", vec![]).unwrap();
        agent_side.ship_finalize(vec![], None).unwrap();
        agent_side
            .roadmap_complete_chunk("work-part", Some("task:verify"))
            .unwrap();
        let inner = inner.clone();
        async move { inner.connect(config).await }
    });
    let judge = Scripted::new([Verdict::met("would wrongly close the whole item")]);
    let script = Script::new().startup_replies(MS(0));
    let (model, report, _) = run_over(
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
    let stage = stage_text(&model);
    assert!(
        !matches!(report.end(), LoopEnd::Failed(_)),
        "{report}: {stage}"
    );
    assert!(
        stage.contains("Completed on its recorded checks: part tests"),
        "{stage}"
    );
    assert!(!stage.contains("belong to different work"), "{stage}");
    assert!(!dir.path().join("unexpected-gate").exists());
    engines.reload_roadmap();
    assert_eq!(
        engines.roadmap_chunk("work-part").unwrap().status,
        ChunkStatus::Done
    );
    assert_ne!(
        engines.roadmap_chunk("work").unwrap().status,
        ChunkStatus::Done,
        "the rest of the item is still to do"
    );
    assert!(
        judge.asked().is_empty(),
        "the remaining item is not judged on the part's evidence"
    );
    assert!(stage.contains("work-part"), "{stage}");
}

/// A flake passed, so it is not evidence of anything to act on.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_flake_never_reopens_a_chunk() {
    let flake = "if [ -e flake.marker ]; then exit 0; else touch flake.marker; exit 9; fi";
    let dir = project("true", flake, "true");
    let engines = engines_with_work(dir.path());
    let agent_side = agent_side(&work_config(dir.path()), "loop-goal");
    let (inner, seen) = fake_connector(FakeAcpAgent::new());
    let inner = Arc::new(inner);
    let root = dir.path().to_path_buf();
    let connector = Connector::new(move |config| {
        agent_completes_work(
            &agent_side,
            &root,
            [("check", "true"), ("test", flake), ("lint", "true")],
        );
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
    let agent_side = agent_side(&work_config(dir.path()), "loop-goal");
    let judge = Scripted::new([Verdict::met("I shipped it")]);
    let (inner, seen) = fake_connector(FakeAcpAgent::new().reply("shipped it"));
    let inner = Arc::new(inner);
    let root = dir.path().to_path_buf();
    let connector = Connector::new(move |config| {
        agent_completes_work(
            &agent_side,
            &root,
            [("check", "true"), ("test", "true"), ("lint", "exit 3")],
        );
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

// ── Durable operations ──────────────────────────────────────────────

/// The checkpoint the loop left on disk, read back the way recovery reads it.
fn checkpoint(dir: &Path) -> crate::workflow::iteration::IterationState {
    let json = std::fs::read_to_string(dir.join(".magistr").join("loop-state.json"))
        .expect("the loop persisted its checkpoint");
    serde_json::from_str(&json).expect("a checkpoint recovery would accept")
}

/// Every operation the iteration dispatched — the attempt's reservation,
/// the provider dispatch, one receipt per gate, the two native mutations
/// the judged completion needs and the frame that announced it — is in
/// the checkpoint with a settled outcome, scoped to one run, so a crash
/// at any point between them would have left recovery a record to ask
/// about rather than a guess.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_judged_iteration_leaves_every_operation_settled_in_the_checkpoint() {
    use crate::workflow::iteration::{OperationKind, OperationOutcome};
    let dir = project("true", "true", "true");
    let engines = engines_with_active_task(dir.path());
    let judge = Scripted::new([Verdict::met("both criteria hold")]);
    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let (_, report, _) = run_with(
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
    let state = checkpoint(dir.path());
    assert_eq!(state.run_id.project().as_str(), "loop-goal");
    let mut seen: Vec<(OperationKind, String, OperationOutcome, Option<String>)> = state
        .operations
        .records()
        .iter()
        .map(|r| (r.kind, r.target.clone(), r.outcome, r.external.clone()))
        .collect();
    seen.sort_by(|a, b| (a.0.as_str(), &a.1).cmp(&(b.0.as_str(), &b.1)));
    let expected = [
        (OperationKind::NativeMutation, "complete_chunk:work"),
        (OperationKind::NativeMutation, "start_chunk:work"),
        (OperationKind::ProviderDispatch, "dispatch"),
        (OperationKind::Publication, "frame:chunk_completed:work"),
        (OperationKind::Receipt, "gate:check@task:impl"),
        (OperationKind::Receipt, "gate:lint@task:impl"),
        (OperationKind::Receipt, "gate:test@task:impl"),
        (OperationKind::Reservation, "attempt:0"),
    ];
    assert_eq!(
        seen.iter()
            .map(|(kind, target, ..)| (*kind, target.as_str()))
            .collect::<Vec<_>>(),
        expected
    );
    for (kind, target, outcome, external) in &seen {
        assert_eq!(*outcome, OperationOutcome::Completed, "{kind} {target}");
        if *kind == OperationKind::Receipt {
            // The record names the durable receipt it recorded, and that
            // receipt is settled and applied on the task the target names.
            let id = external
                .as_deref()
                .and_then(|handle| handle.strip_prefix("receipt:"))
                .unwrap_or_else(|| panic!("{kind} {target} carries no receipt id: {external:?}"));
            let receipt = engines
                .ship_receipt(id)
                .expect("the ledger reads")
                .unwrap_or_else(|| panic!("receipt {id} for {target} is in the ledger"));
            assert_eq!(receipt.target.task_id, "impl");
            assert!(target.starts_with(&format!("gate:{}@", receipt.gate)));
            assert!(receipt.is_applied(), "{target}: {:?}", receipt.settlement);
        } else {
            assert_eq!(
                external.as_deref(),
                None,
                "an in-process fake spawns no runtime, so no handle is claimed for {kind} {target}"
            );
        }
    }
    for record in state.operations.records() {
        assert_eq!(record.attempt.command().run(), &state.run_id);
        assert_eq!(record.attempt.number().get(), 1);
    }
    assert_eq!(state.operations.unsettled().count(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failures_before_the_first_iteration_are_reported_without_connecting_an_agent() {
    for fail_persistence in [false, true] {
        let dir = project("true", "true", "true");
        let spec = dir.path().join("ROADMAP.md");
        let wf = WorkflowConfig::load(dir.path()).unwrap();
        let iteration = IterationLoop::open(dir.path(), &spec, 1, None).unwrap();
        if fail_persistence {
            std::fs::create_dir(dir.path().join(".magistr/loop-state.json")).unwrap();
        } else {
            std::fs::remove_file(&spec).unwrap();
        }
        let (connector, seen) = fake_connector(FakeAcpAgent::new().reply("must not be asked"));
        let mut runtime = Runtime::new(
            FakeTerminal::new(SIZE, Script::new().startup_replies(MS(0))),
            Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500)),
        );
        let commands = runtime.commands().unwrap();
        let driver = LoopDriver::new(
            iteration,
            wf,
            Backend::Acp,
            runtime.inbox(),
            commands,
            connector,
        );
        let (model, report) =
            tokio::time::timeout(Duration::from_secs(5), run_cockpit(runtime, driver))
                .await
                .expect("startup failure is visible without an iteration");
        let LoopEnd::Failed(reason) = report.end() else {
            panic!("startup must fail: {:?}", report.end());
        };
        assert!(reason.contains(if fail_persistence {
            "could not checkpoint attempt"
        } else {
            "pre-iteration check failed"
        }));
        assert_eq!(report.iterations(), 0);
        assert!(seen.lock().unwrap().is_empty());
        assert!(
            model.loop_status().is_none(),
            "no fake first iteration was started"
        );
        let errors: Vec<_> = model
            .turn()
            .lines()
            .iter()
            .filter(|line| matches!(line.kind(), LineKind::Error))
            .collect();
        assert_eq!(errors.len(), 1, "one terminal failure is emitted");
        assert_eq!(errors[0].text(), format!("error: {reason}"));
        // The reason is the error above; the closing note does not say it twice.
        assert!(notes(&model).contains(&"Work stopped.".to_owned()));
        assert_eq!(
            model.turn_state(),
            magistr_cockpit::cockpit::TurnState::AwaitingInput
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_opening_attachment_ends_the_loop_with_a_visible_failure() {
    let dir = project("true", "true", "true");
    std::fs::write(
        dir.path().join("ROADMAP.md"),
        "- [ ] Read @missing-loop-startup.txt before continuing\n",
    )
    .unwrap();
    let (model, report, prompts) = tokio::time::timeout(
        Duration::from_secs(5),
        run(
            dir.path(),
            1,
            Script::new().startup_replies(MS(0)),
            FakeAcpAgent::new().reply("must not be asked"),
            false,
        ),
    )
    .await
    .expect("a rejected opening message ends without waiting for a provider response");
    let LoopEnd::Failed(reason) = report.end() else {
        panic!(
            "message rejection must fail the loop visibly: {:?}",
            report.end()
        );
    };
    assert!(reason.contains("The message could not be sent"));
    assert!(reason.contains("missing-loop-startup.txt"));
    assert_eq!(report.iterations(), 0);
    assert!(
        prompts.is_empty(),
        "no invalid message reached the provider"
    );
    assert_eq!(
        model.jobs_done(),
        0,
        "checks must not run after a rejected message"
    );
    assert!(stage_text(&model).contains("missing-loop-startup.txt"));
    assert!(
        model
            .loop_status()
            .unwrap()
            .ended()
            .unwrap()
            .contains("could not be sent")
    );
    assert!(model.prompt().text().contains("@missing-loop-startup.txt"));
    let saved = checkpoint(dir.path());
    assert_eq!(
        saved.run.terminal_reason,
        Some(magistr_engine::workflow::StopReason::Failed)
    );
    assert!(
        saved
            .operations
            .records()
            .iter()
            .any(|record| record.kind == OperationKind::ProviderDispatch
                && record.outcome == OperationOutcome::Failed)
    );
    assert_eq!(saved.operations.unsettled().count(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quitting_native_work_and_restarting_resumes_it_before_any_pending_item() {
    use magistr_engine::workflow::StopReason;

    for add_pending in [false, true] {
        let dir = project("true", "true", "true");
        let engines = engines_with_work(dir.path());
        let spec = dir.path().join("ROADMAP.md");
        let previous =
            IterationLoop::start_or_recover(dir.path(), &spec, 3, Some(engines.clone())).unwrap();
        let previous_run = previous.state().run_id.clone();
        previous.persist().unwrap();
        let (inner, seen) = fake_connector(FakeAcpAgent::new().stalling());
        let inner = Arc::new(inner);
        let writer = engines.clone();
        let connector = Connector::new(move |config| {
            writer.roadmap_start_chunk("work").unwrap();
            let inner = Arc::clone(&inner);
            async move { inner.connect(config).await }
        });
        let terminal = FakeTerminal::new(SIZE, Script::new().startup_replies(MS(0)));
        let mut runtime = Runtime::new(
            terminal,
            Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500)),
        );
        let commands = runtime.commands().unwrap();
        let driver = LoopDriver::new(
            previous,
            WorkflowConfig::load(dir.path()).unwrap(),
            Backend::Acp,
            runtime.inbox(),
            commands,
            connector,
        )
        .auto_pause(false)
        .persisting(true);
        let inbox = runtime.inbox();
        let quit = tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    if seen
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|agent| !agent.prompts().is_empty())
                    {
                        break;
                    }
                    tokio::time::sleep(MS(10)).await;
                }
            })
            .await
            .expect("the agent received its opening prompt");
            inbox.send(Msg::Key(
                KeyEvent::new(KeyCode::Char('d')).with(Modifiers::CTRL),
            ));
        });
        let (_, report) = run_cockpit(runtime, driver).await;
        quit.await.unwrap();
        assert_eq!(report.end(), &LoopEnd::CockpitClosed);
        let stopped = checkpoint(dir.path());
        assert_eq!(stopped.run.terminal_reason, Some(StopReason::Cancelled));
        assert_eq!(stopped.operations.unsettled().count(), 0);
        assert_eq!(stopped.native_chunk_id.as_deref(), Some("work"));
        assert!(
            stopped
                .operations
                .records()
                .iter()
                .any(
                    |operation| operation.kind == OperationKind::ProviderDispatch
                        && operation.outcome == OperationOutcome::Cancelled
                )
        );
        assert_eq!(
            engines.roadmap_chunk("work").unwrap().status,
            ChunkStatus::InProgress
        );
        if add_pending {
            engines
                .roadmap_add_chunk(
                    "new-work".into(),
                    "A different ready item".into(),
                    "other".into(),
                    ChunkStatus::Pending,
                    1,
                    String::new(),
                    vec!["new acceptance".into()],
                    vec![],
                    None,
                    None,
                )
                .unwrap();
            assert_eq!(engines.roadmap_next().unwrap().id, "new-work");
        } else {
            assert!(
                engines.roadmap_next().is_none(),
                "ready-only selection cannot resume work"
            );
        }

        let requested =
            IterationLoop::start_or_recover(dir.path(), &spec, 1, Some(engines.clone())).unwrap();
        assert_ne!(requested.state().run_id, previous_run);
        requested.persist().unwrap();
        let (connector, seen) =
            fake_connector(FakeAcpAgent::new().reply("Continuing the retry work."));
        let terminal = FakeTerminal::new(SIZE, Script::new().startup_replies(MS(0)));
        let mut runtime = Runtime::new(
            terminal,
            Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500)),
        );
        let commands = runtime.commands().unwrap();
        let judge = Scripted::new([Verdict::not_yet("Finish the remaining check.")]);
        let driver = LoopDriver::new(
            requested,
            WorkflowConfig::load(dir.path()).unwrap(),
            Backend::Acp,
            runtime.inbox(),
            commands,
            connector,
        )
        .auto_pause(false)
        .persisting(true)
        .goal(GoalMode::Native, Some(judge.clone()), None);
        let (model, report) = run_cockpit(runtime, driver).await;
        assert_eq!(report.iterations(), 1);
        assert_eq!(report.end(), &LoopEnd::MaxIterations(1));
        let advanced = checkpoint(dir.path());
        assert_eq!(advanced.run.completed_attempts, 1);
        assert_eq!(
            advanced.native_chunk_id.as_deref(),
            Some("work"),
            "advancing an attempt keeps its unfinished item for the next request"
        );
        let prompts: Vec<_> = seen
            .lock()
            .unwrap()
            .iter()
            .flat_map(|agent| agent.prompts())
            .collect();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].contains("Chunk work"), "{}", prompts[0]);
        assert!(!prompts[0].contains("A different ready item"));
        assert_eq!(judge.asked().len(), 1);
        assert_eq!(judge.asked()[0].chunk, "work");
        assert!(
            notes(&model)
                .iter()
                .any(|line| line == "Resuming Chunk work")
        );
        assert!(
            model
                .turn()
                .rows()
                .iter()
                .any(|row| row.text == "Continuing the retry work.")
        );
        assert_eq!(
            engines.roadmap_chunk("work").unwrap().status,
            ChunkStatus::InProgress
        );
        if add_pending {
            assert_eq!(
                engines.roadmap_chunk("new-work").unwrap().status,
                ChunkStatus::Pending
            );
        }
    }
}

#[tokio::test]
async fn the_selected_judge_target_is_not_replaced_by_another_in_progress_item() {
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    engines
        .roadmap_add_chunk(
            "other".into(),
            "Other work".into(),
            "other".into(),
            ChunkStatus::Pending,
            1,
            String::new(),
            vec!["done".into()],
            vec![],
            None,
            None,
        )
        .unwrap();
    engines.roadmap_start_chunk("other").unwrap();
    let iter_loop = IterationLoop::open(
        dir.path(),
        dir.path().join("ROADMAP.md"),
        1,
        Some(engines.clone()),
    )
    .unwrap();
    let (connector, _) = fake_connector(FakeAcpAgent::new());
    let terminal = FakeTerminal::new(SIZE, Script::new());
    let mut runtime = Runtime::new(terminal, Options::new(Env::new("dumb", false, None)));
    let commands = runtime.commands().unwrap();
    let mut driver = LoopDriver::new(
        iter_loop,
        WorkflowConfig::load(dir.path()).unwrap(),
        Backend::Acp,
        runtime.inbox(),
        commands,
        connector,
    );
    driver.target = Some("work".into());
    assert_eq!(driver.target_chunk(&engines).unwrap().id, "work");
    driver.target = Some("missing".into());
    assert!(
        driver.target_chunk(&engines).is_none(),
        "a removed selection cannot redirect work"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_new_loop_request_after_cancellation_sends_the_opening_prompt() {
    let dir = project("true", "true", "true");
    let spec = dir.path().join("ROADMAP.md");
    let mut previous = IterationLoop::open(dir.path(), &spec, 3, None).unwrap();
    previous.pre_iteration_check().unwrap();
    let dispatch = previous
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
        .unwrap()
        .unwrap();
    previous
        .settle_operation(&dispatch, OperationOutcome::Completed)
        .unwrap();
    previous.stop(magistr_engine::workflow::StopReason::Cancelled);
    previous.persist().unwrap();

    let requested = IterationLoop::start_or_recover(dir.path(), &spec, 1, None).unwrap();
    requested.persist().unwrap();
    let (connector, seen) = fake_connector(FakeAcpAgent::new().reply("The next item is underway."));
    let terminal = FakeTerminal::new(SIZE, Script::new().startup_replies(MS(0)));
    let mut runtime = Runtime::new(
        terminal,
        Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500)),
    );
    let commands = runtime.commands().unwrap();
    let driver = LoopDriver::new(
        requested,
        WorkflowConfig::load(dir.path()).unwrap(),
        Backend::Acp,
        runtime.inbox(),
        commands,
        connector,
    )
    .auto_pause(false);
    let (model, report) = run_cockpit(runtime, driver).await;
    assert_eq!(report.iterations(), 1);
    assert_eq!(report.end(), &LoopEnd::MaxIterations(1));
    let agents = seen.lock().unwrap();
    assert_eq!(
        agents.len(),
        1,
        "the new request connected exactly one agent"
    );
    let prompts = agents[0].prompts();
    assert_eq!(
        prompts.len(),
        1,
        "the opening message reached the real ACP path"
    );
    assert!(prompts[0].contains("make it work"));
    assert!(
        model
            .turn()
            .rows()
            .iter()
            .any(|row| row.text == "The next item is underway.")
    );
    assert_eq!(
        model.loop_status().unwrap().iteration_text(),
        "iteration 1/1"
    );
    assert_ne!(checkpoint(dir.path()).run_id, previous.state().run_id);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_accepted_dispatch_retries_at_two_with_a_durable_marker_before_connect() {
    use magistr_core::workflow::iteration::DispatchPhase;
    let dir = project("true", "true", "true");
    let spec = dir.path().join("ROADMAP.md");
    let mut previous = IterationLoop::open(dir.path(), &spec, 1, None).unwrap();
    previous.pre_iteration_check().unwrap();
    let first = previous
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
        .unwrap()
        .unwrap();
    previous.persist().unwrap();
    let recovered = IterationLoop::open_or_recover(dir.path(), &spec, 1, None).unwrap();
    let (inner, seen) = fake_connector(FakeAcpAgent::new().reply("recovered"));
    let root = dir.path().to_path_buf();
    let connector = Connector::new(move |config| {
        let state = checkpoint(&root);
        let dispatch = state.operations.records().last().unwrap();
        assert_eq!(dispatch.attempt.command(), first.command());
        assert_eq!(dispatch.attempt.number().get(), 2);
        assert_eq!(dispatch.outcome, OperationOutcome::Running);
        assert_eq!(dispatch.dispatch_phase, Some(DispatchPhase::Started));
        assert!(dispatch.external.is_none());
        let inner = inner.clone();
        async move { inner.connect(config).await }
    });
    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let terminal = FakeTerminal::new(SIZE, script);
    let options = Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500));
    let mut runtime = Runtime::new(terminal, options);
    let commands = runtime.commands().unwrap();
    let driver = LoopDriver::new(
        recovered,
        WorkflowConfig::load(dir.path()).unwrap(),
        Backend::Acp,
        runtime.inbox(),
        commands,
        connector,
    )
    .auto_pause(false);
    let (_, report) = run_cockpit(runtime, driver).await;
    assert_eq!(report.iterations(), 1);
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_failed_dispatch_checkpoint_never_calls_the_connector() {
    let dir = project("true", "true", "true");
    let il = IterationLoop::open(dir.path(), dir.path().join("ROADMAP.md"), 1, None).unwrap();
    std::fs::create_dir(dir.path().join(".magistr/loop-state.json")).unwrap();
    let (connector, seen) = fake_connector(FakeAcpAgent::new());
    let terminal = FakeTerminal::new(SIZE, Script::new());
    let mut runtime = Runtime::new(terminal, Options::new(Env::new("xterm-kitty", false, None)));
    let commands = runtime.commands().unwrap();
    let mut driver = LoopDriver::new(
        il,
        WorkflowConfig::load(dir.path()).unwrap(),
        Backend::Acp,
        runtime.inbox(),
        commands,
        connector,
    );
    assert!(matches!(
        driver.intend(OperationKind::ProviderDispatch, "dispatch"),
        Intent::Unrecorded(_)
    ));
    assert!(seen.lock().unwrap().is_empty());
}

/// A checkpoint whose provider dispatch is still owned by a live runtime:
/// the loop over it ends with `Unreconciled`, launches nothing, and
/// latches no terminal reason on the run — the same loop retries once the
/// runtime is gone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unreconciled_dispatch_ends_the_loop_without_a_launch_or_a_terminal_reason() {
    use crate::workflow::iteration::{OperationKind, OperationOutcome};
    let dir = project("true", "true", "true");
    let mut runtime = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("a stand-in provider runtime");
    {
        let mut il = IterationLoop::open(dir.path(), dir.path().join("ROADMAP.md"), 3, None)
            .expect("the loop opens");
        assert_eq!(il.pre_iteration_check().unwrap(), PreCheckResult::Continue);
        let dispatch = il
            .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
            .unwrap()
            .unwrap();
        il.start_operation(&dispatch, Some(format!("pid:{}", runtime.id())))
            .unwrap();
        il.persist().unwrap();
    }
    let recovered =
        IterationLoop::open_or_recover(dir.path(), dir.path().join("ROADMAP.md"), 3, None)
            .expect("recovery");
    assert_eq!(recovered.reconciliation().open.len(), 1);
    let wf = WorkflowConfig::load(dir.path()).expect("the fixture loads");
    let (connector, seen) = fake_connector(FakeAcpAgent::new().reply("never asked"));
    let script = Script::new().startup_replies(MS(0)).text(MS(2_500), "\x04");
    let terminal = FakeTerminal::new(SIZE, script);
    let options = Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500));
    let mut runtime_ui = Runtime::new(terminal, options);
    let commands = runtime_ui.commands().expect("the commands, once");
    let driver = LoopDriver::new(
        recovered,
        wf,
        Backend::Acp,
        runtime_ui.inbox(),
        commands,
        connector,
    )
    .auto_pause(false);
    let (_, report) = run_cockpit(runtime_ui, driver).await;
    assert!(
        matches!(report.end(), LoopEnd::Unreconciled(open) if open.contains("provider_dispatch dispatch")),
        "{:?}",
        report.end()
    );
    assert_eq!(report.iterations(), 0);
    assert!(
        seen.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty(),
        "no agent was connected while the dispatch was unreconciled"
    );
    let state = checkpoint(dir.path());
    assert_eq!(
        state.run.terminal_reason, None,
        "the run is waiting, not stopped"
    );
    assert_eq!(
        state.operations.records().last().map(|r| r.outcome),
        Some(OperationOutcome::Indeterminate)
    );
    runtime.kill().unwrap();
    runtime.wait().unwrap();
    // The runtime gone and no commit past the baseline: failed, retryable.
    let again = IterationLoop::open_or_recover(dir.path(), dir.path().join("ROADMAP.md"), 3, None)
        .expect("recovery");
    assert_eq!(
        again.reconciliation().settled[0].outcome,
        OperationOutcome::Failed
    );
}

// ── Hosted work ─────────────────────────────────────────────────────

/// A start taken inside a live hosted session — the desktop's and
/// `magistr run`'s host — runs on this same driver: the judged iteration
/// completes the chunk with every operation settled in the checkpoint, the
/// session takes conversation again when the run ends, and a further start
/// is answered on its own terms.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_start_in_a_hosted_session_runs_a_judged_iteration_and_returns_to_conversation() {
    use crate::application::workflow_client::EngineClient;
    use crate::orchestration::hosted_work::HostedWork;
    use crate::orchestration::interactive_session::interactive_session;
    use magistr_cockpit::cockpit::LaunchAction;

    let dir = project("true", "true", "true");
    let path = dir.path().join(".magistr/project.json");
    let mut declared: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("project.json")).expect("json");
    declared["roadmap_backend"] = serde_json::json!("think-and-ship");
    std::fs::write(&path, declared.to_string()).expect("project.json");
    let engines = engines_with_active_task(dir.path());
    let judge = Scripted::new([Verdict::met("both criteria hold")]);
    let (connector, seen) = fake_connector(FakeAcpAgent::new().reply("did the thing"));
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(dir.path(), |key| {
        (key == "MAGISTR_GOAL").then(|| "native".to_owned())
    });
    let config = AgentConfig::new("", dir.path()).with_inputs(&inputs);
    let client = EngineClient::attach(engines.clone());
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (_feed, tapped) = unbounded_channel();
    let session = interactive_session(
        config.clone(),
        connector.clone(),
        inbox.clone(),
        Some(client.clone()),
    );
    let scripted = Arc::clone(&judge);
    let work = HostedWork::new(config, connector, inbox, client, tapped).judged_by(Box::new(
        move |_, native, _, _, _| native.then(|| Arc::clone(&scripted) as Arc<dyn Judge>),
    ));
    let task = tokio::spawn(work.run(session, receiver));

    commands
        .send(Cmd::Launch(LaunchAction::Loop(
            magistr_cockpit::cockpit::LoopScope::AllTracks,
        )))
        .unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if let Msg::Loop(LoopEvent::Ended(why)) = events.recv().await.expect("live inbox") {
                return why;
            }
        }
    })
    .await
    .expect("the started loop ends");
    assert_eq!(
        judge.asked().len(),
        1,
        "the shared judge was asked: {ended}"
    );
    engines.reload_roadmap();
    assert_eq!(
        engines.roadmap_chunk("work").expect("the chunk").status,
        ChunkStatus::Done
    );
    let state = checkpoint(dir.path());
    assert!(
        state
            .operations
            .records()
            .iter()
            .any(|record| record.target == "complete_chunk:work"),
        "the judged completion went through the operation ledger"
    );
    assert_eq!(state.operations.unsettled().count(), 0);
    assert!(!task.is_finished(), "the session outlives the run");

    // The same session converses again, on a fresh provider context.
    commands.send(Cmd::Prompt("What changed?".into())).unwrap();
    next_matching(&mut events, |event| {
        matches!(event, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)))
    })
    .await;
    let conversed = seen
        .lock()
        .unwrap()
        .last()
        .expect("a conversation agent")
        .prompts();
    assert_eq!(conversed, ["What changed?"]);

    // A further start is taken too; with nothing left it is said in words.
    commands.send(Cmd::Launch(LaunchAction::Next)).unwrap();
    let refused = next_matching(&mut events, |event| {
        matches!(
            event,
            Msg::CommandFailed {
                cmd: Cmd::Launch(_),
                ..
            }
        )
    })
    .await;
    assert!(
        matches!(&refused, Msg::CommandFailed { error, .. } if error.contains("complete")),
        "{refused:?}"
    );
    drop(commands);
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("the host phase loop ends with its commands")
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn track_choice_waits_inside_the_cockpit_before_preflight_and_provider_work() {
    use magistr_cockpit::cockpit::{LaunchAction, LoopScope};
    let dir = project("true", "true", "true");
    let engines = engines_with_work(dir.path());
    engines
        .roadmap_add_chunk(
            "selected".into(),
            "Selected".into(),
            "Selected work".into(),
            ChunkStatus::Pending,
            1,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .unwrap();
    engines
        .roadmap_set_group("selected", Some("Chosen track".into()))
        .unwrap();
    let iteration = IterationLoop::open(
        dir.path(),
        dir.path().join("ROADMAP.md"),
        1,
        Some(engines.clone()),
    )
    .unwrap();
    iteration.persist().unwrap();
    let wf = WorkflowConfig::load(dir.path())
        .unwrap()
        .with_native(Some(engines.reader()));
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = seen.clone();
    let connector = Connector::new(move |config: AgentConfig| {
        captured.lock().unwrap().push(config);
        async { Err(MagistrError::Workflow("Test provider stopped".into())) }
    });
    let task = LoopDriver::new(iteration, wf, Backend::Acp, inbox, receiver, connector)
        .choosing_track(true)
        .spawn();

    commands.send(Cmd::Launch(LaunchAction::Next)).unwrap();
    next_matching(&mut events, |event| {
        matches!(event, Msg::CommandFailed { .. })
    })
    .await;
    assert!(seen.lock().unwrap().is_empty());
    assert!(checkpoint(dir.path()).operations.records().is_empty());
    assert_eq!(checkpoint(dir.path()).native_chunk_id, None);

    commands
        .send(Cmd::Launch(LaunchAction::Loop(LoopScope::Track(
            "Chosen track".into(),
        ))))
        .unwrap();
    let report = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(report.end(), LoopEnd::Failed(_)));
    assert_eq!(seen.lock().unwrap().len(), 1);
    let state = checkpoint(dir.path());
    assert_eq!(state.native_track.as_deref(), Some("Chosen track"));
    assert_eq!(state.native_chunk_id.as_deref(), Some("selected"));
}

#[tokio::test]
async fn closing_the_track_picker_session_starts_no_work() {
    let dir = project("true", "true", "true");
    let iteration =
        IterationLoop::open(dir.path(), dir.path().join("ROADMAP.md"), 1, None).unwrap();
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let task = LoopDriver::new(
        iteration,
        WorkflowConfig::load(dir.path()).unwrap(),
        Backend::Acp,
        inbox,
        receiver,
        Connector::new(|_| async { panic!("no choice must start no provider") }),
    )
    .choosing_track(true)
    .spawn();
    commands
        .send(Cmd::Launch(magistr_cockpit::cockpit::LaunchAction::Next))
        .unwrap();
    next_matching(&mut events, |event| {
        matches!(event, Msg::CommandFailed { .. })
    })
    .await;
    commands.send(Cmd::Quit).unwrap();
    let report = tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*report.end(), LoopEnd::CockpitClosed);
    assert_eq!(report.iterations(), 0);
    assert!(checkpoint(dir.path()).operations.records().is_empty());
}
