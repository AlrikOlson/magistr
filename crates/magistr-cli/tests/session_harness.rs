//! The session end to end: the real `AcpTransport` connected
//! over a duplex channel to an in-process ACP agent, the session producer
//! between it and the cockpit, and a scripted terminal typing prompts,
//! pressing accelerators and clicking the strip — the prompts the agent
//! saw, the permission outcomes it recorded, the model and the painted
//! screen checked against what went in.

use magistr::agent::cockpit_session::CockpitSession;
use magistr_cockpit::checkpoint::CheckpointPort;
use magistr_engine::policy::RiskTier;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use magistr_acp::AcpTransport;
use magistr_application::sessions::{
    AgentEvent, AskEverything, PolicyDecision, SessionOptions, SessionPolicy,
};
use magistr_cockpit::cockpit::{
    FrameClock, Model, Msg, Size, StageTab, TurnState, Widget, update, view,
};
use magistr_cockpit::engine::{EngineEvent, EnginePort};
use magistr_cockpit::harness::FakeWorkbench;
use magistr_cockpit::layout::LayoutTree;
use magistr_cockpit::turn::{RowKind, Voice};
use magistr_cockpit::workbench::WorkbenchPort;
use magistr_cockpit_runtime::channel::Arrivals;
use magistr_cockpit_runtime::spawn_session;
use magistr_tui::graphics::Env;
use magistr_tui::harness::{AcpObserved, Capture, Deliveries, FakeAcpAgent, FakeTerminal, Script};
use magistr_tui::runtime::{Options, Runtime};

const MS: fn(u64) -> Duration = Duration::from_millis;
const SIZE: Size = Size::new(80, 24);

#[path = "session_harness/graphics.rs"]
mod graphics;

/// The screen a VT emulator shows after replaying every write that
/// happened before `until`.
fn screen_before(capture: &Capture, until: std::time::Instant) -> String {
    let bytes = capture.bytes();
    let mut end = 0;
    for (when, len) in capture.writes() {
        if when >= until {
            break;
        }
        end += len;
    }
    let mut parser = vt100::Parser::new(SIZE.rows, SIZE.cols, 0);
    parser.process(&bytes[..end]);
    parser.screen().contents()
}

/// The screen after the first write at or after `since` — the very next
/// frame the cockpit painted once an event had reached its inbox — or
/// `None` when nothing was written after it.
fn screen_of_first_write_after(capture: &Capture, since: std::time::Instant) -> Option<String> {
    let bytes = capture.bytes();
    let mut end = 0;
    for (when, len) in capture.writes() {
        end += len;
        if when >= since {
            let mut parser = vt100::Parser::new(SIZE.rows, SIZE.cols, 0);
            parser.process(&bytes[..end]);
            return Some(parser.screen().contents());
        }
    }
    None
}

/// The strip is in the very next frame after the request reached the
/// inbox — the one-frame promise, pinned to the write rather than to
/// "some time before the answering key".
/// A line the view opens with a vocabulary glyph, as this session's
/// terminal paints it: the glyph on a plain terminal, a blank cell under
/// a drawn mark on a graphics one.
fn marked(model: &Model, glyph: &str, rest: &str) -> String {
    let mark = if model.has_pixels() { " " } else { glyph };
    format!("{mark} {rest}")
}

fn assert_strip_in_next_frame(capture: &Capture, arrivals: &Arrivals, strip: &str) {
    let requests = arrivals.permission_requests();
    assert_eq!(requests.len(), 1, "one request reached the inbox");
    let frame = screen_of_first_write_after(capture, requests[0])
        .expect("a frame was painted after the request arrived");
    assert!(frame.contains(strip), "{frame}");
}

// VT text assertions explicitly exercise the supported plain terminal. Graphics
// round trips below decode native placements and use their physical hit positions.
fn options() -> Options {
    Options::new(Env::new("xterm-kitty", false, Some("text"))).probe_timeout(MS(500))
}

fn graphics_options() -> Options {
    Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500))
}

fn flat_options() -> Options {
    Options::new(Env::new("xterm-kitty", false, None).with_screen_reader(true))
        .probe_timeout(MS(500))
}

async fn run_flat(
    script: Script,
    model: Model,
    reply: &str,
) -> (Model, Arc<AcpObserved>, Capture, Deliveries, Arrivals) {
    run_seeded(
        flat_options(),
        script,
        FakeAcpAgent::new().reply(reply),
        SessionOptions::new(),
        Box::new(AskEverything),
        None,
        None,
        None,
        model,
    )
    .await
}

/// Connect the transport to `agent` over a duplex channel, run the
/// cockpit over `script` with the session between them, and hand back
/// what everyone saw.
async fn run(
    script: Script,
    agent: FakeAcpAgent,
    session: SessionOptions,
) -> (Model, Arc<AcpObserved>, Capture, Deliveries, Arrivals) {
    run_with_policy(script, agent, session, Box::new(AskEverything)).await
}

/// [`run`], with a policy between the agent's requests and the strip.
async fn run_with_policy(
    script: Script,
    agent: FakeAcpAgent,
    session: SessionOptions,
    policy: Box<dyn SessionPolicy>,
) -> (Model, Arc<AcpObserved>, Capture, Deliveries, Arrivals) {
    run_full(script, agent, session, policy, None, None, None).await
}

/// [`run_with_policy`], with a workbench port behind the session for
/// the Changes tab and the palette commands, an engine port for the
/// Trace tab's pin and checkpoint, and a checkpoint port for the
/// ledger's save, restore and fork.
#[allow(clippy::too_many_arguments)]
async fn run_full(
    script: Script,
    agent: FakeAcpAgent,
    session: SessionOptions,
    policy: Box<dyn SessionPolicy>,
    workbench: Option<Box<dyn WorkbenchPort>>,
    engine: Option<Box<dyn EnginePort>>,
    checkpoints: Option<Box<dyn CheckpointPort>>,
) -> (Model, Arc<AcpObserved>, Capture, Deliveries, Arrivals) {
    run_seeded(
        options(),
        script,
        agent,
        session,
        policy,
        workbench,
        engine,
        checkpoints,
        Model::new(SIZE).with_tab(StageTab::Turn),
    )
    .await
}

/// [`run_full`], starting from a prepared model — how the Roadmap tab's
/// tests seed the store's chunks before the terminal is scripted — and
/// from explicit [`Options`] — how the plain-text tests put tmux or
/// `MAGISTR_TUI_MODE=text` in the environment.
// Eight arguments because this is the one fan-in every session test
// rides; a config struct would re-spell the same eight at each call.
#[allow(clippy::too_many_arguments)]
async fn run_seeded(
    options: Options,
    script: Script,
    agent: FakeAcpAgent,
    session: SessionOptions,
    policy: Box<dyn SessionPolicy>,
    workbench: Option<Box<dyn WorkbenchPort>>,
    engine: Option<Box<dyn EnginePort>>,
    checkpoints: Option<Box<dyn CheckpointPort>>,
    model: Model,
) -> (Model, Arc<AcpObserved>, Capture, Deliveries, Arrivals) {
    let (client_end, observed, agent_task) = agent.spawn();
    let mut transport =
        AcpTransport::new_with_args(PathBuf::from("fake-acp-agent"), Vec::new(), None)
            .expect("a transport");
    transport
        .connect_over(client_end)
        .await
        .expect("the handshake with the fake agent");
    let terminal = FakeTerminal::new(model.size(), script);
    let capture = terminal.capture();
    let deliveries = terminal.deliveries();
    let arrivals = Arrivals::new();
    let mut runtime = Runtime::new(terminal, options);
    let commands = runtime.commands().expect("the commands, once");
    let session = CockpitSession::start_connected(transport, session, policy)
        .expect("the supervised session");
    let session = spawn_session(
        Box::new(session),
        runtime.inbox().stamped(arrivals.clone()),
        commands,
        workbench,
        engine,
        checkpoints,
    );
    let model = runtime.run(model).await.expect("the run succeeds");
    session
        .await
        .expect("the session relay joins")
        .expect("the supervised session cleanup succeeds");
    agent_task.abort();
    (model, observed, capture, deliveries, arrivals)
}

fn rows(model: &Model) -> Vec<(RowKind, &str)> {
    model
        .turn()
        .rows()
        .iter()
        .map(|r| (r.kind, r.text.as_str()))
        .collect()
}

#[tokio::test]
async fn two_turns_on_one_session_without_reconnecting() {
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "and the second\r")
        .text(MS(600), "\x04");
    let (model, observed, capture, deliveries, _arrivals) = run(
        script,
        FakeAcpAgent::new().reply("Hello there"),
        SessionOptions::new()
            .opening_prompt("the first")
            .model("fast"),
    )
    .await;
    assert_eq!(observed.prompts(), ["the first", "and the second"]);
    assert_eq!(
        observed.config_sets(),
        [("model".to_owned(), "fast".to_owned())],
        "the model was selected before the opening prompt"
    );
    assert_eq!(observed.cancels(), 0);
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
    assert_eq!(
        rows(&model),
        [
            (RowKind::Prose(Voice::User), "the first"),
            (RowKind::Prose(Voice::Agent), "Hello there"),
            (RowKind::Prose(Voice::User), "and the second"),
            (RowKind::Prose(Voice::Agent), "Hello there"),
        ]
    );
    assert!(model.prompt().is_empty());
    assert_eq!(model.prompt().history(), ["and the second"]);
    assert_eq!(model.permissions_seen(), 0);
    // The screen before the quit: both turns on the stage, the empty
    // prompt line, the idle hint.
    let quit = *deliveries.instants().last().expect("the quit");
    let screen = screen_before(&capture, quit);
    assert!(screen.contains("the first"), "{screen}");
    assert!(screen.contains("and the second"), "{screen}");
    assert!(screen.contains(" awaiting input"), "{screen}");
    assert!(screen.contains(" enter  send"), "{screen}");
}

fn plain_mode() -> Msg {
    Msg::Mode(magistr_cockpit::capability::Mode::MinimalText(
        magistr_cockpit::capability::TextReason::Forced,
    ))
}

fn button_cell(title: &str, button: Widget) -> (u16, u16) {
    // This VT assertion uses the explicitly requested plain arrangement;
    // its locator must resolve that same surface.
    let mut model = Model::new(SIZE);
    update(&mut model, plain_mode());
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("Hello there".to_owned())),
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 1,
            title: title.to_owned(),
            choices: vec![
                magistr_application::sessions::PermissionChoice::new(
                    "allow",
                    magistr_application::sessions::PermissionKind::AllowOnce,
                ),
                magistr_application::sessions::PermissionChoice::new(
                    "allow_always",
                    magistr_application::sessions::PermissionKind::AllowAlways,
                ),
                magistr_application::sessions::PermissionChoice::new(
                    "reject",
                    magistr_application::sessions::PermissionKind::RejectOnce,
                ),
            ],
            edit: false,
            risk: RiskTier::Medium,
            plan: None,
        }),
    );
    let root = view(&model, &FrameClock::new(0, Duration::ZERO));
    let resolved = LayoutTree::new()
        .compute(&root, SIZE.cols, SIZE.rows)
        .expect("layout");
    let rect = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(button.id()))
        .expect("the button is laid out")
        .rect;
    (rect.x + 1, rect.y)
}

#[tokio::test]
async fn a_permission_answered_always_by_key_still_obeys_the_next_policy_ask() {
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "A")
        .text(MS(600), "again\r")
        .text(MS(900), "d")
        .text(MS(1200), "\x04");
    let (model, observed, capture, deliveries, arrivals) = run(
        script,
        FakeAcpAgent::new().asking_permission("Bash"),
        SessionOptions::new().opening_prompt("run the tests"),
    )
    .await;
    assert_eq!(arrivals.permission_requests().len(), 2);
    for arrived in arrivals.permission_requests() {
        let frame = screen_of_first_write_after(&capture, arrived)
            .expect("the next frame shows the permission request");
        let card = marked(&model, "⚠", "Bash needs permission");
        assert!(frame.contains(&card), "{frame}");
    }
    assert_eq!(observed.prompts(), ["run the tests", "again"]);
    assert_eq!(
        observed.outcomes(),
        [Some("allow_always".to_owned()), Some("reject".to_owned())],
        "the policy's next ask must reach the user despite the earlier Always choice"
    );
    assert_eq!(
        model.permissions_seen(),
        2,
        "remembered permission scopes belong to the policy"
    );
    assert_eq!(model.permissions_pending(), 0);
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
    assert!(
        model.prompt().is_empty(),
        "the `A` went to the strip, not the prompt"
    );
    // The strip was on screen before the key was delivered, and gone by
    // the end.
    let key = deliveries.instants()[1];
    let before = screen_before(&capture, key);
    let card = marked(&model, "⚠", "Bash needs permission");
    assert!(before.contains(&card), "{before}");
    assert!(before.contains("[\u{21e7}A] Always"), "{before}");
    assert!(
        before.contains(" A  allow   \u{21e7}A  always   D  deny"),
        "{before}"
    );
    let quit = *deliveries.instants().last().expect("the quit");
    let after = screen_before(&capture, quit);
    assert!(!after.contains("needs permission"), "{after}");
    assert!(after.contains("again"), "{after}");
}

#[tokio::test]
async fn a_permission_answered_by_click_and_denied_by_key_round_trips() {
    let (col, row) = button_cell("Bash", Widget::Allow);
    let script = Script::new()
        .startup_replies(MS(0))
        .click(MS(300), col, row)
        .text(MS(600), "again\r")
        .text(MS(900), "d")
        .text(MS(1_200), "\x04");
    let (model, observed, capture, deliveries, _arrivals) = run(
        script,
        FakeAcpAgent::new().asking_permission("Bash"),
        SessionOptions::new().opening_prompt("run the tests"),
    )
    .await;
    assert_eq!(observed.prompts(), ["run the tests", "again"]);
    assert_eq!(
        observed.outcomes(),
        [Some("allow".to_owned()), Some("reject".to_owned())]
    );
    assert_eq!(model.permissions_seen(), 2, "allow once asks again");
    assert_eq!(model.permissions_pending(), 0);
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
    // The strip stood before the click and again before the `D`.
    let click = deliveries.instants()[1];
    assert!(screen_before(&capture, click).contains("[A] Allow"));
    let deny = deliveries.instants()[3];
    assert!(screen_before(&capture, deny).contains("[D] Deny"));
    // Every animation step paints one frame of its own (the spinner
    // while the turn streams, the dock's arrival pulse twice); the
    // frames the events themselves cost are the coalescing guarantee.
    let frames = model.frames() - model.ticks();
    assert!(
        frames <= 12,
        "{frames} frames for two turns and two answers ({} steps)",
        model.ticks()
    );
}

/// A policy scripted by title: one set answered allow, one deny, the
/// rest ask — the port the root crate's real profile policy plugs into.
struct TitlePolicy {
    allow: &'static [&'static str],
    deny: &'static [&'static str],
}

impl SessionPolicy for TitlePolicy {
    fn decide(
        &mut self,
        request: &magistr_application::sessions::SessionPolicyRequest,
    ) -> PolicyDecision {
        if self.allow.contains(&request.tool_title.as_str()) {
            PolicyDecision::Allow
        } else if self.deny.contains(&request.tool_title.as_str()) {
            PolicyDecision::Deny
        } else {
            PolicyDecision::Ask(RiskTier::Medium)
        }
    }
}

#[tokio::test]
async fn a_policy_allow_is_answered_without_the_strip() {
    let script = Script::new().startup_replies(MS(0)).text(MS(600), "\x04");
    let (model, observed, capture, deliveries, _arrivals) = run_with_policy(
        script,
        FakeAcpAgent::new().asking_permission("Bash"),
        SessionOptions::new().opening_prompt("run the tests"),
        Box::new(TitlePolicy {
            allow: &["Bash"],
            deny: &[],
        }),
    )
    .await;
    assert_eq!(observed.outcomes(), [Some("allow".to_owned())]);
    assert_eq!(model.permissions_seen(), 0, "the strip never appeared");
    let quit = *deliveries.instants().last().expect("the quit");
    assert!(!screen_before(&capture, quit).contains("needs permission"));
}

#[tokio::test]
async fn a_policy_deny_is_answered_without_the_strip() {
    let script = Script::new().startup_replies(MS(0)).text(MS(600), "\x04");
    let (model, observed, capture, deliveries, _arrivals) = run_with_policy(
        script,
        FakeAcpAgent::new().asking_permission("Bash"),
        SessionOptions::new().opening_prompt("run the tests"),
        Box::new(TitlePolicy {
            allow: &[],
            deny: &["Bash"],
        }),
    )
    .await;
    assert_eq!(observed.outcomes(), [Some("reject".to_owned())]);
    assert_eq!(model.permissions_seen(), 0, "the strip never appeared");
    let quit = *deliveries.instants().last().expect("the quit");
    assert!(!screen_before(&capture, quit).contains("needs permission"));
}

#[tokio::test]
async fn ctrl_c_cancels_the_turn_in_flight_and_the_second_ctrl_c_quits() {
    let script = Script::new()
        .startup_replies(MS(0))
        .bytes(MS(300), b"\x03".as_slice())
        .bytes(MS(600), b"\x03".as_slice());
    let (model, observed, _capture, _deliveries, _arrivals) = run(
        script,
        FakeAcpAgent::new().asking_permission("Bash"),
        SessionOptions::new().opening_prompt("hang on a permission"),
    )
    .await;
    assert_eq!(observed.cancels(), 1);
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
    assert!(model.quit_requested());
    let notes: Vec<&str> = model
        .turn()
        .rows()
        .iter()
        .filter(|r| r.kind == RowKind::Note)
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(notes, ["Stopping. Your draft is kept.", "Stopped."]);
}

#[tokio::test]
async fn ctrl_c_cancels_pending_permission_and_removes_the_card() {
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "\x03")
        .text(MS(650), "\x04");
    let (model, observed, capture, deliveries, _arrivals) = run(
        script,
        FakeAcpAgent::new().asking_permission("Bash"),
        SessionOptions::new().opening_prompt("run the tests"),
    )
    .await;
    assert_eq!(observed.cancels(), 1);
    assert_eq!(observed.outcomes(), [None], "the ACP answer is Cancelled");
    assert_eq!(model.permissions_pending(), 0);
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
    let cancel = deliveries.instants()[1];
    assert!(screen_before(&capture, cancel).contains("needs permission"));
    let quit = *deliveries.instants().last().expect("the quit");
    assert!(!screen_before(&capture, quit).contains("needs permission"));
    assert!(
        rows(&model)
            .iter()
            .all(|(_, text)| !text.contains("denied after"))
    );
}

#[tokio::test]
async fn an_edit_permission_shows_as_a_diff_approval_and_round_trips() {
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "a")
        .text(MS(600), "\x04");
    let (model, observed, capture, deliveries, arrivals) = run(
        script,
        FakeAcpAgent::new()
            .asking_permission("Edit README.md")
            .as_edit(),
        SessionOptions::new().opening_prompt("fix the readme"),
    )
    .await;
    let card = marked(&model, "✎", "Edit README.md — approve the diff");
    assert_strip_in_next_frame(&capture, &arrivals, &card);
    assert_eq!(observed.outcomes(), [Some("allow".to_owned())]);
    assert_eq!(model.permissions_seen(), 1);
    assert_eq!(model.permissions_pending(), 0);
    // The diff-approval card stood before the key was delivered, with
    // the same buttons a plain permission offers, and was gone by the end.
    let key = deliveries.instants()[1];
    let before = screen_before(&capture, key);
    assert!(before.contains(&card), "{before}");
    assert!(before.contains("[A] Allow"), "{before}");
    let quit = *deliveries.instants().last().expect("the quit");
    assert!(!screen_before(&capture, quit).contains("approve the diff"));
}

#[tokio::test]
async fn a_palette_read_round_trips_through_the_session_workbench() {
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "/read README.md\r")
        .text(MS(600), "\x04");
    let workbench = FakeWorkbench::new().with_file("README.md", "# Magistr\nthe toolkit\n");
    let (model, observed, capture, deliveries, _arrivals) = run_full(
        script,
        FakeAcpAgent::new().reply("Hello there"),
        SessionOptions::new(),
        Box::new(AskEverything),
        Some(Box::new(workbench)),
        None,
        None,
    )
    .await;
    assert_eq!(
        observed.prompts(),
        Vec::<String>::new(),
        "the palette command never reached the agent"
    );
    let (title, lines) = model
        .changes()
        .viewer()
        .expect("the file opened in the pane");
    assert_eq!(title, "README.md");
    assert_eq!(lines, ["# Magistr", "the toolkit"]);
    // The viewer stood on the Changes tab before the quit.
    let quit = *deliveries.instants().last().expect("the quit");
    let screen = screen_before(&capture, quit);
    assert!(screen.contains("# Magistr"), "{screen}");
    assert!(screen.contains("README.md"), "{screen}");
    assert!(
        screen.contains("Read only") && screen.contains("2 lines · Esc closes"),
        "{screen}"
    );
}

#[tokio::test]
async fn palette_pin_and_checkpoint_round_trip_through_the_session_engine() {
    use magistr_tui::harness::FakeEnginePort;
    let mut locator = Model::new(SIZE).with_tab(StageTab::Session);
    update(&mut locator, plain_mode());
    let placed = LayoutTree::new()
        .compute(
            &view(&locator, &FrameClock::new(0, MS(0))),
            SIZE.cols,
            SIZE.rows,
        )
        .unwrap();
    let details = placed.rect(Widget::Inspector.id()).unwrap();
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "/pin 7\r")
        .text(MS(450), "/unpin 7\r")
        // The trailing space closes the completion popup — bare
        // "/checkpoint" keeps it open and Enter would accept the
        // candidate instead of sending.
        .text(MS(600), "/checkpoint \r")
        .click(MS(750), details.x + 2, details.y + details.height - 2)
        .text(MS(800), "\x1b[B")
        .text(MS(1000), "\x04");
    let port = FakeEnginePort::new()
        .with_report(serde_json::json!({"open_hypotheses": [{"step_number": 7}]}));
    let (model, observed, capture, deliveries, _arrivals) = run_full(
        script,
        FakeAcpAgent::new().reply("Hello there"),
        SessionOptions::new(),
        Box::new(AskEverything),
        None,
        Some(Box::new(port.clone())),
        None,
    )
    .await;
    assert_eq!(
        observed.prompts(),
        Vec::<String>::new(),
        "no palette command reached the agent"
    );
    assert_eq!(
        port.pins(),
        vec![(7, true), (7, false)],
        "both pins reached the engine"
    );
    let lines = model.trace().viewer().expect("the report pane opened");
    assert!(
        lines.iter().any(|line| line.contains("open_hypotheses")),
        "{lines:?}"
    );
    // The report stood on the Trace tab before the quit.
    let quit = *deliveries.instants().last().expect("the quit");
    let screen = screen_before(&capture, quit);
    assert!(
        screen.contains("Checkpoint") && screen.contains("Saved reasoning"),
        "{screen}"
    );
    assert!(screen.contains("7 lines · Esc closes"), "{screen}");
    assert!(screen.contains("open_hypotheses"), "{screen}");
}

/// The ledger, end to end over the real relay: a manual checkpoint, a
/// restore and a fork typed at the prompt reach the checkpoint port and
/// come back onto the rail and the stage, and none of them touches the
/// agent.
#[tokio::test]
async fn the_ledger_palette_round_trips_through_the_checkpoint_port() {
    use magistr_tui::harness::FakeCheckpoints;
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "/checkpoint-now before the refactor\r")
        .text(MS(600), "/fork cp-1 try/other-way\r")
        .text(MS(900), "/restore cp-1\r")
        .text(MS(1200), "\x04");
    let port = FakeCheckpoints::new()
        .in_repo("0123456789abcdef")
        .restoring_to(7, &["the ACP agent process"]);
    let (model, observed, capture, deliveries, _arrivals) = run_full(
        script,
        FakeAcpAgent::new().reply("Hello there"),
        SessionOptions::new(),
        Box::new(AskEverything),
        None,
        None,
        Some(Box::new(port.clone())),
    )
    .await;

    assert_eq!(
        observed.prompts(),
        Vec::<String>::new(),
        "no ledger command reached the agent"
    );
    assert_eq!(
        port.saved(),
        vec![(
            "cp-1".to_owned(),
            magistr_engine::CheckpointReason::Manual,
            "before the refactor".to_owned()
        )],
        "the name the rail minted is the name the port was given"
    );
    assert_eq!(
        port.forked(),
        vec![("cp-1".to_owned(), "try/other-way".to_owned())]
    );
    assert_eq!(port.restored(), vec!["cp-1".to_owned()]);

    let spine = model.rail().spine();
    assert_eq!(spine.len(), 1);
    let head = spine.head().expect("the ledger head");
    assert_eq!(head.id(), "cp-1");
    assert!(
        head.is_verified(),
        "the port answered a clean tree: {}",
        head.row()
    );

    // The stage carried what the two verbs answered before the quit.
    // The spine itself is data on the model and not yet a drawing: the
    // terminal rail is untouched by design, and the window's Checkpoints
    // section is drawn from `Rail::spine()` against the
    // session layout.
    let quit = *deliveries.instants().last().expect("the quit");
    let screen = screen_before(&capture, quit);
    assert!(
        !screen.contains("ledger"),
        "nothing on the terminal rail draws the spine: {screen}"
    );
    assert!(screen.contains("forked cp-1 · try/other-way"), "{screen}");
    assert!(screen.contains("restored cp-1 · iteration 7"), "{screen}");
    // Wrapped across rows on an 80-column stage, so the assertion is on
    // the phrase with the whitespace squashed rather than on where the
    // wrap put it.
    let squashed: String = screen
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '│' | '▎'))
        .collect();
    assert!(
        squashed.contains("leftbehind:the"),
        "what the resume could not bring is on screen, not dropped: {screen}"
    );
}

#[tokio::test]
async fn roadmap_clicks_round_trip_through_the_session_engine_and_update_the_rail() {
    use magistr_cockpit::cockpit::StageTab;
    use magistr_cockpit::engine::EngineEvent;
    use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
    use magistr_cockpit::layout::Rect;
    use magistr_tui::harness::FakeEnginePort;

    // The store's seed: a pending chunk and a researched signal. The
    // done chunk the scenario once listed is gone: the stage panel's
    // ring ended the two overdraw rows the padding used to
    // leak, and without list scrolling a done section would
    // push the signals inbox off a 24-row screen.
    let ui =
        serde_json::json!({"id": "ui", "status": "pending", "priority": 200, "deps": ["base"]});
    let signal = serde_json::json!({"id": "s1", "kind": "idea", "from": "alice",
        "body": "faster", "status": "researched"});
    let mut model = Model::new(SIZE).with_tab(StageTab::Roadmap);
    magistr_cockpit::cockpit::update(
        &mut model,
        Msg::Engine(EngineEvent::Seed(magistr_engine::NativeSnapshot {
            progress: magistr_cockpit::harness::fixtures::progress(1, 2),
            chunks: magistr_cockpit::harness::fixtures::chunks_from(vec![ui.clone()]),
            signals: magistr_cockpit::harness::fixtures::signals_from(vec![signal.clone()]),
            ..magistr_engine::NativeSnapshot::default()
        })),
    );

    // Coordinates come from the same plain layout used by the live run:
    // select the work item, start it, select the signal, then promote it
    // through its detail actions.
    let rect_of = |m: &Model, id: magistr_cockpit::widget_id::WidgetId| -> Rect {
        // The same explicitly requested plain arrangement as the live run.
        let mut m = m.clone();
        magistr_cockpit::cockpit::update(&mut m, plain_mode());
        let resolved = LayoutTree::new()
            .compute(&view(&m, &FrameClock::new(0, MS(0))), SIZE.cols, SIZE.rows)
            .expect("layout succeeds");
        resolved
            .placed()
            .iter()
            .find_map(|p| (p.id == Some(id)).then_some(p.rect))
            .unwrap_or_else(|| {
                let placed: Vec<String> = resolved
                    .placed()
                    .iter()
                    .filter_map(|p| p.id.map(|id| format!("{id:?}@{:?}", p.rect)))
                    .collect();
                panic!("{id:?} is not on screen; placed: {placed:#?}")
            })
    };
    let mut preview = model.clone();
    preview.set_roadmap_graph_rect(Some(Rect::new(0, 0, 40, 8)));
    let row = rect_of(&preview, Widget::roadmap_row(0));
    let mut opened = preview.clone();
    magistr_cockpit::cockpit::update(
        &mut opened,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), row.x, row.y),
            target: Some(Widget::roadmap_row(0)),
        },
    );
    let start = rect_of(&opened, Widget::RoadmapStart.id());
    // The inspector stays open while the stage scrolls to its signal inbox.
    let tab = rect_of(&preview, Widget::TabRoadmap.id());
    let mut triage = preview.clone();
    update(&mut triage, plain_mode());
    update(
        &mut triage,
        Msg::Key(magistr_cockpit::input::KeyEvent::new(
            magistr_cockpit::input::KeyCode::End,
        )),
    );
    let signal_row = rect_of(&triage, Widget::signal_row(0));
    update(
        &mut triage,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), signal_row.x, signal_row.y),
            target: Some(Widget::signal_row(0)),
        },
    );
    let promote = rect_of(&triage, Widget::signal_promote(0));
    let script = Script::new()
        .startup_replies(MS(0))
        .click(MS(300), row.x, row.y)
        .click(MS(500), start.x + 1, start.y)
        .click(MS(700), tab.x, tab.y)
        .text(MS(800), "\x1b[F")
        .click(MS(900), signal_row.x, signal_row.y)
        .click(MS(1050), promote.x + 1, promote.y)
        .click(MS(1150), tab.x, tab.y)
        .text(MS(1200), "\x1b[H")
        .text(MS(1400), "\x04");
    let port = FakeEnginePort::new()
        .with_chunk(magistr_cockpit::harness::fixtures::chunk_from(ui))
        .with_signal(magistr_cockpit::harness::fixtures::signal_from(signal));
    let (model, observed, capture, deliveries, _arrivals) = run_seeded(
        options(),
        script,
        FakeAcpAgent::new().reply("Hello there"),
        SessionOptions::new(),
        Box::new(AskEverything),
        None,
        Some(Box::new(port.clone())),
        None,
        model,
    )
    .await;

    assert_eq!(
        observed.prompts(),
        Vec::<String>::new(),
        "no roadmap action reached the agent"
    );
    assert_eq!(
        port.acts(),
        vec!["start ui".to_owned(), "promote s1".to_owned()],
        "both acts reached the engine"
    );
    assert_eq!(
        model.roadmap().chunk("ui").map(|c| c.state),
        Some(magistr_cockpit::cockpit::ChunkState::InProgress),
        "the tab folded the port's answer"
    );
    assert_eq!(
        model.rail().chunk(),
        Some("ui"),
        "the rail took you-are-here from the same frame"
    );
    assert!(
        model.roadmap().signals().is_empty(),
        "the promoted signal left the inbox"
    );
    // The started chunk stood in its section before the quit.
    let quit = *deliveries.instants().last().expect("the quit");
    let screen = screen_before(&capture, quit);
    assert!(
        screen.contains("Status: Active") && screen.contains("working"),
        "{screen}"
    );
}

/// A model with the ship plan already on the rail, as the engine feed
/// would have seeded it before the terminal is scripted.
fn planned_model() -> Model {
    let mut m = Model::new(SIZE).with_tab(StageTab::Turn);
    for (kind, payload) in [
        (
            "objective_set",
            serde_json::json!({"objective": {"description": "ship the plain-text cockpit"}}),
        ),
        (
            "task_added",
            serde_json::json!({"task_id": "impl", "title": "wire the banner"}),
        ),
        (
            "task_started",
            serde_json::json!({"task": {"id": "impl", "title": "wire the banner"}}),
        ),
    ] {
        update(
            &mut m,
            Msg::Engine(EngineEvent::local(
                magistr_cockpit::harness::fixtures::frame("ship", kind, payload),
            )),
        );
    }
    m
}

#[tokio::test]
async fn a_tmux_session_without_passthrough_runs_a_full_iteration_in_plain_cells() {
    // DA1 only at startup: tmux swallowed the kitty query, so the mode
    // is MinimalText(TmuxNoPassthrough).
    let script = Script::new()
        .plain_replies(MS(0))
        .text(MS(300), "A")
        .text(MS(600), "\x04");
    let (model, observed, capture, deliveries, arrivals) = run_seeded(
        Options::new(Env::new("tmux-256color", true, None)).probe_timeout(MS(500)),
        script,
        FakeAcpAgent::new()
            .asking_permission("Bash")
            .reply("Gates are green."),
        SessionOptions::new().opening_prompt("run the tests"),
        Box::new(AskEverything),
        None,
        None,
        None,
        planned_model(),
    )
    .await;
    // The iteration ran to its end: the prompt reached the agent, the
    // Needs-you strip was answered from its key, the turn came back.
    assert_eq!(observed.prompts(), ["run the tests"]);
    assert_eq!(observed.outcomes(), [Some("allow_always".to_owned())]);
    assert_eq!(model.permissions_pending(), 0);
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
    let key = deliveries.instants()[1];
    assert!(screen_before(&capture, key).contains("⚠ Bash needs permission"));
    assert_strip_in_next_frame(&capture, &arrivals, "⚠ Bash needs permission");
    // The final screen: the banner naming the passthrough fix, the plan
    // on the rail, the agent's text on the stage — all cells.
    let quit = *deliveries.instants().last().expect("the quit");
    let screen = screen_before(&capture, quit);
    assert!(screen.contains("Plain-text mode"), "{screen}");
    assert!(screen.contains("allow-passthrough"), "{screen}");
    assert!(screen.contains("wire the banner"), "{screen}");
    assert!(screen.contains("Gates are green."), "{screen}");
    // Not one kitty graphics command went out — no transmit, placement
    // or delete (the probe's one query is a query, not a command).
    assert_eq!(capture.count(b"\x1b_Ga=t"), 0);
    assert_eq!(capture.count(b"\x1b_Ga=p"), 0);
    assert_eq!(capture.count(b"\x1b_Ga=d"), 0);
}

#[tokio::test]
async fn magistr_tui_mode_text_keeps_a_kitty_terminal_plain() {
    // The terminal answers the kitty query OK — and the override still
    // wins: MinimalText(Forced), the banner naming the variable.
    let script = Script::new().startup_replies(MS(0)).text(MS(500), "\x04");
    let (model, observed, capture, deliveries, _arrivals) = run_seeded(
        Options::new(Env::new("xterm-kitty", false, Some("text"))).probe_timeout(MS(500)),
        script,
        FakeAcpAgent::new().reply("Hello there"),
        SessionOptions::new().opening_prompt("the first"),
        Box::new(AskEverything),
        None,
        None,
        None,
        planned_model(),
    )
    .await;
    assert_eq!(observed.prompts(), ["the first"]);
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
    let quit = *deliveries.instants().last().expect("the quit");
    let screen = screen_before(&capture, quit);
    assert!(screen.contains("MAGISTR_TUI_MODE"), "{screen}");
    assert!(screen.contains("wire the banner"), "{screen}");
    assert!(screen.contains("Hello there"), "{screen}");
    assert_eq!(capture.count(b"\x1b_Ga=t"), 0);
    assert_eq!(capture.count(b"\x1b_Ga=p"), 0);
}

#[tokio::test]
async fn a_blocker_card_clears_through_the_session_engine_and_the_frame_takes_the_card() {
    use magistr_tui::harness::FakeEnginePort;

    // The store's blocked chunk, as the feed's frame put it on the dock.
    let stuck = serde_json::json!({"id": "stuck", "status": "pending",
        "blocked_by": {"kind": "awaiting_human", "reason": "needs a person"}});
    let mut model = Model::new(SIZE);
    update(
        &mut model,
        Msg::Engine(EngineEvent::local(
            magistr_cockpit::harness::fixtures::frame(
                "roadmap",
                "chunk_changed",
                serde_json::json!({"family": "roadmap", "type": "chunk_changed",
                "chunk": stuck.clone()}),
            ),
        )),
    );
    assert_eq!(model.dock().len(), 1, "the card is on the dock to answer");

    // `c` on an empty prompt is the card's accelerator: the cockpit asks
    // the engines, never the agent.
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "c")
        .text(MS(700), "\x04");
    let port =
        FakeEnginePort::new().with_chunk(magistr_cockpit::harness::fixtures::chunk_from(stuck));
    let (model, observed, _capture, _deliveries, _arrivals) = run_seeded(
        options(),
        script,
        FakeAcpAgent::new().reply("Hello there"),
        SessionOptions::new(),
        Box::new(AskEverything),
        None,
        Some(Box::new(port.clone())),
        None,
        model,
    )
    .await;

    assert_eq!(
        observed.prompts(),
        Vec::<String>::new(),
        "no card answer reached the agent"
    );
    assert_eq!(
        port.acts(),
        vec!["clear the blocker on stuck".to_owned()],
        "the answer reached the engines through the session's port"
    );
    assert!(
        model.dock().is_empty(),
        "and the engine's own frame left the card gone"
    );
    assert!(
        model.prompt().is_empty(),
        "the letter was the card's, not the prompt's"
    );
}

#[tokio::test]
async fn a_screen_reader_session_is_appended_plain_lines_and_never_a_redraw() {
    // The terminal answers the kitty query OK, and the screen-reader
    // flag still wins: no alternate screen, no mouse, no pixels, and
    // every line written exactly once.
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(400), "A")
        .text(MS(800), "\x04");
    let (model, observed, capture, _deliveries, _arrivals) = run_seeded(
        Options::new(Env::new("xterm-kitty", false, None).with_screen_reader(true))
            .probe_timeout(MS(500)),
        script,
        FakeAcpAgent::new().asking_permission("Bash"),
        SessionOptions::new().opening_prompt("run the tests"),
        Box::new(AskEverything),
        None,
        None,
        None,
        Model::new(SIZE).with_title("magistr"),
    )
    .await;
    assert_eq!(observed.prompts(), ["run the tests"]);
    assert_eq!(observed.outcomes(), [Some("allow_always".to_owned())]);

    let wire = String::from_utf8(capture.bytes()).expect("utf-8");
    // The transcript: the session line, the mode banner, the prompt and
    // the agent's answer, each with its role and each on its own line.
    for line in [
        "session: magistr (flat mode)\r\n",
        "note: Screen-reader mode:",
        "you: run the tests\r\n",
        "agent: Hello from the fake agent\r\n",
    ] {
        assert!(wire.contains(line), "missing {line:?} in:\n{wire}");
    }
    // The announcement rides BOTH channels: the OSC 9 notification it
    // always was, and a plain line, because a notification is not in the
    // scrollback a flat session is read from.
    assert_eq!(capture.count(b"\x1b]9;needs you: "), 1);
    assert!(
        wire.contains("attention: needs you: "),
        "the announcement is not on the page:\n{wire}"
    );

    // Nothing that takes the terminal over, and nothing that redraws.
    for taken in [
        "\x1b[?1049h", // alternate screen
        "\x1b[?25l",   // cursor hidden
        "\x1b[?1000h", // mouse
        "\x1b[?2026h", // the synchronized-output bracket a frame is wrapped in
        "\x1b[2J",     // a clear
        "\x1b[H",      // a cursor home
    ] {
        assert_eq!(
            capture.count(taken.as_bytes()),
            0,
            "{taken:?} reached a flat session"
        );
    }
    assert_eq!(capture.count(b"\x1b_Ga=t"), 0);
    assert_eq!(capture.count(b"\x1b_Ga=p"), 0);

    // Every line exactly once: a screen reader has already read it.
    let lines: Vec<&str> = wire.split("\r\n").filter(|l| !l.is_empty()).collect();
    let mut seen = std::collections::HashSet::new();
    for line in &lines {
        if line.starts_with("session: ") || line.starts_with("agent: ") {
            assert!(seen.insert(*line), "{line:?} was written twice:\n{wire}");
        }
    }
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
}

#[tokio::test]
async fn flat_prompt_echo_is_visible_before_enter_and_keeps_edits_within_a_burst() {
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "hel")
        .text(MS(450), "lx\x7fo")
        .text(MS(600), "\r")
        .close(MS(900));
    let (model, observed, capture, deliveries, _arrivals) = run_flat(
        script,
        Model::new(SIZE).with_tab(StageTab::Turn),
        "received the corrected prompt",
    )
    .await;
    let instants = deliveries.instants();
    let before_enter = screen_before(&capture, instants[3]);
    assert!(
        before_enter.contains("input: hello"),
        "the edited draft must be readable before submission:\n{before_enter}"
    );
    assert_eq!(observed.prompts(), ["hello"]);
    assert!(model.prompt().is_empty());

    let wire = String::from_utf8(capture.bytes()).expect("utf-8");
    assert!(
        wire.contains("hel") && wire.contains("lx"),
        "each insertion is echoed even when a later key in the same burst removes it:\n{wire:?}"
    );
    assert!(
        wire.find("session:").expect("opening") < wire.find("input:").expect("draft"),
        "probe replies must not echo an orphan draft before the session header"
    );
    assert_eq!(capture.count(b"you: hello\r\n"), 1);
    assert_eq!(
        capture.count(b"agent: received the corrected prompt\r\n"),
        1
    );
}

#[tokio::test]
async fn flat_prompt_echo_follows_multiline_unicode_edits_and_history() {
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "\x1b[200~e\u{301}漢🦀\nsecond\x1b[201~")
        .text(MS(450), "\x1b[A")
        .text(MS(600), "\x7f")
        .text(MS(750), "\x1b[H")
        .text(MS(900), "\x1b[3~")
        .text(MS(1050), "\x1b[B\x1b[F")
        .text(MS(1200), "\r")
        .text(MS(1500), "\x1b[A")
        .text(MS(1650), "\x1b[A")
        .text(MS(1800), "\x1b[B\x1b[B")
        .text(MS(1950), "\x1b[200~👨‍👩‍👧\x1b[201~")
        .text(MS(2100), "\x7f")
        .close(MS(2250));
    let (model, observed, capture, deliveries, _arrivals) = run_flat(
        script,
        Model::new(SIZE).with_tab(StageTab::Turn),
        "unicode received",
    )
    .await;
    let instants = deliveries.instants();
    let pasted = screen_before(&capture, instants[2]);
    assert!(pasted.contains("input 2/2: second"), "{pasted}");
    let backspaced = screen_before(&capture, instants[4]);
    assert!(backspaced.contains("input 1/2: e\u{301}漢"), "{backspaced}");
    assert!(
        !backspaced.contains("🦀"),
        "backspace erases both cells of the emoji from the live row:\n{backspaced}"
    );
    let deleted = screen_before(&capture, instants[6]);
    assert!(deleted.contains("input 1/2: 漢"), "{deleted}");
    assert_eq!(observed.prompts(), ["漢\nsecond"]);
    let recalled = screen_before(&capture, instants[9]);
    assert!(recalled.contains("input 2/2: second"), "{recalled}");
    let first_line = screen_before(&capture, instants[10]);
    assert!(first_line.contains("input 1/2: 漢"), "{first_line}");
    assert!(
        model.prompt().is_empty(),
        "Down restored the empty draft; one Backspace removed the later joined emoji"
    );
    let wire = String::from_utf8(capture.bytes()).expect("utf-8");
    // Graphemes stay intact on the wire, with cursor anchors between
    // them for terminals that do not join emoji sequences.
    for grapheme in ["e\u{301}", "漢", "🦀", "👨‍👩‍👧"] {
        assert!(wire.contains(grapheme), "missing {grapheme:?}: {wire:?}");
    }
    let mut parser = vt100::Parser::new(SIZE.rows, SIZE.cols, 0);
    parser.process(&capture.bytes());
    let erased = parser.screen().contents();
    assert!(
        erased
            .lines()
            .rev()
            .find(|line| line.starts_with("input:"))
            .is_some_and(|line| line.trim_end() == "input:"),
        "one Backspace clears the family even on a nonjoining terminal:\n{erased}"
    );
}

#[tokio::test]
async fn flat_prompt_echo_accepts_completion_and_restores_the_unsent_draft() {
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "read @ca")
        .text(MS(450), "\t")
        .text(MS(600), "\r")
        .text(MS(900), "draft")
        .text(MS(1050), "\x1b[A")
        .text(MS(1200), "\x1b[B")
        .close(MS(1350));
    let project = tempfile::tempdir().expect("temporary project");
    std::fs::create_dir(project.path().join("src")).unwrap();
    std::fs::write(project.path().join("src/café.rs"), "// café\n").unwrap();
    let (client_end, observed, agent_task) =
        FakeAcpAgent::new().reply("completion received").spawn();
    let mut transport = AcpTransport::new_with_args(
        PathBuf::from("fake-acp-agent"),
        Vec::new(),
        Some(project.path().to_owned()),
    )
    .unwrap();
    transport.connect_over(client_end).await.unwrap();
    let terminal = FakeTerminal::new(SIZE, script);
    let capture = terminal.capture();
    let deliveries = terminal.deliveries();
    let mut runtime = Runtime::new(terminal, flat_options());
    let commands = runtime.commands().unwrap();
    let session =
        CockpitSession::start_connected(transport, SessionOptions::new(), Box::new(AskEverything))
            .unwrap();
    let session = spawn_session(
        Box::new(session),
        runtime.inbox(),
        commands,
        None,
        None,
        None,
    );
    let model = runtime
        .run(
            Model::new(SIZE)
                .with_tab(StageTab::Turn)
                .with_workspace_files(vec!["src/café.rs".to_owned()]),
        )
        .await
        .unwrap();
    session.await.unwrap().unwrap();
    agent_task.abort();
    let instants = deliveries.instants();
    let accepted = screen_before(&capture, instants[3]);
    assert!(accepted.contains("input: read @src/café.rs"), "{accepted}");
    assert_eq!(
        observed.prompts(),
        ["read @src/café.rs Attached file: src/café.rs\n\n// café\n"]
    );
    let recalled = screen_before(&capture, instants[6]);
    assert!(recalled.contains("input: read @src/café.rs"), "{recalled}");
    assert_eq!(model.prompt().text(), "draft");
    let mut parser = vt100::Parser::new(SIZE.rows, SIZE.cols, 0);
    parser.process(&capture.bytes());
    let final_screen = parser.screen().contents();
    assert_eq!(
        final_screen
            .lines()
            .rev()
            .find(|line| line.starts_with("input:"))
            .map(str::trim_end),
        Some("input: draft"),
        "the shorter draft must erase the recalled line's suffix:\n{final_screen}"
    );
    assert_eq!(capture.count(b"agent: completion received\r\n"), 1);
}

#[tokio::test]
async fn flat_prompt_echo_survives_async_output_without_replaying_settled_lines() {
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(300), "ac")
        .text(MS(450), "\x1b[D")
        .text(MS(900), "b")
        .close(MS(1050));
    let terminal = FakeTerminal::new(SIZE, script);
    let capture = terminal.capture();
    let deliveries = terminal.deliveries();
    let runtime = Runtime::new(terminal, flat_options());
    let inbox = runtime.inbox();
    let producer = tokio::spawn(async move {
        tokio::time::sleep(MS(650)).await;
        inbox.send(Msg::Agent(AgentEvent::Text(
            "background answer\n".to_owned(),
        )));
        inbox.send(Msg::Agent(AgentEvent::TurnEnded(
            magistr_application::sessions::TurnEnd::EndTurn,
        )));
    });
    let model = runtime
        .run(Model::new(SIZE))
        .await
        .expect("the flat run succeeds");
    producer.await.expect("the producer finishes");
    let restored = screen_before(&capture, deliveries.instants()[3]);
    assert!(restored.contains("agent: background answer"), "{restored}");
    assert!(restored.contains("input: ac"), "{restored}");
    assert_eq!(model.prompt().text(), "abc");
    let mut parser = vt100::Parser::new(SIZE.rows, SIZE.cols, 0);
    parser.process(&capture.bytes());
    let final_screen = parser.screen().contents();
    assert!(final_screen.contains("input: abc"), "{final_screen}");
    assert_eq!(
        capture.count(b"session: magistr cockpit (flat mode)\r\n"),
        1
    );
    assert_eq!(capture.count(b"agent: background answer\r\n"), 1);
}
