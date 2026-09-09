//! The scripted scenes every still, filmstrip and view test is taken of.
//!
//! One copy, two readers: the cockpit's own view tests assert on these
//! models, and magistr-tui's golden suite (`tests/goldens.rs`) rasterizes
//! them through the terminal renderer — so a still and the assertion
//! about the same screen describe the same model. Every scene is built
//! the way a session builds it: through `update`, from the events a
//! producer would post.

use std::time::Duration;

use magistr_engine::policy::RiskTier;

use crate::agent::{AgentEvent, ToolStatus, ToolVerb};
use crate::cockpit::{
    FrameClock, Model, Msg, Size, StageTab, Widget, WorkbenchEvent, update, view,
};
use crate::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use crate::layout::LayoutTree;
use crate::theme::{Theme, ThemeMode};
use crate::widget_id::WidgetId;

/// The scenes, as associated functions: each takes the size the still is
/// taken at and answers the model that surface shows.
pub struct Scenes;

fn agent(m: &mut Model, event: AgentEvent) {
    update(m, Msg::Agent(event));
}

impl Scenes {
    pub fn streaming(size: Size) -> Model {
        let mut m = Model::new(size);
        agent(
            &mut m,
            AgentEvent::Thought("I should read the roadmap chunk first, then plan.".to_owned()),
        );
        // A streaming session is a busy one: the frames between the chunks
        // are what the header's activity monitor draws.
        update(&mut m, Msg::FrameDue);
        agent(
            &mut m,
            AgentEvent::UserText("implement the turn stage".to_owned()),
        );
        agent(
            &mut m,
            AgentEvent::Text(
                "# Plan\nFirst, read the file and record a think step. The wrap keeps every row \
                 inside the stage whatever the terminal width happens to be.\n\n- one item\n\
                 - a second item long enough to wrap onto a continuation row at eighty columns\n\
                 1. numbered\n\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\nStill streaming"
                    .to_owned(),
            ),
        );
        update(&mut m, Msg::FrameDue);
        m
    }

    /// A run card open on the stage while its command streams: the
    /// pane's last eight rows in the program's own colours — cargo's
    /// bold green verbs, a yellow warning, a red failure, a dim note, a
    /// truecolor word and a progress bar overwritten in place — the
    /// cursor at the tail.
    pub fn terminal_pane(size: Size) -> Model {
        use crate::agent::ToolRecord;
        let mut m = Model::new(size);
        agent(&mut m, AgentEvent::Text("Running the gates.\n".to_owned()));
        agent(
            &mut m,
            AgentEvent::ToolCall {
                id: "c1".to_owned(),
                title: "cargo nextest run -p magistr-tui".to_owned(),
                verb: ToolVerb::Run,
                status: ToolStatus::InProgress,
                diff: None,
                images: Vec::new(),
                record: Box::new(ToolRecord {
                    input: Some(serde_json::json!({
                        "command": "cargo nextest run -p magistr-tui"
                    })),
                    terminal: Some("term-1".to_owned()),
                    ..ToolRecord::default()
                }),
            },
        );
        Self::stream_run(&mut m, 0);
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Enter)));
        m
    }

    /// [`Self::terminal_pane`] with the reader scrolled two notches up
    /// inside the card and three more rows arrived under the view: the
    /// pane holds its place and the jump pill counts them.
    pub fn terminal_pane_scrolled(size: Size) -> Model {
        use crate::turn::RowKind;
        let mut m = Self::terminal_pane(size);
        Self::settle(&mut m);
        let first = m
            .turn()
            .rows()
            .iter()
            .position(|row| row.kind == RowKind::Pane)
            .expect("the card opened into its pane");
        let index = first.saturating_sub(m.top_row());
        for _ in 0..2 {
            update(
                &mut m,
                Msg::Mouse {
                    event: MouseEvent::new(MouseKind::ScrollUp, 10, 8),
                    target: Some(Widget::stage_row(index)),
                },
            );
        }
        Self::stream_run(&mut m, 1);
        m
    }

    /// [`Self::terminal_pane`] with the screen raised into the dock
    /// by `ctrl-t` and the rise settled: the grip row
    /// naming the command over the docked rows, the stage's rows above
    /// it, the prompt below.
    pub fn terminal_dock(size: Size) -> Model {
        use crate::input::Modifiers;
        let mut m = Self::terminal_pane(size);
        Self::settle(&mut m);
        update(
            &mut m,
            Msg::Key(KeyEvent::new(KeyCode::Char('t')).with(Modifiers::CTRL)),
        );
        Self::settle(&mut m);
        m
    }

    /// [`Self::terminal_pane`] with the kill held on `k` for four ticks
    /// past the dead zone: the terminal has reported the kitty keyboard protocol's
    /// event types, so the key may be held, and the card's head is
    /// part-way red from the left.
    pub fn terminal_kill(size: Size) -> Model {
        use crate::cockpit::TermHold;
        use crate::input::Reply;
        let mut m = Self::terminal_pane(size);
        Self::settle(&mut m);
        update(&mut m, Msg::Reply(Reply::KeyboardFlags(3)));
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char('k'))));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let dead = TermHold::DEAD.ceil() as usize;
        for _ in 0..dead + 4 {
            update(&mut m, Msg::Tick);
        }
        m
    }

    /// [`Self::terminal_kill`] held through the half second: the kill
    /// completed, the screen frozen and marked the reader's doing, the
    /// strip row under the head reading `killed by you`.
    pub fn terminal_killed(size: Size) -> Model {
        use crate::cockpit::TermHold;
        let mut m = Self::terminal_kill(size);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let hold = TermHold::HOLD.ceil() as usize;
        for _ in 0..hold {
            update(&mut m, Msg::Tick);
        }
        m
    }

    /// What the run writes to its terminal, in two parts: the build and
    /// the first tests (`part` 0), then three more results (`part` 1).
    /// The build arrives in five chunks the ticks apart a real build
    /// is — the same second, a pause of two, a long silence of seven
    /// before the tests report — so the pane's time rail has a tick,
    /// two breaks and their stamps to show; the rows are joined by CRLF
    /// as a pty carries them; the progress bar is one row written twice,
    /// the second over the first with a bare CR, each with the ConEmu
    /// progress report (OSC 9;4) cargo-nextest would send if it could,
    /// so the card carries the strip.
    fn stream_run(m: &mut Model, part: usize) {
        const BUILD: [(u64, &[&str]); 5] = [
            (
                0,
                &[
                    "\x1b[1m\x1b[32m   Compiling\x1b[0m magistr-cockpit v0.1.0\r\n",
                    "\x1b[1m\x1b[32m   Compiling\x1b[0m magistr-tui v0.1.0\r\n",
                ],
            ),
            (
                10,
                &[
                    "\x1b[1m\x1b[33mwarning\x1b[0m: unused variable: `pane`\r\n",
                    "  \x1b[1m\x1b[34m-->\x1b[0m crates/magistr-tui/src/lib.rs:12:9\r\n",
                ],
            ),
            (
                30,
                &[
                    "\x1b[1m\x1b[32m    Finished\x1b[0m `test` profile in 4.2s\r\n",
                    "\x1b[1m    Starting\x1b[0m 212 tests across 3 binaries\r\n",
                ],
            ),
            (
                10,
                &[
                    "        \x1b[32mPASS\x1b[0m [   0.012s] term::a_red_run_is_a_red_cell\r\n",
                    "        \x1b[31mFAIL\x1b[0m [   0.201s] goldens::shot_terminal_pane\r\n",
                    "        \x1b[32mPASS\x1b[0m [   0.009s] \x1b[38;2;250;179;135mpen::the_term_pen\x1b[0m\r\n",
                ],
            ),
            (
                125,
                &[
                    "progress [\x1b[36m####------\x1b[0m]  40%\x1b]9;4;1;40\x07\r\
                     progress [\x1b[36m#######---\x1b[0m]  70%  \
                     \x1b[2m(3 tests still running)\x1b[0m\x1b]9;4;1;70\x07",
                ],
            ),
        ];
        const MORE: [&str; 4] = [
            "\r\x1b[K        \x1b[32mPASS\x1b[0m [   0.030s] terminals::chunks_land_in_order",
            "        \x1b[32mPASS\x1b[0m [   0.031s] terminals::follow_all",
            "        \x1b[31mFAIL\x1b[0m [   0.400s] goldens::shot_terminal_pane_scrolled",
            "progress [\x1b[36m##########\x1b[0m] 100%\x1b]9;4;1;100\x07",
        ];
        let chunk = |m: &mut Model, bytes: Vec<u8>| {
            agent(
                m,
                AgentEvent::Terminal {
                    terminal_id: "term-1".to_owned(),
                    bytes,
                },
            );
        };
        if part == 0 {
            for (after, rows) in BUILD {
                Self::ticks(m, after);
                chunk(m, rows.concat().into_bytes());
            }
        } else {
            chunk(m, MORE.join("\r\n").into_bytes());
        }
    }

    /// Streaming with a permission request pending for a shell command.
    pub fn permission(size: Size) -> Model {
        let mut m = Self::permission_turn(size);
        Self::request_permission(&mut m);
        m
    }

    /// The turn the permission surface is about, without the request yet —
    /// so a caller can settle the stage before the card arrives.
    pub fn permission_turn(size: Size) -> Model {
        let mut m = Model::new(size);
        agent(&mut m, AgentEvent::Text("Let me run the tests.".to_owned()));
        agent(
            &mut m,
            AgentEvent::ToolCall {
                id: "c1".to_owned(),
                title: "cargo nextest run -p magistr-tui".to_owned(),
                verb: ToolVerb::Run,
                status: ToolStatus::Pending,
                diff: None,
                images: Vec::new(),
                record: Default::default(),
            },
        );
        m
    }

    /// The request that puts the card on the dock.
    pub fn request_permission(m: &mut Model) {
        use crate::agent::{PermissionChoice, PermissionKind};
        agent(
            m,
            AgentEvent::PermissionRequest {
                id: 1,
                title: "Bash".to_owned(),
                choices: vec![
                    PermissionChoice::new("allow", PermissionKind::AllowOnce),
                    PermissionChoice::new("allow_always", PermissionKind::AllowAlways),
                    PermissionChoice::new("reject", PermissionKind::RejectOnce),
                ],
                edit: false,
                risk: RiskTier::Medium,
                plan: None,
            },
        );
    }

    /// Iteration 2 of 5 in the execute phase, the check gate green, the test
    /// gate red, lint still running, 1m 23s in, the agent mid-turn.
    pub fn looping(size: Size) -> Model {
        use crate::cockpit::LoopEvent;
        use crate::job::{JobId, JobOutput};
        let mut m = Model::new(size);
        update(
            &mut m,
            Msg::Loop(LoopEvent::Started {
                iteration: 2,
                max: 5,
            }),
        );
        update(
            &mut m,
            Msg::Loop(LoopEvent::Note(
                "starting iteration 2 — 3/10 chunks done".to_owned(),
            )),
        );
        for (family, kind, payload) in [
            (
                "roadmap",
                "chunk_changed",
                serde_json::json!({"chunk": {"id": "a", "status": "in_progress"}}),
            ),
            (
                "ship",
                "objective_set",
                serde_json::json!({"objective": {}}),
            ),
            ("ship", "task_started", serde_json::json!({"task_id": "t1"})),
        ] {
            update(
                &mut m,
                Msg::Engine(crate::engine::EngineEvent::Frame(
                    magistr_engine::NativeFrame {
                        family: family.to_owned(),
                        kind: kind.to_owned(),
                        payload,
                    },
                )),
            );
        }
        agent(
            &mut m,
            AgentEvent::UserText("implement the next chunk".to_owned()),
        );
        agent(&mut m, AgentEvent::Text("Reading the plan.".to_owned()));
        for (i, name) in ["check", "test", "lint"].into_iter().enumerate() {
            update(
                &mut m,
                Msg::Loop(LoopEvent::GateStarted {
                    name: name.to_owned(),
                    job: JobId::new(i as u64),
                }),
            );
        }
        update(
            &mut m,
            Msg::Job {
                id: JobId::new(0),
                output: JobOutput::Text("ok".to_owned()),
            },
        );
        update(
            &mut m,
            Msg::Job {
                id: JobId::new(1),
                output: JobOutput::Failed("2 tests failed".to_owned()),
            },
        );
        update(
            &mut m,
            Msg::Loop(LoopEvent::Elapsed(Duration::from_secs(83))),
        );
        m
    }

    /// The Changes tab over a journal of two edits — one parse-failed.
    pub fn changed(size: Size) -> Model {
        use crate::workbench::JournalEntry;
        let mut m = Model::new(size);
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Tab)));
        assert_eq!(m.tab(), StageTab::Changes);
        update(
            &mut m,
            Msg::Workbench(WorkbenchEvent::Journal {
                entries: vec![
                    JournalEntry {
                        id: "e1".to_owned(),
                        path: "src/lib.rs".to_owned(),
                        added: 2,
                        removed: 1,
                        parse_ok: true,
                        diff:
                            "@@ -1,1 +1,2 @@\n-fn old() {}\n+fn renamed() {}\n+pub use renamed as old;"
                                .to_owned(),
                    },
                    JournalEntry {
                        id: "e2".to_owned(),
                        path: "src/parser.rs".to_owned(),
                        added: 4,
                        removed: 0,
                        parse_ok: false,
                        diff: "+fn broken( {".to_owned(),
                    },
                ],
            }),
        );
        m
    }

    pub fn engine_frame(m: &mut Model, family: &str, kind: &str, payload: serde_json::Value) {
        use crate::engine::EngineEvent;
        update(
            m,
            Msg::Engine(EngineEvent::Frame(magistr_engine::NativeFrame {
                family: family.to_owned(),
                kind: kind.to_owned(),
                payload,
            })),
        );
    }

    /// The Trace tab fed by the engine: three main-line steps (the first
    /// pinned, the second revised by the fifth), a two-step branch since
    /// merged, and a ship timeline of an action, a passed check and a
    /// failed one.
    pub fn traced(size: Size) -> Model {
        let mut m = Model::new(size);
        Self::engine_frame(
            &mut m,
            "think",
            "step_appended",
            serde_json::json!({"step": {
                "step_number": 1,
                "estimated_total": 4,
                "purpose": "hypothesis",
                "context": "the trace tab",
                "thought": "the feed can drive the whole surface",
                "outcome": "it can",
                "next_action": "wire the view",
                "rationale": "one fold path for feed and port",
                "confidence": 0.62,
                "uncertainty_notes": "no branch case yet",
                "dependencies": [7, 9],
                "execution_ref": "task:trace-state",
            }}),
        );
        Self::engine_frame(
            &mut m,
            "think",
            "step_appended",
            serde_json::json!({"step": {"step_number": 2, "purpose": "analysis", "confidence": 0.7}}),
        );
        Self::engine_frame(
            &mut m,
            "think",
            "step_appended",
            serde_json::json!({"step": {
                "step_number": 3,
                "purpose": "hypothesis",
                "branch_id": "b-alt",
                "branch_name": "alt path",
                "branch_from": 2,
            }}),
        );
        Self::engine_frame(
            &mut m,
            "think",
            "step_appended",
            serde_json::json!({"step": {"step_number": 4, "branch_id": "b-alt", "purpose": "validation"}}),
        );
        Self::engine_frame(
            &mut m,
            "think",
            "step_appended",
            serde_json::json!({"step": {"step_number": 5, "purpose": "correction", "revises_step": 2}}),
        );
        Self::engine_frame(
            &mut m,
            "think",
            "pin_changed",
            serde_json::json!({"step_number": 1, "pinned": true}),
        );
        Self::engine_frame(
            &mut m,
            "think",
            "branch_status_changed",
            serde_json::json!({"branch_id": "b-alt", "status": "merged"}),
        );
        Self::engine_frame(
            &mut m,
            "ship",
            "action_recorded",
            serde_json::json!({"task_id": "trace-state", "action": {
                "action_type": "code", "description": "wired the trace tab"}}),
        );
        Self::engine_frame(
            &mut m,
            "ship",
            "check_recorded",
            serde_json::json!({"check": {"name": "test", "passed": true, "exit_code": 0,
                "results": {"duration_ms": 5400}}}),
        );
        Self::engine_frame(
            &mut m,
            "ship",
            "check_recorded",
            serde_json::json!({"check": {"name": "lint", "passed": false, "exit_code": 101,
                "duration_ms": 900}}),
        );
        update(
            &mut m,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 40, 2),
                target: Some(Widget::TabTrace.id()),
            },
        );
        m
    }

    /// A trace long enough to outgrow the stage: twenty main-line steps and
    /// a ship timeline, on the Trace tab at 80x24.
    pub fn long_trace(size: Size) -> Model {
        let mut m = Model::new(size);
        for number in 1..=20u32 {
            Self::engine_frame(
                &mut m,
                "think",
                "step_appended",
                serde_json::json!({"step": {
                    "step_number": number,
                    "purpose": format!("step {number}"),
                }}),
            );
        }
        Self::engine_frame(
            &mut m,
            "ship",
            "check_recorded",
            serde_json::json!({"check": {"name": "test", "passed": true, "exit_code": 0}}),
        );
        update(
            &mut m,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 40, 2),
                target: Some(Widget::TabTrace.id()),
            },
        );
        m
    }

    /// A 120x40 model on the Roadmap tab, seeded with two workstreams, an
    /// open proposal and a researched signal.
    pub fn roadmapped(size: Size) -> Model {
        let mut m = Model::new(size).with_tab(StageTab::Roadmap);
        let mut proposed = serde_json::json!({
            "id": "tui2-3-roadmap", "name": "Roadmap view", "title": "The Roadmap tab of the cockpit",
            "status": "pending", "priority": 200, "group": "TUI rewrite", "deps": ["tui2-2-rail"],
            "description": "The tech-tree, the list and the actions.",
            "acceptance": ["the DAG renders", "clicks select"],
            "cross_refs": ["think:190"],
        });
        proposed["reprioritize"] =
            serde_json::json!({"suggested_priority": 100, "reason": "sooner"});
        update(
            &mut m,
            Msg::Engine(crate::engine::EngineEvent::Seed(crate::engine::StoreSeed {
                ship_status: serde_json::Value::Null,
                chunk: Some("tui2-2-rail".to_owned()),
                done: 1,
                total: 3,
                signals_pending: 1,
                chunks: vec![
                    serde_json::json!({"id": "acp-2", "name": "ACP transport", "status": "done",
                        "priority": 100, "group": "Engine"}),
                    serde_json::json!({"id": "tui2-2-rail", "name": "Rail and dock", "status": "in_progress",
                        "priority": 100, "group": "TUI rewrite", "deps": ["acp-2"]}),
                    proposed,
                ],
                signals: vec![serde_json::json!({
                    "id": "sig-fast", "kind": "idea", "from": "alice", "body": "make startup faster",
                    "status": "researched", "enrichment": [{"confidence": 0.8}],
                })],
                trace: Vec::new(),
            })),
        );
        m
    }

    /// A settled native catch-up overlay with a stuck chunk, unread change,
    /// completed work, and budget usage, sorted by severity.
    pub fn briefed(size: Size) -> Model {
        let mut m = Self::sky_working(size);
        for id in ["e10-fleet", "tui2-sheet-overflow"] {
            Self::engine_frame(
                &mut m,
                "roadmap",
                "chunk_completed",
                serde_json::json!({
                    "chunk": {"id": id, "name": id, "status": "done", "priority": 200}
                }),
            );
        }
        update(
            &mut m,
            Msg::Workbench(crate::cockpit::WorkbenchEvent::Journal {
                entries: (0..3)
                    .map(|n| crate::workbench::JournalEntry {
                        id: format!("e{n}"),
                        path: format!("crates/magistr-gui2/src/f{n}.rs"),
                        added: 12,
                        removed: 4,
                        parse_ok: true,
                        diff: String::new(),
                    })
                    .collect(),
            }),
        );
        m.rail_mut()
            .set_budget(crate::cockpit::Budget::capped(1200).map(|b| b.spent(361).stop_at(1000)));
        // The away is set before the settle, so the deal is timed from
        // tick zero and the settled model is the *landed* still rather
        // than a frame of the rise.
        let mut m = m.with_away(Duration::from_secs(6 * 3600 + 12 * 60));
        Self::settle(&mut m);
        m
    }

    /// The native roadmap after the idle countdown and chrome fade.
    /// Only the roadmap and status rows remain visible.
    pub fn resting(size: Size) -> Model {
        let mut m = Self::sky_working(size);
        Self::ticks(
            &mut m,
            crate::cockpit::Motion::REST_STEPS + crate::cockpit::Motion::CHROME_FADE_STEPS,
        );
        m
    }

    /// A window on the Sky with a chunk in progress and one stuck —
    /// the room both the Brief and the rest state are read over.
    pub fn sky_working(size: Size) -> Model {
        let mut m = Model::new(size)
            .with_surface(crate::cockpit::Surface::Native)
            .with_tab(StageTab::Roadmap);
        update(
            &mut m,
            Msg::Engine(crate::engine::EngineEvent::Seed(crate::engine::StoreSeed {
                ship_status: serde_json::Value::Null,
                chunk: Some("gui2-2a".to_owned()),
                done: 211,
                total: 306,
                signals_pending: 0,
                chunks: vec![
                    serde_json::json!({"id": "motion", "name": "motion", "status": "done",
                        "priority": 200, "group": "core"}),
                    serde_json::json!({"id": "gui2-2a", "name": "gui2-2a design",
                        "status": "in_progress", "priority": 159, "group": "core",
                        "deps": ["motion"]}),
                    serde_json::json!({"id": "e6b", "name": "OTLP endpoint scope",
                        "status": "stuck", "priority": 300, "group": "ops"}),
                ],
                signals: Vec::new(),
                trace: Vec::new(),
            })),
        );
        m
    }

    /// A configured project on the Home tab: the launcher job delivered,
    /// the roadmap seeded — every card populated.
    pub fn launcher(size: Size) -> Model {
        let mut m = Model::new(size).with_tab(StageTab::Home);
        update(
            &mut m,
            Msg::Engine(crate::engine::EngineEvent::Seed(crate::engine::StoreSeed {
                ship_status: serde_json::Value::Null,
                chunk: None,
                done: 2,
                total: 4,
                signals_pending: 0,
                chunks: vec![
                    serde_json::json!({"id": "core", "name": "Core", "status": "done", "priority": 100}),
                    serde_json::json!({"id": "rail", "name": "Rail", "status": "done", "priority": 200}),
                    serde_json::json!({"id": "tabs", "name": "Tabs", "status": "in_progress", "priority": 200}),
                    serde_json::json!({"id": "launcher", "name": "Launcher", "title": "The launcher page", "status": "pending", "priority": 300, "deps": ["tabs", "rail"]}),
                ],
                signals: Vec::new(),
                trace: Vec::new(),
            })),
        );
        update(
            &mut m,
            Msg::Job {
                id: crate::job::JobId::new(0),
                output: crate::job::JobOutput::Launcher(Box::new(crate::cockpit::LauncherData {
                    project: "magistr".to_owned(),
                    has_config: true,
                    stack: "Rust / Cargo".to_owned(),
                    git: "master, clean".to_owned(),
                    backend: "ACP (available)".to_owned(),
                    sessions: vec![
                        crate::cockpit::LauncherSession {
                            id: Some("sess-1".to_owned()),
                            when: "Aug 28 14:02".to_owned(),
                            duration: "12m 30s".to_owned(),
                            outcome: crate::cockpit::SessionOutcome::Success,
                            preview: "shipped the roadmap tab".to_owned(),
                        },
                        crate::cockpit::LauncherSession {
                            // Gone from disk: the entry point could give
                            // this row no id, so it never resumes.
                            id: None,
                            when: "Aug 27 19:41".to_owned(),
                            duration: "3m 05s".to_owned(),
                            outcome: crate::cockpit::SessionOutcome::Error,
                            preview: "gates failed on lint".to_owned(),
                        },
                    ],
                })),
            },
        );
        m
    }

    /// Steps a still is settled by before it is rasterized. Every transient
    /// the cockpit has — an arrival fade, a pop, a tab or dock transition,
    /// the trace graph's sweep, a progress glide — is over well inside this, so a still shows the
    /// *surface* rather than the frame the fixture happened to stop on. The
    /// two motions that never settle are periodic, and this is a whole
    /// number of both their periods, so they land exactly where an unticked
    /// model had them: the shimmer wraps every [`SHIMMER_STEPS`] and the
    /// spinner turns fully every four [`Model::SPIN_STEPS`]. Motion itself
    /// is reviewed on the filmstrips, not here.
    pub const SETTLE_STEPS: u64 = 24;

    /// `model` advanced to its settled frame — see [`Self::SETTLE_STEPS`].
    pub fn settle(model: &mut Model) {
        Self::ticks(model, Self::SETTLE_STEPS);
    }

    /// `make`'s surface with the pointer resting on the top-left cell of
    /// `id`'s rect — the move a real mouse would make, through `update`, so
    /// the fixture goes through the same fold the runtime does.
    pub fn hovering(make: fn(Size) -> Model, size: Size, id: WidgetId) -> Model {
        let mut model = make(size);
        let root = view(&model, &FrameClock::new(0, Duration::ZERO));
        let rect = LayoutTree::new()
            .compute(&root, size.cols, size.rows)
            .expect("layout succeeds")
            .rect(id)
            .expect("the widget is laid out");
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Move, rect.x, rect.y),
                target: Some(id),
            },
        );
        model
    }

    /// The streaming turn with the pointer on the Roadmap tab.
    pub fn hovered_tab(size: Size) -> Model {
        Self::hovering(Self::streaming, size, Widget::TabRoadmap.id())
    }

    /// The permission card with the pointer on its \[Allow\] button.
    pub fn hovered_button(size: Size) -> Model {
        Self::hovering(Self::permission, size, Widget::Allow.id())
    }

    /// The accelerator sheet over the launcher, at the top of its scroll —
    /// which at both still sizes is a window onto a registry that runs past
    /// the stage, so the heading's position readout is on the still.
    pub fn sheeted(size: Size) -> Model {
        let mut model = Self::launcher(size);
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char('?'))));
        model
    }

    pub fn ticks(model: &mut Model, steps: u64) {
        for _ in 0..steps {
            update(model, Msg::Tick);
        }
    }

    /// The status strip the step two gates settled on — one passed, one
    /// failed — so the strip carries the pass's bright pop beside the
    /// failure's error-coloured flash. Both land between the same
    /// two ticks, which is what the motion layer merges into one move.
    pub fn gates_popping() -> Model {
        use crate::cockpit::LoopEvent;
        use crate::job::{JobId, JobOutput};
        let mut m = Self::looping(Size::new(80, 24)).with_theme(Theme::forced(ThemeMode::Dark));
        update(
            &mut m,
            Msg::Loop(LoopEvent::GateStarted {
                name: "fmt".to_owned(),
                job: JobId::new(3),
            }),
        );
        Self::settle(&mut m);
        update(
            &mut m,
            Msg::Job {
                id: JobId::new(2),
                output: JobOutput::Text("ok".to_owned()),
            },
        );
        update(
            &mut m,
            Msg::Job {
                id: JobId::new(3),
                output: JobOutput::Failed("3 files unformatted".to_owned()),
            },
        );
        m
    }
}
