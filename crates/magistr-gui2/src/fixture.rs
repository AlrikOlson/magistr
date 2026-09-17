use magistr_application::sessions::{
    AgentEvent, AskPlan, EditDiff, ImageBlock, PermissionChoice, PermissionKind, ToolStatus,
    ToolVerb, TurnEnd,
};
use magistr_cockpit::capability::Mode;
use magistr_cockpit::cockpit::{
    AgentMissing, Birth, BornData, Budget, CharterOffer, CharterRule, GalaxyData, GalaxyProject,
    LauncherData, LauncherSession, LoopEvent, Model, Motion, Msg, Place, Proposal, ProposedChunk,
    RuleMode, ScaffoldKind, SessionAgent, SessionOutcome, SettingsData, Shelf, ShelfStill, Size,
    StageTab, StillKind, Surface, SurveyKind, SurveyReport, TermHold, Widget, WorkbenchEvent,
    update,
};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::harness::fixtures;
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseKind};
use magistr_cockpit::job::{JobId, JobOutput};
use magistr_cockpit::pixel::CellSize;
use magistr_cockpit::turn::StreamImage;
use magistr_cockpit::workbench::JournalEntry;
use magistr_domain::policy::RiskTier;
use magistr_domain::workflow::CheckpointReason;
use magistr_workflow_contracts::FeedStatus;
use magistr_workflow_contracts::NativeSnapshot;

/// The two cockpit surfaces the native painter is proven on, built from
/// the cockpit's own `Msg`s.
///
/// gui2's own fixtures rather than magistr-cockpit's: `view.tests.rs` builds
/// its `launcher`/`streaming` inside a `cfg(test)` module, which is
/// unreachable from another crate. What matters is not that the two
/// files are identical but that these go through the same public
/// `update` — a model this crate hand-assembled by writing fields would
/// prove nothing about the cockpit.
///
/// Not `cfg(test)`: the measurement harness under `tests/` needs them
/// too, and a fixture behind `cfg(test)` is one every integration test
/// has to re-create.
pub struct Fixture;

impl Fixture {
    /// Ticks a surface is settled by before it is drawn.
    ///
    /// The screenshot gate's own `SETTLE_STEPS`, and for its reasons:
    /// long enough that every transient is over, and a whole number of
    /// both periodic motions' periods so they land where an unticked
    /// model had them. A native still that disagreed with a terminal
    /// still only because the two stopped on different frames would be
    /// a golden about timing rather than about painting.
    pub const SETTLE_STEPS: u64 = 24;

    /// The Home surface: the launcher, with a project, a roadmap and
    /// two past sessions.
    #[must_use]
    pub fn home(size: Size) -> Model {
        let mut model = Model::new(size).with_tab(StageTab::Session);
        seed(&mut model);
        update(
            &mut model,
            Msg::Job {
                id: JobId::new(0),
                output: JobOutput::Launcher(Box::new(LauncherData {
                    project: "magistr".to_owned(),
                    has_config: true,
                    stack: "Rust / Cargo".to_owned(),
                    git: "master, clean".to_owned(),
                    backend: "ACP (available)".to_owned(),
                    sessions: vec![
                        LauncherSession {
                            id: Some("sess-1".to_owned()),
                            when: "Aug 30 14:02".to_owned(),
                            duration: "12m 30s".to_owned(),
                            outcome: SessionOutcome::Success,
                            preview: "shipped the framework extraction".to_owned(),
                        },
                        LauncherSession {
                            id: None,
                            when: "Aug 29 09:41".to_owned(),
                            duration: "4m 02s".to_owned(),
                            outcome: SessionOutcome::Error,
                            preview: "gate failed on the legacy ratchet".to_owned(),
                        },
                    ],
                })),
            },
        );
        settle(model)
    }

    /// The Turn surface: an agent mid-answer, with prose, a list and a
    /// fenced block — the wrap, the accents and the streaming shimmer
    /// all on screen at once.
    #[must_use]
    pub fn turn(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_surface(Surface::Native)
            .with_tab(StageTab::Turn);
        let agent = |model: &mut Model, event| update(model, Msg::Agent(event));
        agent(
            &mut model,
            AgentEvent::Thought("Read the roadmap chunk first, then plan.".to_owned()),
        );
        // A streaming session is a busy one, and the frames between the
        // chunks are what the header's activity monitor draws.
        update(&mut model, Msg::FrameDue);
        agent(
            &mut model,
            AgentEvent::UserText("paint the cockpit natively".to_owned()),
        );
        agent(
            &mut model,
            AgentEvent::Text(
                "# Plan\nWalk the resolved layout and emit a draw list. The wrap keeps every \
                 row inside the stage whatever the window width happens to be.\n\n- one item\n\
                 - a second item long enough to wrap onto a continuation row at eighty columns\n\
                 1. numbered\n\n```rust\nfn paint(node: &Node) {\n    emit(node);\n}\n```\n\
                 Still streaming"
                    .to_owned(),
            ),
        );
        update(&mut model, Msg::FrameDue);
        settle(model)
    }

    /// A window mid-tool-call — the working thing `breathe` is for: the agent said what
    /// it is doing and a tool has been running since, with nothing arriving. Native
    /// (the breath is the window's) and settled on a foot of the wave, so the still is
    /// the rest frame and a tick from it is the breath.
    #[must_use]
    pub fn working(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_surface(Surface::Native);
        update(
            &mut model,
            Msg::Agent(AgentEvent::Thought(
                "The gates decide whether this commits.".to_owned(),
            )),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::Text(
                "Running the gates before the commit.\n".to_owned(),
            )),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::ToolCall {
                id: "tc-gates".to_owned(),
                title: "cargo nextest run --workspace".to_owned(),
                verb: ToolVerb::Tool,
                status: ToolStatus::InProgress,
                diff: None,
                images: Vec::new(),
                record: Default::default(),
            }),
        );
        settle(model)
    }

    /// A run card open on the stage while its command streams on a pseudo-terminal, the
    /// pane following its tail — the cockpit's own scene (cargo's bold green verbs, a
    /// warning, a failure, a dim note, a truecolor word, a progress bar overwritten in
    /// place) on the native surface, settled.
    #[must_use]
    pub fn pane_following(size: Size) -> Model {
        let mut model = magistr_cockpit::harness::Scenes::terminal_pane(size);
        model.set_surface(Surface::Native);
        settle(model)
    }

    /// [`Self::pane_following`] with the reader scrolled two notches up
    /// inside the card and three more rows arrived under the window:
    /// the pane holds its place, the thumb marks it and the pill counts
    /// them.
    #[must_use]
    pub fn pane_scrolled(size: Size) -> Model {
        let mut model = magistr_cockpit::harness::Scenes::terminal_pane_scrolled(size);
        model.set_surface(Surface::Native);
        settle(model)
    }

    /// [`Self::pane_following`] with the screen raised into the dock by `⌘↵` and the
    /// rise settled: the grip over the docked rows between the feed and the composer, a
    /// third of the stage the first time.
    #[must_use]
    pub fn pane_docked(size: Size) -> Model {
        let mut model = Self::pane_following(size);
        update(
            &mut model,
            Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::SUPER)),
        );
        settle(model)
    }

    /// [`Self::pane_following`] with the kill held on `k` past the dead zone: the
    /// card's head part-way red from the left, the run still streaming under it.
    #[must_use]
    pub fn pane_killing(size: Size) -> Model {
        let mut model = Self::pane_following(size);
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char('k'))));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let dead = TermHold::DEAD.ceil() as usize;
        for _ in 0..dead + 4 {
            update(&mut model, Msg::Tick);
        }
        model
    }

    /// [`Self::pane_killing`] held through the half second: the kill
    /// done, the screen frozen, `killed by you` where the strip was and
    /// on the head's measure.
    #[must_use]
    pub fn pane_killed(size: Size) -> Model {
        let mut model = Self::pane_killing(size);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let hold = TermHold::HOLD.ceil() as usize;
        for _ in 0..hold {
            update(&mut model, Msg::Tick);
        }
        model
    }

    /// View 05's session with the charter proposing a widening: the reader has allowed
    /// every one of fourteen edit asks under `crates/magistr-tui`, and the rail offers
    /// to stop asking — the offer's row under the charter's, `Delegate` at its edge.
    #[must_use]
    pub fn rail_offer(size: Size) -> Model {
        let mut model = Self::mock("05", size).expect("view 05 has a fixture");
        model.set_surface(Surface::Native);
        model.rail_mut().set_offers(vec![CharterOffer::new(
            "edit:crates/magistr-tui",
            "Edit \u{b7} crates/magistr-tui",
            14,
            14,
        )]);
        model
    }

    /// [`Self::pane_docked`] maximized by `⌘⇧↵`: the dock takes the
    /// stage's rows, the rail stays.
    #[must_use]
    pub fn pane_maximized(size: Size) -> Model {
        let mut model = Self::pane_docked(size);
        update(
            &mut model,
            Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::SUPER | Modifiers::SHIFT)),
        );
        settle(model)
    }

    /// [`Self::pane_following`] after the command ended: the same card,
    /// settled — the screen frozen, the cursor gone, the receipt line
    /// reading the exit code and the duration.
    #[must_use]
    pub fn pane_finished(size: Size) -> Model {
        let mut model = magistr_cockpit::harness::Scenes::terminal_pane(size);
        model.set_surface(Surface::Native);
        update(
            &mut model,
            Msg::Agent(AgentEvent::ToolCallUpdate {
                id: "c1".to_owned(),
                title: None,
                status: Some(ToolStatus::Completed),
                diff: None,
                duration_ms: Some(4_812),
                images: Vec::new(),
                record: Box::new(magistr_application::sessions::ToolRecord {
                    output: Some(magistr_application::sessions::ToolPayload::Json(
                        serde_json::json!({"exit_code": 1}).to_string(),
                    )),
                    ..Default::default()
                }),
            }),
        );
        settle(model)
    }

    /// One card of every verb, every one open: a run with its output pane and a cut
    /// record, a read with its range and excerpt, a search with its hits, an edit with
    /// its diff and receipt strip, a fetch, a thought, an MCP tool with its input and
    /// output, and a failed check with the gate's own rows — each carrying the record
    /// the wire would, opened through the Turn's own toggle. Native, and tall enough
    /// that no card scrolls off: pass a size with the rows to hold them.
    #[must_use]
    pub fn cards(size: Size) -> Model {
        use magistr_application::sessions::ToolRecord;
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_surface(Surface::Native);
        let agent = |model: &mut Model, event| update(model, Msg::Agent(event));
        let call = |model: &mut Model, id: &str, title: &str, verb, record: ToolRecord, ms| {
            agent(
                model,
                AgentEvent::ToolCall {
                    id: id.to_owned(),
                    title: title.to_owned(),
                    verb,
                    status: ToolStatus::InProgress,
                    diff: None,
                    images: Vec::new(),
                    record: Box::new(record),
                },
            );
            agent(
                model,
                AgentEvent::ToolCallUpdate {
                    id: id.to_owned(),
                    title: None,
                    status: Some(ToolStatus::Completed),
                    diff: None,
                    duration_ms: Some(ms),
                    images: Vec::new(),
                    record: Box::default(),
                },
            );
        };
        agent(
            &mut model,
            AgentEvent::Text("One card of each verb, every one open.\n".to_owned()),
        );
        call(
            &mut model,
            "c-run",
            "cargo nextest run -p magistr-gui2",
            ToolVerb::Run,
            ToolRecord {
                input: Some(magistr_application::sessions::ToolPayload::Json(serde_json::json!({"command": "cargo nextest run -p magistr-gui2"}).to_string())),
                output: Some(magistr_application::sessions::ToolPayload::Json(serde_json::json!("   Compiling magistr-cockpit v0.1.0\n   Compiling magistr-gui2 v0.1.0\n    \
                 Finished `test` profile [unoptimized + debuginfo] target(s) in 7.36s\n     \
                 Summary [   0.315s] 46 tests run: 46 passed, 492 skipped\n").to_string())),
                truncated_from: Some(236_544),
                ..Default::default()
            },
            7_360,
        );
        call(
            &mut model,
            "c-read",
            "Read comp_feed.rs",
            ToolVerb::Read,
            ToolRecord {
                input: Some(magistr_application::sessions::ToolPayload::Json(
                    serde_json::json!({
                        "file_path": "crates/magistr-gui2/src/comp_feed.rs",
                        "offset": 314, "limit": 4,
                    })
                    .to_string(),
                )),
                output: Some(magistr_application::sessions::ToolPayload::Json(
                    serde_json::json!(
                        "fn card(\n    model: &Model,\n    line: &Line,\n    rows: &[Row],\n"
                    )
                    .to_string(),
                )),
                locations: vec![(
                    String::from("crates/magistr-gui2/src/comp_feed.rs"),
                    Some(314),
                )],
                ..Default::default()
            },
            12,
        );
        call(
            &mut model,
            "c-search",
            "Search card_at_row",
            ToolVerb::Search,
            ToolRecord {
                input: Some(magistr_application::sessions::ToolPayload::Json(
                    serde_json::json!({"pattern": "card_at_row"}).to_string(),
                )),
                output: Some(magistr_application::sessions::ToolPayload::Json(
                    serde_json::json!(
                        "turn/stream.rs:332: pub fn card_at_row(&self, at: usize)\n\
                 comp_feed.rs:137: if let Some((line, span)) = model.turn().card_at_row(at)\n\
                 stream.tests.rs:792: let (line, span) = turn.card_at_row(head)\n"
                    )
                    .to_string(),
                )),
                ..Default::default()
            },
            38,
        );
        agent(
            &mut model,
            AgentEvent::ToolCall {
                id: "c-edit".to_owned(),
                title: "edit cockpit/surface.rs".to_owned(),
                verb: ToolVerb::Edit,
                status: ToolStatus::Completed,
                diff: Some(EditDiff {
                    path: "cockpit/surface.rs".to_owned(),
                    old_text: "pub enum Surface { Terminal }\n".to_owned(),
                    new_text: "pub enum Surface { Terminal, Native }\n".to_owned(),
                }),
                images: Vec::new(),
                record: Box::default(),
            },
        );
        model.turn_mut().set_receipt(
            "c-edit",
            magistr_cockpit::turn::Receipt {
                added: 1,
                removed: 1,
                ordinal: 241,
                reach: Some("cp-4".to_owned()),
                killed: false,
            },
        );
        call(
            &mut model,
            "c-fetch",
            "Fetch the eighth edition",
            ToolVerb::Fetch,
            ToolRecord {
                input: Some(magistr_application::sessions::ToolPayload::Json(
                    serde_json::json!({
                        "url": "https://example.com/design"
                    })
                    .to_string(),
                )),
                output: Some(magistr_application::sessions::ToolPayload::Json(
                    serde_json::json!(
                        "<!doctype html>\n<title>Almagest \u{b7} eighth edition</title>\n\
                 <section id=\"II.k\">the session's components</section>\n"
                    )
                    .to_string(),
                )),
                ..Default::default()
            },
            640,
        );
        call(
            &mut model,
            "c-think",
            "Weighing the two fixes",
            ToolVerb::Think,
            ToolRecord {
                input: Some(magistr_application::sessions::ToolPayload::Json(
                    serde_json::json!({
                        "thought": "The card opens in place; nothing navigates.\n\
                                    Esc folds it before Esc climbs."
                    })
                    .to_string(),
                )),
                ..Default::default()
            },
            2_100,
        );
        call(
            &mut model,
            "c-mcp",
            "mcp__workbench-mcp__read",
            ToolVerb::Tool,
            ToolRecord {
                input: Some(magistr_application::sessions::ToolPayload::Json(serde_json::json!({
                    "file": "crates/magistr-gui2/src/comp_feed.rs", "symbol": "card"
                }).to_string())),
                output: Some(magistr_application::sessions::ToolPayload::Json(serde_json::json!([
                    {"type": "text", "text": "{\"ok\": true, \"results\": [{\"kind\": \"unit\"}]}"}
                ]).to_string())),
                ..Default::default()
            },
            9,
        );
        update(
            &mut model,
            Msg::Engine(EngineEvent::local(fixtures::frame(
                "ship",
                "check_recorded",
                serde_json::json!({"check": {
                    "name": "lint",
                    "command": "cargo clippy --workspace --all-targets -- -D warnings",
                    "passed": false, "exit_code": 101, "duration_ms": 41_200,
                    "details": "error: unused variable: `fade`\n  --> crates/magistr-gui2/src/comp_feed.rs:352:9\nhelp: if this is intentional, prefix it with an underscore: `_fade`",
                }}),
            ))),
        );
        // Open every card, back to front — a later card's toggle moves
        // no earlier card's head — through the Turn's own toggle.
        let heads: Vec<usize> = model
            .turn()
            .lines()
            .iter()
            .filter(|line| {
                matches!(
                    line.kind(),
                    magistr_cockpit::turn::LineKind::Tool { .. }
                        | magistr_cockpit::turn::LineKind::Edit { .. }
                        | magistr_cockpit::turn::LineKind::Check { .. }
                )
            })
            .map(magistr_cockpit::turn::Line::row_start)
            .collect();
        for head in heads.into_iter().rev() {
            model.turn_mut().toggle_at_row(head);
        }
        settle(model)
    }

    /// A completed chunk with a commit, three passing gates, cost and token totals, a
    /// recording, workflow references, and five preview images.
    ///
    /// The previews are rendered from fixtures rather than read from disk. Completion
    /// is two days before the fixture clock.
    #[must_use]
    pub fn shelf() -> Shelf {
        let mut shelf = Shelf::new(
            "gui2-2a",
            &["think:388".to_owned(), "task:verify".to_owned()],
        );
        shelf.shipped_secs = Some(i64::try_from(Self::NOW_SECS).unwrap_or(0) - 2 * 86_400);
        shelf.commit = Some((
            "a3f9c21".to_owned(),
            "feat(gui2): the design language".to_owned(),
        ));
        shelf.gates = vec![
            ("check".to_owned(), true),
            ("test".to_owned(), true),
            ("lint".to_owned(), true),
        ];
        shelf.cost_cents = Some(207);
        shelf.tokens = Some(41_000);
        shelf.recording_secs = Some(12 * 60);
        shelf.stills = vec![
            still("turn-night.png", StillKind::Still, (160, 100)),
            still("sky-night.png", StillKind::Still, (160, 100)),
            still("sky-ignite-mocha.png", StillKind::Filmstrip, (320, 40)),
            still("gate-pop-mocha.png", StillKind::Filmstrip, (320, 40)),
            still("galaxy-breathe-mocha.png", StillKind::Filmstrip, (320, 40)),
        ];
        shelf
    }

    /// A completed chunk opens at its Report with a deliverables shelf. Its Sky card
    /// retains the same gallery, receipts, and Export action.
    #[must_use]
    pub fn shelved(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_surface(Surface::Native)
            .with_tab(StageTab::Roadmap);
        ship_seed(&mut model, true);
        let star = model
            .roadmap()
            .graph_ids()
            .iter()
            .position(|id| *id == "gui2-2a")
            .expect("gui2-2a is on the graph");
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target: Some(Widget::sky_star(star)),
            },
        );
        update(
            &mut model,
            Msg::Job {
                id: JobId::new(7),
                output: JobOutput::Shelf(Box::new(Self::shelf())),
            },
        );
        settle(model)
    }

    /// The survey's report for `tidewatch` — the volume's F2 rows,
    /// word for word: the four facts `magistr init` finds, the four
    /// receipts it leaves, the points the sweep passes (the volume's
    /// own coordinates on its 1100x590 body, as thousandths), and the
    /// chips the interview pre-offers.
    #[must_use]
    pub fn survey_report() -> SurveyReport {
        let mut report = SurveyReport::named("tidewatch");
        report.detected = vec![
            (
                SurveyKind::Stack,
                "Rust \u{b7} edition 2024 \u{b7} cargo workspace, 4 crates".to_owned(),
            ),
            (
                SurveyKind::Tests,
                "nextest \u{b7} 214 tests \u{b7} clippy configured".to_owned(),
            ),
            (
                SurveyKind::Git,
                "main \u{b7} clean \u{b7} 312 commits, 2 authors".to_owned(),
            ),
            (SurveyKind::Agent, "claude-agent-acp on PATH".to_owned()),
        ];
        report.written = vec![
            (
                ScaffoldKind::Config,
                ".magistr/project.json".to_owned(),
                true,
            ),
            (
                ScaffoldKind::Rules,
                ".claude/rules \u{b7} 2 files".to_owned(),
                true,
            ),
            (
                ScaffoldKind::Hooks,
                "write-guard \u{b7} gates".to_owned(),
                true,
            ),
            (ScaffoldKind::Store, "think-and-ship".to_owned(), true),
        ];
        report.points = vec![(391, 407), (482, 542), (545, 373), (455, 712)];
        report.never_break = vec![
            "prediction accuracy".to_owned(),
            "the public API".to_owned(),
            "offline operation".to_owned(),
            "install under 5 min".to_owned(),
        ];
        report
    }

    /// The charting's proposal for `tidewatch` — the volume's F4 list:
    /// eight chunks over three workstreams, standing on one another
    /// the way the drawn constellation pulls its edges.
    #[must_use]
    pub fn proposal() -> Proposal {
        Proposal {
            chunks: vec![
                ProposedChunk::new("harmonic-engine", "core", 100),
                ProposedChunk::new("station-data-import", "core", 110)
                    .standing_on(&["harmonic-engine"]),
                ProposedChunk::new("prediction-accuracy-gate", "core", 120)
                    .standing_on(&["harmonic-engine"]),
                ProposedChunk::new("forecast-endpoint", "api", 130)
                    .standing_on(&["station-data-import", "prediction-accuracy-gate"]),
                ProposedChunk::new("station-admin", "api", 140).standing_on(&["forecast-endpoint"]),
                ProposedChunk::new("single-binary-install", "ops", 150)
                    .standing_on(&["forecast-endpoint"]),
                ProposedChunk::new("offline-tiles", "ops", 160)
                    .standing_on(&["single-binary-install"]),
                ProposedChunk::new("harbor-onboarding", "ops", 170)
                    .standing_on(&["station-admin", "single-binary-install"]),
            ],
            sources: vec![
                "Code scan \u{b7} interview answers".to_owned(),
                "Dependencies and priorities inferred".to_owned(),
            ],
        }
    }

    /// F1 — the ground: `＋ New project` pressed on the Galaxy. The
    /// card floats over the room with its slot waiting; no motion.
    #[must_use]
    pub fn birth_f1(size: Size) -> Model {
        let mut model = Self::galaxy(size);
        update(&mut model, click(Widget::GalaxyNew.id()));
        settle(model)
    }

    /// F2 — the survey: `~/Code/tidewatch` dropped from the desktop,
    /// the survey asked for and its report landed, every row risen —
    /// the sweep still circling, the interview not yet opened.
    #[must_use]
    pub fn birth_f2(size: Size) -> Model {
        let mut model = Self::galaxy(size);
        update(&mut model, Msg::Dropped("~/Code/tidewatch".to_owned()));
        update(
            &mut model,
            Msg::Job {
                id: JobId::new(2),
                output: JobOutput::Survey(Box::new(Self::survey_report())),
            },
        );
        // Every row landed and settled, the survey's beat not yet up,
        // and the sweep back at the top of its turn — where the
        // volume's capture holds it. One turn is longer than the rows
        // take to land and shorter than the landing plus the beat.
        let landed = Self::survey_report().rows() as u64 * Motion::RISE_STAGGER;
        debug_assert!(landed <= Birth::RADAR_STEPS);
        debug_assert!(Birth::RADAR_STEPS < landed + Birth::SURVEY_BEAT);
        for _ in 0..Birth::RADAR_STEPS {
            update(&mut model, Msg::Tick);
        }
        model
    }

    /// F3 — the interview: the survey settled and the first question
    /// answered in the volume's own words, the second's first two
    /// chips picked.
    #[must_use]
    pub fn birth_f3(size: Size) -> Model {
        let mut model = Self::birth_f2(size);
        for _ in 0..(Birth::SURVEY_BEAT + 1) {
            update(&mut model, Msg::Tick);
        }
        for c in "A tide-prediction service small harbors can self-host \u{2014} accurate enough to trust, cheap enough to ignore.".chars() {
            update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
        }
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
        update(&mut model, click(Widget::birth_chip(0)));
        update(&mut model, click(Widget::birth_chip(1)));
        settle(model)
    }

    /// F4 — the charting: the interview's last two answers given, the
    /// proposal drafted and just landed — the constellation at the
    /// first frame of its birth, which is what the volume's capture
    /// holds; the born filmstrip pins the drawing.
    #[must_use]
    pub fn birth_f4(size: Size) -> Model {
        let mut model = Self::birth_f3(size);
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
        update(&mut model, click(Widget::birth_chip(2)));
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
        let mut model = settle(model);
        update(
            &mut model,
            Msg::Job {
                id: JobId::new(3),
                output: JobOutput::Proposal(Box::new(Self::proposal())),
            },
        );
        model
    }

    /// F5 — first light: the proposal accepted and written, the Sky
    /// seeded from what was written, the toast up with its first claim.
    #[must_use]
    pub fn birth_f5(size: Size) -> Model {
        let mut model = settle(Self::birth_f4(size));
        update(&mut model, click(Widget::BirthAccept.id()));
        let proposal = Self::proposal();
        update(
            &mut model,
            Msg::Job {
                id: JobId::new(4),
                output: JobOutput::Born(Box::new(BornData {
                    name: "tidewatch".to_owned(),
                    dir: "~/Code/tidewatch".to_owned(),
                    chunks: proposal.records(),
                    gates: vec!["cargo nextest run".to_owned()],
                    protected: Vec::new(),
                })),
            },
        );
        settle(model)
    }

    /// A turn *and* a roadmap: the one surface that puts all three
    /// chrome slots on screen at once — the brand mark, the activity
    /// monitor a busy session earns, and the rail's gauge. What the
    /// convergence rule is measured against, and the reason it is a
    /// rule at all.
    #[must_use]
    pub fn busy(size: Size) -> Model {
        let mut model = Self::turn(size);
        seed(&mut model);
        settle(model)
    }

    /// The Trace surface: a branched reasoning trace on its own tab —
    /// two main-line steps, a two-step branch forked off the second,
    /// and a return to the main line, fed through the same
    /// `think/step_appended` frames the engines broadcast. The fork is
    /// what makes [`Trace::has_branches`] answer yes, which is what
    /// makes the view place the graph block at all.
    ///
    /// [`Trace::has_branches`]: magistr_cockpit::cockpit::Trace::has_branches
    #[must_use]
    pub fn traced(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_place(Place::Trace);
        let step = |model: &mut Model, payload: serde_json::Value| {
            update(
                model,
                Msg::Engine(EngineEvent::local(fixtures::frame(
                    "think",
                    "step_appended",
                    payload,
                ))),
            );
        };
        step(
            &mut model,
            serde_json::json!({"step": {"step_number": 1, "purpose": "analysis"}}),
        );
        step(
            &mut model,
            serde_json::json!({"step": {"step_number": 2, "purpose": "hypothesis"}}),
        );
        for number in [3, 4] {
            step(
                &mut model,
                serde_json::json!({"step": {
                    "step_number": number,
                    "branch_id": "alt",
                    "branch_name": "the alternative",
                    "branch_from": 2,
                }}),
            );
        }
        step(
            &mut model,
            serde_json::json!({"step": {"step_number": 5, "purpose": "conclusion"}}),
        );
        settle(model)
    }

    /// The Roadmap surface: the shared seed on its own tab — what the
    /// tech-tree and its rect-report loop are proven on.
    #[must_use]
    pub fn roadmapped(size: Size) -> Model {
        let mut model = Model::new(size).with_tab(StageTab::Roadmap);
        seed(&mut model);
        settle(model)
    }

    /// The Changes surface: the workbench journal on its own tab — two
    /// entries, one that parses and one that does not, which is what
    /// puts both verdicts on the same still.
    #[must_use]
    pub fn changed(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_place(Place::Changes);
        update(
            &mut model,
            Msg::Workbench(WorkbenchEvent::Journal {
                entries: vec![
                    JournalEntry {
                        at_ms: None,
                        id: "e1".to_owned(),
                        path: "crates/magistr-gui2/src/paint.rs".to_owned(),
                        added: 12,
                        removed: 4,
                        parse_ok: true,
                        diff: "@@ -1,1 +1,2 @@\n-fn paint(&self) {}\n+fn paint(&mut self) {}\n                               +    self.runs.push(run);"
                            .to_owned(),
                    },
                    JournalEntry {
                        at_ms: None,
                        id: "e2".to_owned(),
                        path: "crates/magistr-gui2/src/access.rs".to_owned(),
                        added: 3,
                        removed: 0,
                        parse_ok: false,
                        diff: "+fn tree( {".to_owned(),
                    },
                ],
            }),
        );
        settle(model)
    }

    /// The accelerator sheet over the launcher — the one surface that is
    /// an *overlay*, and at both still sizes a window onto a registry
    /// that runs past the stage, so its position readout is on screen.
    #[must_use]
    pub fn sheeted(size: Size) -> Model {
        let mut model = Self::home(size);
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char('?'))));
        settle(model)
    }

    /// A turn carrying a decoded picture: prose, then a `width` ×
    /// `height` red PNG as the agent's image block — the stream's one
    /// placement, over the rows its footprint takes.
    #[must_use]
    pub fn imaged(size: Size, width: u32, height: u32) -> Model {
        let mut model = Model::new(size)
            .with_surface(Surface::Native)
            .with_tab(StageTab::Turn);
        update(
            &mut model,
            Msg::Agent(AgentEvent::Text("here it is:\n".to_owned())),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::Image(ImageBlock {
                mime_type: "image/png".to_owned(),
                data: png_base64(width, height),
                uri: None,
            })),
        );
        settle(model)
    }

    /// A tool call that returned a picture: prose, then "Take a screenshot" announced
    /// and completed with a 300 × 120 px PNG on its content — the Session view's
    /// screenshot at its own pixels under the card that produced it. The picture is a
    /// checkerboard with a border, so a still shows at once whether it was resampled: a
    /// resampled checker blurs at every square's edge.
    #[must_use]
    pub fn screenshot(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_surface(Surface::Native)
            .with_tab(StageTab::Turn);
        update(
            &mut model,
            Msg::Agent(AgentEvent::Text("Checking the page:\n".to_owned())),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::ToolCall {
                id: "tc-shot".to_owned(),
                title: "Take a screenshot".to_owned(),
                verb: ToolVerb::Tool,
                status: ToolStatus::InProgress,
                diff: None,
                images: Vec::new(),
                record: Default::default(),
            }),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::ToolCallUpdate {
                id: "tc-shot".to_owned(),
                title: None,
                status: Some(ToolStatus::Completed),
                diff: None,
                duration_ms: Some(420),
                images: vec![ImageBlock {
                    mime_type: "image/png".to_owned(),
                    data: checker_base64(300, 120),
                    uri: None,
                }],
                record: Default::default(),
            }),
        );
        settle(model)
    }

    /// The 19 views of the blessed volume, by capture id — the ids the
    /// mock parity gate walks, so a view with no fixture is reported by
    /// name rather than forgotten. `01`–`14` are the volume's numbered
    /// views and `F1`–`F5` its flows; each has a `night-NN.png` and a
    /// `day-NN.png` under `tests/mocks`.
    pub const VIEWS: [&str; 19] = [
        "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13", "14", "F1",
        "F2", "F3", "F4", "F5",
    ];

    /// The captures' pixel size — the volume's application window,
    /// which is 1100x640 in every one of the nineteen views, and what a
    /// still is rendered at so the diff is pixel for pixel.
    ///
    /// It is the *window* and not the `.fit` box the window sits in:
    /// the captures used to carry the box's full 1132x641, a row of
    /// page background above the window and 32 columns of it to the
    /// right, and rendering the cockpit on that size laid it out 32px
    /// wider than the design's own window — putting the whole
    /// right-anchored supervision rail 25px right of where the volume
    /// draws it, inside the very region the mask excused as browser
    /// chrome.
    pub const CAPTURE: (u32, u32) = (1100, 640);

    /// The cockpit grid a capture-sized still is laid out on, for a
    /// cell of `cell` pixels: as many whole cells as fit. The layout
    /// fills `cols × cell` of the viewport and the clear colour fills
    /// the rest, which the diff then measures like any other pixel.
    #[must_use]
    pub fn capture_grid(cell: (f32, f32)) -> Size {
        let (width, height) = Self::CAPTURE;
        let cols = (width as f32 / cell.0)
            .floor()
            .clamp(1.0, f32::from(u16::MAX)) as u16;
        let rows = (height as f32 / cell.1)
            .floor()
            .clamp(1.0, f32::from(u16::MAX)) as u16;
        Size::new(cols, rows)
    }

    /// The model the volume's view `view` shows, or `None` for a view
    /// no fixture builds yet — which the parity gate reports as SKIP by
    /// name, never as a pass.
    ///
    /// A mock fixture uses the capture's own words wherever they are
    /// data (the chunk id, the iteration count, the token count, the
    /// diff line) and the cockpit's own messages to put them there, so
    /// the still is the view the design shows on the model the cockpit
    /// really keeps.
    #[must_use]
    pub fn mock(view: &str, size: Size) -> Option<Model> {
        match view {
            "01" => Some(Self::galaxy_01(size)),
            "03" => Some(Self::sky_03(size)),
            "04" => Some(Self::sky_04(size)),
            "05" => Some(Self::session_05(size)),
            "06" => Some(Self::moment_06(size)),
            "07" => Some(Self::review_07(size)),
            "08" => Some(Self::moment_08(size)),
            "09" => Some(Self::shelved(size)),
            "11" => Some(Self::trace_11(size)),
            "12" => Some(Self::signals_12(size)),
            "13" => Some(Self::search_13(size)),
            "10" => Some(Self::settings_10(size)),
            "14" => Some(Self::bad_day_14(size)),
            "F1" => Some(Self::birth_f1(size)),
            "F2" => Some(Self::birth_f2(size)),
            "F3" => Some(Self::birth_f3(size)),
            "F4" => Some(Self::birth_f4(size)),
            "F5" => Some(Self::birth_f5(size)),
            _ => None,
        }
    }

    /// The wall clock the mock stills are drawn at — 2026-09-01T12:00Z,
    /// in seconds since the epoch — so a capture's ages (`2 d`, `5 d`,
    /// `7 d`) come off real `created` stamps against a pinned now
    /// rather than off a clock the test happens to run at.
    pub const NOW_SECS: u64 = 1_788_264_000;

    /// View 14 — a bad day, the doctrine demonstrated: the Sky with
    /// `e6b` on the rail, and three failures the cockpit already keeps
    /// read as cards. The agent binary is gone (`claude`,
    /// last detected `09:14`, as the entry point stamped it); the
    /// budget's breaker at `10.00` tripped an hour into the loop, after
    /// the spine had saved `cp-6`; the `tidewatch-server` feed hung up
    /// three minutes ago, on the loop's own clock. Two of the three
    /// block, which is the header's count. Every word on the cards is
    /// [`BadDay::of`]'s, off those facts — the fixture writes none.
    ///
    /// [`BadDay::of`]: magistr_cockpit::cockpit::BadDay::of
    #[must_use]
    pub fn bad_day_14(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_surface(Surface::Native)
            .with_tab(StageTab::Roadmap)
            .with_server("tidewatch-server")
            .with_agent_missing(AgentMissing::new("claude").seen("09:14"));
        let done = |id: &str, deps: Vec<&str>| {
            serde_json::json!({
                "id": id, "name": id, "status": "done", "priority": 200,
                "group": "core", "deps": deps,
            })
        };
        update(
            &mut model,
            Msg::Engine(EngineEvent::Seed(NativeSnapshot {
                progress: fixtures::progress(211, 306),
                chunks: fixtures::chunks_from(vec![
                    done("layout", vec![]),
                    serde_json::json!({
                        "id": "e6b", "name": "e6b", "status": "in_progress",
                        "priority": 300, "group": "core", "deps": ["layout"],
                    }),
                ]),
                signals: fixtures::signals_from(Vec::new()),
                ..NativeSnapshot::default()
            })),
        );
        model
            .rail_mut()
            .set_budget(Budget::capped(1200).map(|b| b.spent(1000).stop_at(1000)));
        for _ in 0..6 {
            model
                .rail_mut()
                .spine_mut()
                .open(CheckpointReason::Iteration, "iteration");
        }
        model.rail_mut().spine_mut().saved("cp-6", None, true);
        update(
            &mut model,
            Msg::Loop(LoopEvent::Started {
                iteration: 6,
                max: 10,
            }),
        );
        let hour = std::time::Duration::from_secs(3600);
        update(&mut model, Msg::Loop(LoopEvent::Elapsed(hour)));
        update(
            &mut model,
            Msg::Loop(LoopEvent::Paused("the 10.00 USD limit".to_owned())),
        );
        update(
            &mut model,
            Msg::Loop(LoopEvent::Elapsed(
                2 * hour - 3 * std::time::Duration::from_secs(60),
            )),
        );
        update(
            &mut model,
            Msg::Engine(EngineEvent::Feed(FeedStatus::Disconnected)),
        );
        update(&mut model, Msg::Loop(LoopEvent::Elapsed(2 * hour)));
        settle(model)
    }

    /// Signals inbox with one new bug, one researched concern, and one promoted bug.
    /// The promoted signal references a completed roadmap chunk; the header counts
    /// the other two signals as open.
    #[must_use]
    pub fn signals_12(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_surface(Surface::Native)
            .with_place(Place::Signals);
        update(
            &mut model,
            Msg::Engine(EngineEvent::Seed(NativeSnapshot {
                progress: fixtures::progress(211, 306),
                chunks: fixtures::chunks_from(vec![serde_json::json!({
                    "id": "e10c-session-migration", "name": "session migration",
                    "status": "done", "priority": 250, "group": "enterprise",
                })]),
                signals: fixtures::signals_from(vec![
                    serde_json::json!({
                        "id": "sig-retina", "kind": "bug", "from": "developer", "status": "new",
                        "created": "2026-08-30T12:00:00Z",
                        "body": "Affects all native windows. Cell metrics ignore the display scale factor.",
                        "content": {"version": 1, "summary": "Retina window renders text at half size"},
                    }),
                    serde_json::json!({
                        "id": "sig-a11y", "kind": "concern", "from": "team", "status": "researched",
                        "created": "2026-08-27T12:00:00Z",
                        "body": "Screen readers can read the tree but cannot press anything in it.",
                        "content": {
                            "version": 1,
                            "summary": "Screen readers cannot trigger actions in the accessibility tree",
                        },
                        "enrichment": [{
                            "summary": "AccessKit 0.24 supports actions. All actions are currently refused (gui2-accesskit-actions). Estimated effort: 1 chunk.",
                            "confidence": 0.8, "sources": [], "at": "2026-08-28T09:00:00Z",
                        }],
                    }),
                    serde_json::json!({
                        "id": "sig-pause", "kind": "bug", "from": "developer", "status": "promoted",
                        "created": "2026-08-25T12:00:00Z",
                        "body": "A paused loop forgets it was paused when the machine changes.",
                        "content": {"version": 1, "summary": "Loop pause does not survive a machine change"},
                        "cross_refs": ["chunk:e10c-session-migration"],
                    }),
                ]),
                ..NativeSnapshot::default()
            })),
        );
        settle(model)
    }

    /// View 13 — ⌘⇧F over the dimmed session: `contrast floor` typed
    /// into the search, six hits, four of them shown. The corpus is
    /// what the cockpit really keeps: four shipped chunks whose
    /// descriptions carry the phrase or its words (their `updated_at`
    /// stamps are the capture's `Aug 29` / `Aug 30`), think step 387
    /// whose thought asks after the floor, and ship action #198 on
    /// `theme/ink.rs` — all seeded through the store seed and read
    /// back by [`Search::results`], so the count and every row are the
    /// matcher's answer, not strings this fixture wrote. The session
    /// behind is view 05's prose row, dimmed by the view.
    ///
    /// [`Search::results`]: magistr_cockpit::cockpit::Search::results
    #[must_use]
    pub fn search_13(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_surface(Surface::Native);
        let chapter = |id: &str, title: &str, description: &str, updated: &str| {
            serde_json::json!({
                "id": id, "name": id, "title": title, "status": "done", "priority": 200,
                "group": "desktop-gui", "description": description, "updated_at": updated,
            })
        };
        update(
            &mut model,
            Msg::Engine(EngineEvent::Seed(NativeSnapshot {
                progress: fixtures::progress(211, 306),
                chunks: fixtures::chunks_from(vec![
                    chapter(
                        "latte-floor",
                        "Every light palette clears its floor",
                        "The Latte pens are all contrast-checked, nudged until it clears the \
                         contrast floor the validator declares, role by role.",
                        "2026-08-29T15:00:00Z",
                    ),
                    chapter(
                        "pane-parity",
                        "The native pane at parity",
                        "Stills in both media are measured; every run clears the cell pens\u{2019} \
                         own contrast floor (crates/magistr-gui2/src/lexicon.rs).",
                        "2026-08-30T09:30:00Z",
                    ),
                    chapter(
                        "cell-sweep",
                        "The contrast sweep",
                        "A sweep over every painted cell asks the floor of each pen.",
                        "2026-08-27T09:30:00Z",
                    ),
                    chapter(
                        "ink-consumer",
                        "Ink at the consumer",
                        "The consumer fixes its own floor; the palette is never retuned.",
                        "2026-08-28T09:30:00Z",
                    ),
                    serde_json::json!({
                        "id": "gui2-2a", "name": "gui2-2a design", "status": "in_progress",
                        "priority": 159, "group": "desktop-gui",
                    }),
                ]),
                signals: fixtures::signals_from(Vec::new()),
                steps: vec![fixtures::step_from(serde_json::json!({
                    "step_number": 387,
                    "purpose": "the pen on screen",
                    "thought": "Anti-aliasing is a rasterizer-question; the pen on screen \
                                is style after REVERSE, and the floor is asked of what was \
                                painted, never of the palette.",
                }))],
                tasks: vec![fixtures::task_from(
                    serde_json::json!({"id":"ink-consumer","actions":[{
                        "id": 198, "action_type": "edit",
                        "description": "ensure_contrast at the consumer",
                        "files_touched": ["theme/ink.rs"],
                    }]}),
                )],
                ..NativeSnapshot::default()
            })),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::Text(
                "The vocabulary is 48 static ids plus 20 indexed spaces\u{2026}\n".to_owned(),
            )),
        );
        update(
            &mut model,
            Msg::Key(KeyEvent::new(KeyCode::Char('F')).with(Modifiers::SUPER | Modifiers::SHIFT)),
        );
        for c in "contrast floor".chars() {
            update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
        }
        settle(model)
    }

    /// View 10 — ⌘, over view 05's session: the Appearance pane with
    /// the theme following the OS, both switches on and the density
    /// comfortable — a fresh install's values — under the project's
    /// identity (`magistr ~/Code/magistr`) and a team of four. The
    /// surface is raised through the chord like a hand would.
    #[must_use]
    pub fn settings_10(size: Size) -> Model {
        let mut model = Self::session_05(size).with_settings(SettingsData {
            project_path: "~/Code/magistr".to_owned(),
            team_size: Some(4),
            ..SettingsData::default()
        });
        model.set_surface(Surface::Native);
        update(
            &mut model,
            Msg::Key(KeyEvent::new(KeyCode::Char(',')).with(Modifiers::SUPER)),
        );
        settle(model)
    }

    /// View 11: a trace with 102 main-line steps, eight pins, and one retained branch.
    /// Opening a step updates the displayed counts and receipts through the shared
    /// model.
    ///
    /// The fixture's execution references also appear in roadmap `cross_refs`, so
    /// [`Roadmap::chunk_for_ref`] resolves the associated chunk.
    ///
    /// [`Roadmap::chunk_for_ref`]: magistr_cockpit::cockpit::Roadmap::chunk_for_ref
    #[must_use]
    pub fn trace_11(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_surface(Surface::Native)
            .with_place(Place::Trace);
        update(
            &mut model,
            Msg::Engine(EngineEvent::Seed(NativeSnapshot {
                progress: fixtures::progress(211, 306),
                chunks: fixtures::chunks_from(vec![
                    serde_json::json!({
                        "id": "gui2-2-session-pane", "name": "session pane", "status": "done",
                        "priority": 158, "group": "desktop-gui",
                    }),
                    serde_json::json!({
                        "id": "gui2-2a", "name": "gui2-2a design", "status": "in_progress",
                        "priority": 159, "group": "desktop-gui",
                        "cross_refs": ["task:design-canvas"],
                    }),
                ]),
                signals: fixtures::signals_from(Vec::new()),
                ..NativeSnapshot::default()
            })),
        );
        let step = |model: &mut Model, step: serde_json::Value| {
            update(
                model,
                Msg::Engine(EngineEvent::local(fixtures::frame(
                    "think",
                    "step_appended",
                    serde_json::json!({"step": step}),
                ))),
            );
        };
        for number in 287..=389u32 {
            if number == 387 {
                continue;
            }
            let mut payload = serde_json::json!({
                "step_number": number,
                "purpose": format!("step {number}"),
            });
            if matches!(number, 300 | 310 | 320 | 330 | 340 | 381 | 388) {
                payload["pinned"] = serde_json::Value::Bool(true);
            }
            match number {
                381 => payload["purpose"] = "gui2 is the flagship".into(),
                386 => {
                    payload["purpose"] = "one inventory".into();
                    payload["thought"] = "One inventory serves the contrast floor and the \
                                          accessible names \u{2014} two questions, one walk."
                        .into();
                    payload["execution_ref"] = "chunk:gui2-2-session-pane".into();
                }
                388 => {
                    payload["purpose"] = "open gui2-2a".into();
                    payload["thought"] = "Open gui2-2a: the IA re-sited not re-skinned; three \
                                          directions that genuinely disagree; the mechanism \
                                          reuses every widget id. Confidence .8; the one call I \
                                          cannot make alone is the choice itself."
                        .into();
                    payload["execution_ref"] = "task:design-canvas".into();
                }
                389 => {
                    payload["purpose"] = "close gui2-2a".into();
                    payload["thought"] =
                        "Close gui2-2a: nine volumes, ten decisions, the catalog rule".into();
                }
                _ => {}
            }
            step(&mut model, payload);
            if number == 386 {
                // The branch forks off 386 and lands before 388 in the
                // feed, as it did in the trace.
                step(
                    &mut model,
                    serde_json::json!({
                        "step_number": 387,
                        "purpose": "close session pane",
                        "branch_id": "alt",
                        "branch_name": "the alternative",
                        "branch_from": 386,
                        "pinned": true,
                    }),
                );
            }
        }
        update(
            &mut model,
            Msg::Engine(EngineEvent::local(fixtures::frame(
                "think",
                "branch_status_changed",
                serde_json::json!({"branch_id": "alt", "status": "merged", "merged_into": 388}),
            ))),
        );
        // 388 opened, through the cockpit's own click on its row.
        if let Some(index) = model
            .trace()
            .steps()
            .iter()
            .position(|step| step.number == 388)
        {
            update(
                &mut model,
                Msg::Mouse {
                    event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                    target: Some(Widget::trace_row(index)),
                },
            );
        }
        settle(model)
    }

    /// The project overview fixture has three projects: two with running sessions, one
    /// awaiting an answer, and one idle. It shares a twelve-dollar spending cap with a
    /// ten-dollar stop threshold.
    #[must_use]
    pub fn galaxy(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_surface(Surface::Native)
            .with_place(Place::Galaxy);
        update(
            &mut model,
            Msg::Job {
                id: JobId::new(0),
                output: JobOutput::Launcher(Box::new(LauncherData {
                    project: "think-and-ship".to_owned(),
                    has_config: true,
                    stack: "Rust / Cargo".to_owned(),
                    git: "master, clean".to_owned(),
                    backend: "ACP (available)".to_owned(),
                    sessions: Vec::new(),
                })),
            },
        );
        update(
            &mut model,
            Msg::Job {
                id: JobId::new(1),
                output: JobOutput::Galaxy(Box::new(GalaxyData {
                    projects: vec![
                        GalaxyProject {
                            name: "magistr".to_owned(),
                            done: Some(211),
                            running: 1,
                            ..GalaxyProject::default()
                        },
                        GalaxyProject {
                            name: "think-and-ship".to_owned(),
                            here: true,
                            running: 1,
                            asking: 1,
                            asked_secs: Some(11 * 60),
                            ..GalaxyProject::default()
                        },
                        GalaxyProject {
                            name: "quicksilver".to_owned(),
                            idle_secs: Some(3 * 86_400),
                            ..GalaxyProject::default()
                        },
                    ],
                    spent_cents: 245,
                    cap_cents: Some(1200),
                    stop_cents: Some(1000),
                })),
            },
        );
        settle(model)
    }

    /// View 01 — the ⌘K palette over the Galaxy: `thi` typed, the
    /// think-and-ship place chosen with its live badge (`1 action
    /// required`, the dock's own count of a real plan-less ask) and the
    /// pending act under it — every string on the card derived from
    /// state the cockpit really keeps, raised by the same ⌘K and the
    /// same three keystrokes a hand would send, over [`Fixture::galaxy`].
    #[must_use]
    pub fn galaxy_01(size: Size) -> Model {
        let mut model = Self::galaxy(size);
        // A plan-less ask: it counts on the dock (the badge, and the
        // window's own cluster) without raising the Moment — the
        // capture's palette floats over a fully lit room, not a scrim.
        update(
            &mut model,
            Msg::Agent(AgentEvent::PermissionRequest {
                id: 1,
                title: "Write a file".to_owned(),
                choices: vec![PermissionChoice::new("allow", PermissionKind::AllowOnce)],
                edit: false,
                risk: RiskTier::Low,
                plan: None,
            }),
        );
        update(
            &mut model,
            Msg::Key(KeyEvent::new(KeyCode::Char('k')).with(Modifiers::SUPER)),
        );
        for c in "thi".chars() {
            update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
        }
        settle(model)
    }

    /// View 03: a constellation of seven completed chunks converging on one active
    /// chunk, without navigation controls.
    ///
    /// This mostly empty view uses a 0.5% parity threshold; the usual 1.5% would allow
    /// foreground differences to be hidden by the background. `RoadmapDag` places stars
    /// by dependency depth rather than using fixed capture coordinates.
    #[must_use]
    pub fn sky_03(size: Size) -> Model {
        let mut model = Model::new(size).with_tab(StageTab::Roadmap);
        sky_seed(&mut model, false);
        settle(model)
    }

    /// View 04 adds a ready chunk and a stuck chunk to the resting constellation. The
    /// stuck chunk has a red dashed ring, matching its status chip.
    #[must_use]
    pub fn sky_04(size: Size) -> Model {
        let mut model = Model::new(size).with_tab(StageTab::Roadmap);
        sky_seed(&mut model, true);
        settle(model)
    }

    /// View 08 after completion: the counter reads `212 / 306`, three gates pass, and
    /// the newly ready chunk appears at the end of a dashed edge.
    ///
    /// Parity compares the settled still. [`Fixture::ship_08`] exercises the completion
    /// animation separately.
    #[must_use]
    pub fn moment_08(size: Size) -> Model {
        let mut model = Model::new(size).with_tab(StageTab::Roadmap);
        ship_seed(&mut model, true);
        green_gates(&mut model);
        settle(model)
    }

    /// View 08 during completion. The initial counter is `211`; the engine's
    /// `chunk_completed` frame starts the animation from a settled model tick.
    /// Filmstrip panels follow the flare and its outgoing edge.
    #[must_use]
    pub fn ship_08(size: Size) -> Model {
        let mut model = Model::new(size).with_tab(StageTab::Roadmap);
        ship_seed(&mut model, false);
        green_gates(&mut model);
        let mut model = settle(model);
        update(
            &mut model,
            Msg::Engine(EngineEvent::local(fixtures::frame(
                "roadmap",
                "chunk_completed",
                serde_json::json!({
                    "chunk": {
                        "id": "gui2-2a", "name": "gui2-2a design", "status": "done",
                        "priority": 159, "group": "desktop-gui", "deps": ["motion"],
                    }
                }),
            ))),
        );
        model
    }

    /// View 06 — the Moment: the session of view 05 dimmed behind a
    /// centred plan card asking to re-bless twelve goldens, with the
    /// checkpoint the spine minted for it, three numbered steps, a
    /// picture to show and `goldens` as the scope a session-wide allow
    /// would cover.
    ///
    /// Built on the *same* session the capture dims behind it (the
    /// volume draws view 05's own prose row through the scrim), and
    /// raised by the ask the cockpit really folds — the checkpoint name
    /// on the card is `Spine::open`'s, minted in that fold, not a
    /// string this fixture wrote.
    fn moment_06(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_surface(Surface::Native)
            .with_tab(StageTab::Turn);
        seed_chunk(&mut model, Some("gui2-2a"));
        // The capture's session behind the scrim is one prose row and the phase it is
        // in — no receipts, no supervision rail, no composer. That is not this fixture
        // being lazy: third altitude is "one decision, everything needed to make it,
        // nothing else", and the view is what takes the chrome away.
        update(
            &mut model,
            Msg::Engine(EngineEvent::local(
                magistr_workflow_contracts::NativeEvent::Magistr(
                    magistr_workflow_contracts::MagistrFrame::PhaseSet {
                        phase: magistr_domain::workflow::WorkflowPhase::Executing,
                    },
                ),
            )),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::Text(
                "The vocabulary is 48 static ids plus 20 indexed spaces — a native \
                 arrangement reuses all of them…\n"
                    .to_owned(),
            )),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::PermissionRequest {
                id: 1,
                title: "Re-bless 12 golden screenshots".to_owned(),
                choices: vec![
                    PermissionChoice::new("allow", PermissionKind::AllowOnce),
                    PermissionChoice::new("allow_always", PermissionKind::AllowAlways),
                    PermissionChoice::new("reject", PermissionKind::RejectOnce),
                ],
                edit: false,
                // High is what mints the checkpoint the card names.
                risk: RiskTier::High,
                plan: Some(
                    AskPlan::new([
                        "Render the 6 surfaces in both themes",
                        "Compare with goldens. Only status-bar rows may change",
                        "Update matching goldens. Reject all others",
                    ])
                    .scoped("goldens")
                    .with_images(vec![ImageBlock {
                        mime_type: "image/png".to_owned(),
                        data: png_base64(300, 120),
                        uri: None,
                    }]),
                ),
            }),
        );
        settle(model)
    }

    /// View 07 — the Moment: a review. A finished theme sweep asking
    /// for an eye: three files written since `cp-3`, the three gates
    /// green in the header beside *Approve & merge*, the agent that did
    /// it named with what it cost, `theme/ink.rs`'"'"'s own diff in the card
    /// and the agent'"'"'s one-line *why* under it.
    ///
    /// Every part of it is state the cockpit really keeps: the files
    /// are the workbench journal, the scope is the spine'"'"'s newest
    /// verified checkpoint, the gates are the loop'"'"'s own LEDs settled
    /// by real job results, and the note is the last thing the agent
    /// thought. Only the agent identity block is seeded, the way the
    /// charter and the budget are — nothing meters a live session'"'"'s
    /// model name.
    fn review_07(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_surface(Surface::Native)
            .with_tab(StageTab::Turn);
        seed_chunk(&mut model, Some("theme sweep · review"));
        {
            let spine = model.rail_mut().spine_mut();
            for label in ["iter 1", "iter 2", "iter 3"] {
                let id = spine.open(CheckpointReason::Iteration, label);
                spine.saved(&id, Some("abc123".to_owned()), true);
            }
            model.rail_mut().set_agent(Some(
                SessionAgent::new("sonnet", "Draft mode").spent(61, 12, 2),
            ));
        }
        update(
            &mut model,
            Msg::Loop(LoopEvent::Started {
                iteration: 4,
                max: 12,
            }),
        );
        for (index, gate) in ["check", "test", "lint"].into_iter().enumerate() {
            let job = JobId::new(index as u64 + 40);
            update(
                &mut model,
                Msg::Loop(LoopEvent::GateStarted {
                    name: gate.to_owned(),
                    job,
                }),
            );
            update(
                &mut model,
                Msg::Job {
                    id: job,
                    output: JobOutput::Text("ok".to_owned()),
                },
            );
        }
        update(
            &mut model,
            Msg::Workbench(WorkbenchEvent::Journal {
                entries: vec![
                    journal("e1", "theme/ink.rs", 31, INK_DIFF),
                    journal(
                        "e2",
                        "theme/css.rs",
                        9,
                        "@@ -40,1 +40,2 @@\n let ink = ink(field);\n+let ink = ink(field, palette);",
                    ),
                    journal(
                        "e3",
                        "theme/css.tests.rs",
                        40,
                        "+assert_eq!(ink(Text, &latte), floor);",
                    ),
                ],
            }),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::Thought(
                "the consumer is protected, not the palette — all thirteen built-ins and \
                 every user TOML clear the floor through this one site"
                    .to_owned(),
            )),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::PermissionRequest {
                id: 7,
                title: "Apply the theme sweep".to_owned(),
                choices: vec![
                    PermissionChoice::new("allow", PermissionKind::AllowOnce),
                    PermissionChoice::new("reject", PermissionKind::RejectOnce),
                ],
                edit: true,
                risk: RiskTier::Medium,
                plan: None,
            }),
        );
        settle(model)
    }

    /// View 05: iteration 4 of 12, prose before and after a completed one-line edit of
    /// `cockpit/surface.rs`, and 38,204 tokens spent.
    fn session_05(size: Size) -> Model {
        // The header's started stop — the label the entry point supplies.
        let mut model = Model::new(size)
            .with_tab(StageTab::Turn)
            .with_session_started("09:02");
        seed_chunk(&mut model, Some("gui2-2a"));
        // The capture's supervision rail: a ledger of four states with
        // the newest two labelled as the volume labels them, one receipt
        // from an earlier turn (the visible edit below is the second),
        // the charter's three rows, and the bound budget. Seeded through
        // the rail's own construction-time setters — the spine's verbs
        // are the same ones `update` calls, and the charter and budget
        // have no live feed to arrive on.
        {
            let rail = model.rail_mut();
            let spine = rail.spine_mut();
            // The session's history before this window: 239 receipts,
            // so the counter the card's `receipt #241` and the head's
            // `2 receipts after cp-4` both read is one number.
            spine.seed_receipts(239);
            // The design's rows: iter 1 as a bare boundary, iter 2 and
            // 3 wearing three green gates, then a manual state. The
            // marks are the checkpoints' own gate data, drawn by the
            // rail in the icon set's glyphs — never a literal in a label.
            let green = || {
                ["check", "test", "lint"]
                    .map(|gate| (gate.to_owned(), true))
                    .to_vec()
            };
            let rows = [
                spine.open_boundary(1, Vec::new()),
                spine.open_boundary(2, green()),
                spine.open_boundary(3, green()),
                spine.open(CheckpointReason::Manual, "before golden update"),
            ];
            for id in rows {
                spine.saved(&id, Some("abc123".to_owned()), true);
            }
            spine.note_receipt();
            rail.set_charter(vec![
                CharterRule::new("Edit · magistr-tui", RuleMode::Ask),
                CharterRule::new("Run · check, test", RuleMode::Auto),
                CharterRule::new("git push", RuleMode::Off),
            ]);
            // The mode word the charter's cap reads — the volume's
            // `Charter · Draft mode`.
            rail.set_mode("Draft mode");
            rail.set_budget(Budget::capped(1200).map(|b| b.spent(184).stop_at(1000)));
            // The handshake named the model: the agent block and the
            // identity band's `sonnet · Draft mode` are bound from it.
            rail.bind_agent(Some("sonnet"));
        }
        // The iteration first, then the phase inside it — the order a
        // real run has, and the only order that holds: `LoopEvent::
        // Started` begins a fresh iteration, and a fresh iteration has
        // no phase yet, so a phase announced before it is wiped by it.
        update(
            &mut model,
            Msg::Loop(LoopEvent::Started {
                iteration: 4,
                max: 12,
            }),
        );
        // The capture's bottom bar lights `execute` with research and
        // plan crossed. The phase rides the ship engine's own frame —
        // the one kind `Phase::from_frame` reads and `turn_frame`
        // leaves off the feed, so the stage shows exactly the capture's
        // rows.
        update(
            &mut model,
            Msg::Engine(EngineEvent::local(
                magistr_workflow_contracts::NativeEvent::Magistr(
                    magistr_workflow_contracts::MagistrFrame::PhaseSet {
                        phase: magistr_domain::workflow::WorkflowPhase::Executing,
                    },
                ),
            )),
        );
        let agent = |model: &mut Model, event| update(model, Msg::Agent(event));
        agent(
            &mut model,
            AgentEvent::Text(
                "The vocabulary is 48 static ids plus 20 indexed spaces — a native \
                 arrangement reuses all of them, so the hit registry answers the same \
                 questions.\n"
                    .to_owned(),
            ),
        );
        agent(
            &mut model,
            AgentEvent::ToolCall {
                id: "receipt-241".to_owned(),
                title: "edit cockpit/surface.rs".to_owned(),
                verb: ToolVerb::Edit,
                status: ToolStatus::InProgress,
                diff: Some(EditDiff {
                    path: "cockpit/surface.rs".to_owned(),
                    old_text: String::new(),
                    new_text: "pub enum Surface { Terminal, Native }\n".to_owned(),
                }),
                images: Vec::new(),
                record: Default::default(),
            },
        );
        agent(
            &mut model,
            AgentEvent::ToolCallUpdate {
                id: "receipt-241".to_owned(),
                title: None,
                status: Some(ToolStatus::Completed),
                diff: None,
                duration_ms: Some(180),
                images: Vec::new(),
                record: Default::default(),
            },
        );
        // The fold minted receipt #241 inside cp-4's reach off the real
        // counter; the capture's `+47` belongs to the larger edit the
        // volume illustrates, so the stat is seeded as data the way the
        // rail's charter rows are.
        model.turn_mut().set_receipt(
            "receipt-241",
            magistr_cockpit::turn::Receipt {
                added: 47,
                removed: 0,
                ordinal: 241,
                reach: Some("cp-4".to_owned()),
                killed: false,
            },
        );
        agent(
            &mut model,
            AgentEvent::Text(
                "Wiring it through Model::surface so the one view arranges the same ids \
                 for a window\n"
                    .to_owned(),
            ),
        );
        agent(
            &mut model,
            // The one metering message: the context (the foot's
            // `38,204 tok · 162k left`), the session's price (the meter's
            // fill and the band's `1.84 USD` — the capture's own spend)
            // and its clock (the agent block's `12 min`).
            AgentEvent::Usage {
                used: 38_204,
                size: 200_000,
                cost_cents: Some(184),
                elapsed_secs: Some(12 * 60),
            },
        );
        agent(&mut model, AgentEvent::TurnEnded(TurnEnd::EndTurn));
        settle(model)
    }

    /// `model` told it has a pixel layer.
    ///
    /// The cockpit's one switch for "this surface can show pixels"
    /// ([`Model::has_pixels`](magistr_cockpit::cockpit::Model::has_pixels)) is the
    /// startup probe's answer, and the probe's vocabulary is terminal protocols. A
    /// native window is not kitty and does not pretend to be one on the wire — but it
    /// *does* answer the only question the view asks, which is whether to stand the
    /// cell drawing down for a surface. Declaring it through the existing message is
    /// what keeps the view free of a second predicate.
    #[must_use]
    pub fn with_pixels(mut model: Model) -> Model {
        update(
            &mut model,
            Msg::Mode(Mode::Graphics {
                pointer_shapes: true,
            }),
        );
        model
    }
}

/// A golden `name` of `kind`, `size` pixels, drawn as a two-tone
/// diagonal wash — deterministic, and unmistakably a picture at
/// thumbnail height — decoded the way the shelf decodes a real one.
fn still(name: &str, kind: StillKind, size: (u32, u32)) -> ShelfStill {
    let (width, height) = size;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("a pixmap allocates");
    let pixels = pixmap.pixels_mut();
    for y in 0..height {
        for x in 0..width {
            let lit = (x + y) % 16 < 8;
            let (r, g, b) = if lit {
                (0x24, 0x27, 0x3a)
            } else {
                (0xca, 0x9e, 0xe6)
            };
            pixels[(y * width + x) as usize] =
                tiny_skia::ColorU8::from_rgba(r, g, b, 255).premultiply();
        }
    }
    let bytes = pixmap.encode_png().expect("png encodes");
    let image = StreamImage::decode(&bytes, name, CellSize::DEFAULT).expect("the picture decodes");
    ShelfStill {
        name: name.to_owned(),
        source: std::path::PathBuf::from("tests/golden")
            .join(kind.dir_name())
            .join(name),
        kind,
        image,
    }
}

/// The volume's own view-07 diff, as the workbench journals it.
const INK_DIFF: &str = "@@ -12,4 +12,6 @@\n \
    /// What a stylesheet should paint a slot with.\n\
-pub fn ink(field: PaletteField) -> Color {\n\
+pub fn ink(field: PaletteField, palette: &Palette) -> Color {\n\
+    let floor = field.declared_floor();\n\
+    ensure_contrast(palette.color(field), palette.base(), floor)\n\
 }";

/// One journaled edit, as the review's file column lists it.
fn journal(id: &str, path: &str, added: u32, diff: &str) -> JournalEntry {
    JournalEntry {
        at_ms: None,
        id: id.to_owned(),
        path: path.to_owned(),
        added,
        removed: 0,
        parse_ok: true,
        diff: diff.to_owned(),
    }
}

/// The roadmap the fixtures share: four chunks, two done, one in
/// progress — enough that the rail has a gauge to draw.
fn seed(model: &mut Model) {
    seed_chunk(model, None);
}

/// Seed the Sky views with seven completed chunks converging on one active chunk. The
/// awake variant adds a ready chunk and a stuck chunk. Summary counts remain `211 /
/// 306` to match the reference capture.
fn sky_seed(model: &mut Model, awake: bool) {
    let done = |id: &str, deps: Vec<&str>| {
        serde_json::json!({
            "id": id, "name": id, "status": "done", "priority": 200,
            "group": "core", "deps": deps,
        })
    };
    let mut chunks = vec![
        done("cockpit", vec![]),
        done("layout", vec!["cockpit"]),
        done("theme", vec!["cockpit"]),
        done("painter", vec!["layout"]),
        done("faces", vec!["theme"]),
        done("window", vec!["painter", "faces"]),
        done("motion", vec!["window"]),
        serde_json::json!({
            "id": "gui2-2a", "name": "gui2-2a design", "status": "in_progress",
            "priority": 159, "group": "core", "deps": ["motion"],
        }),
    ];
    if awake {
        chunks.push(serde_json::json!({
            "id": "gui2-2b", "name": "gui2-2b", "status": "pending",
            "priority": 160, "group": "core", "deps": ["motion"],
        }));
        chunks.push(serde_json::json!({
            "id": "e6b", "name": "e6b", "status": "stuck",
            "priority": 300, "group": "ops", "deps": ["layout"],
        }));
    }
    update(
        model,
        Msg::Engine(EngineEvent::Seed(NativeSnapshot {
            progress: fixtures::progress(211, 306),
            chunks: fixtures::chunks_from(chunks),
            signals: fixtures::signals_from(Vec::new()),
            ..NativeSnapshot::default()
        })),
    );
}

/// Seed the completion view with five completed prerequisites, one active chunk, and
/// one dependent chunk. Completion unlocks exactly one chunk and advances the summary
/// count from `211 / 306` to `212 / 306`.
fn ship_seed(model: &mut Model, shipped: bool) {
    let done = |id: &str, deps: Vec<&str>| {
        serde_json::json!({
            "id": id, "name": id, "status": "done", "priority": 200,
            "group": "desktop-gui", "deps": deps,
        })
    };
    let chunks = vec![
        done("cockpit", vec![]),
        done("layout", vec!["cockpit"]),
        done("theme", vec!["cockpit"]),
        done("painter", vec!["layout"]),
        done("motion", vec!["painter", "theme"]),
        // The claim, the prose and the ship stamp are view 09's: the
        // chapter a stakeholder reads once the star is opened.
        serde_json::json!({
            "id": "gui2-2a", "name": "gui2-2a design",
            "title": "The design language, decided live",
            "description": "One view, two arrangements. A surface kind on the model \
                            decides where the run's state sits \u{2014} the header's second \
                            row on cells, a status bar in a window \u{2014} with the same \
                            widget ids either way, so the hit registry and every golden \
                            held still.",
            "status": if shipped { "done" } else { "in_progress" },
            "priority": 159, "group": "desktop-gui", "deps": ["motion"],
            "updated_at": "2026-08-30T14:02:00Z",
        }),
        serde_json::json!({
            "id": "gui2-2b", "name": "gui2-2b layout", "status": "pending",
            "priority": 160, "group": "desktop-gui", "deps": ["gui2-2a"],
        }),
    ];
    update(
        model,
        Msg::Engine(EngineEvent::Seed(NativeSnapshot {
            progress: fixtures::progress(if shipped { 212 } else { 211 }, 306),
            chunks: fixtures::chunks_from(chunks),
            signals: fixtures::signals_from(Vec::new()),
            ..NativeSnapshot::default()
        })),
    );
}

/// The three quality gates, run and green — the cascade the chrome
/// shows as `gates ■ ■ ■`. Driven by the loop's own events and real job
/// results, exactly as [`Fixture::review_07`] drives them.
fn green_gates(model: &mut Model) {
    update(
        model,
        Msg::Loop(LoopEvent::Started {
            iteration: 4,
            max: 12,
        }),
    );
    for (index, gate) in ["check", "test", "lint"].into_iter().enumerate() {
        let job = JobId::new(index as u64 + 80);
        update(
            model,
            Msg::Loop(LoopEvent::GateStarted {
                name: gate.to_owned(),
                job,
            }),
        );
        update(
            model,
            Msg::Job {
                id: job,
                output: JobOutput::Text("ok".to_owned()),
            },
        );
    }
}

/// [`seed`], with the chunk in progress named — what the native header's session crumb
/// shows.
fn seed_chunk(model: &mut Model, chunk: Option<&str>) {
    let mut chunks = vec![
        serde_json::json!({"id": "core", "name": "Core", "status": "done", "priority": 100}),
        serde_json::json!({"id": "rail", "name": "Rail", "status": "done", "priority": 200}),
        serde_json::json!({"id": "paint", "name": "Painter", "status": "in_progress", "priority": 200}),
        serde_json::json!({"id": "window", "name": "Window", "title": "The native window", "status": "pending", "priority": 300, "deps": ["paint", "rail"]}),
    ];
    // The chunk the session is on is a chunk the roadmap holds, with the title the
    // identity band's chip names it by.
    if let Some(id) = chunk {
        chunks.insert(
            0,
            serde_json::json!({
                "id": id, "name": id, "title": "The design language",
                "status": "in_progress", "priority": 150
            }),
        );
    }
    update(
        model,
        Msg::Engine(EngineEvent::Seed(NativeSnapshot {
            progress: fixtures::progress(2, 4),
            chunks: fixtures::chunks_from(chunks),
            signals: fixtures::signals_from(Vec::new()),
            ..NativeSnapshot::default()
        })),
    );
}

/// A base64 PNG of `width` × `height` red pixels, as an image block
/// carries it — real encoder output, so the fixture proves the same
/// decode path a live agent's screenshot takes.
fn png_base64(width: u32, height: u32) -> String {
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).expect("a fixture-sized pixmap allocates");
    pixmap.fill(tiny_skia::Color::from_rgba8(200, 40, 40, 255));
    let png = pixmap.encode_png().expect("a pixmap encodes");
    let mut out = Vec::new();
    magistr_cockpit::base64::encode(&png, &mut out);
    String::from_utf8(out).expect("base64 is ascii")
}

/// A base64 PNG of `width` × `height`: 8 px checker squares in two
/// blues inside a 2 px amber border.
fn checker_base64(width: u32, height: u32) -> String {
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).expect("a fixture-sized pixmap allocates");
    let pixels = pixmap.pixels_mut();
    for y in 0..height {
        for x in 0..width {
            let border = x < 2 || y < 2 || x >= width - 2 || y >= height - 2;
            let (r, g, b) = if border {
                (240, 160, 60)
            } else if ((x / 8) + (y / 8)) % 2 == 0 {
                (60, 90, 170)
            } else {
                (190, 205, 235)
            };
            pixels[(y * width + x) as usize] =
                tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, 255)
                    .expect("opaque colour is premultiplied");
        }
    }
    let png = pixmap.encode_png().expect("a pixmap encodes");
    let mut out = Vec::new();
    magistr_cockpit::base64::encode(&png, &mut out);
    String::from_utf8(out).expect("base64 is ascii")
}

/// A left click on the widget `id`, as the runtime reports one.
fn click(id: magistr_cockpit::widget_id::WidgetId) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
        target: Some(id),
    }
}

/// `model` advanced to its settled frame.
fn settle(mut model: Model) -> Model {
    for _ in 0..Fixture::SETTLE_STEPS {
        update(&mut model, Msg::Tick);
    }
    model
}

#[cfg(test)]
#[path = "fixture.tests.rs"]
mod tests;
