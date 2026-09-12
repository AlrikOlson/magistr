use magistr_engine::policy::RiskTier;
use std::fmt::Write as _;
use std::time::Duration;

use super::*;
use crate::agent::{AgentEvent, ImageBlock, ToolStatus, ToolVerb, TurnEnd};
use crate::cockpit::{
    Eased, HoverRole, Motion, Msg, ShipRun, Size, TurnState, WorkbenchEvent, update,
};
use crate::harness::Scenes;
use crate::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind, Reply};
use crate::layout::{LayoutTree, Resolved};
use crate::render::paint;
use crate::render::{Buffer, Color, Style, Underline};
use crate::theme::{Icons, Theme, ThemeMode};
use crate::turn::{Accent, RowKind};
use crate::widget_id::WidgetId;

fn model() -> Model {
    Model::new(Size::new(80, 24))
}

fn clock() -> FrameClock {
    FrameClock::new(0, Duration::ZERO)
}

fn layout(root: &Node, size: Size) -> Resolved {
    LayoutTree::new()
        .compute(root, size.cols, size.rows)
        .expect("layout succeeds")
}

/// Every id-carrying rect, named.
fn placement(resolved: &Resolved) -> String {
    let mut out = String::new();
    for placed in resolved.placed() {
        let Some(widget) = placed.id.and_then(Widget::from_id) else {
            continue;
        };
        let r = placed.rect;
        let _ = writeln!(
            out,
            "{:<8} depth={} x={} y={} w={} h={}",
            widget.label(),
            placed.depth,
            r.x,
            r.y,
            r.width,
            r.height
        );
    }
    out
}

fn texts(node: &Node, out: &mut Vec<String>) {
    if let Some(text) = node.text_metrics() {
        out.push(text.text().to_owned());
    }
    for child in node.child_nodes() {
        texts(child, out);
    }
}

fn count(node: &Node) -> usize {
    1 + node.child_nodes().iter().map(count).sum::<usize>()
}

/// Every styled node in the tree, depth first.
fn styles(node: &Node, out: &mut Vec<Style>) {
    if let Some(style) = node.cell_style() {
        out.push(style.clone());
    }
    for child in node.child_nodes() {
        styles(child, out);
    }
}

/// The screen as text: the model laid out and painted at its size, one
/// line per row, trailing blanks trimmed.
fn screen(model: &Model) -> String {
    let size = model.size();
    let root = view(model, &FrameClock::new(7, Duration::from_millis(1500)));
    let resolved = layout(&root, size);
    let mut buffer = Buffer::new(size.cols, size.rows);
    paint(&root, &resolved, &mut buffer);
    let mut out = String::new();
    for row in 0..size.rows {
        let mut line = String::new();
        for col in 0..size.cols {
            if let Some(cell) = buffer.get(col, row) {
                line.push_str(&cell.symbol);
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn agent(m: &mut Model, event: AgentEvent) {
    update(m, Msg::Agent(event));
}

fn tool_calls(size: Size) -> Model {
    let mut m = Model::new(size);
    agent(&mut m, AgentEvent::Text("Let me look around.".to_owned()));
    for (i, status) in [
        ToolStatus::Pending,
        ToolStatus::InProgress,
        ToolStatus::Completed,
        ToolStatus::Failed,
    ]
    .into_iter()
    .enumerate()
    {
        agent(
            &mut m,
            AgentEvent::ToolCall {
                id: format!("c{i}"),
                title: format!("tool call {i}: read crates/magistr-tui/src/cockpit/view.rs"),
                verb: ToolVerb::Read,
                status: ToolStatus::Pending,
                diff: None,
                images: Vec::new(),
                record: Default::default(),
            },
        );
        agent(
            &mut m,
            AgentEvent::ToolCallUpdate {
                id: format!("c{i}"),
                title: None,
                status: Some(status),
                diff: None,
                duration_ms: matches!(status, ToolStatus::Completed | ToolStatus::Failed)
                    .then_some(842),
                images: Vec::new(),
                record: Default::default(),
            },
        );
    }
    agent(&mut m, AgentEvent::Text("Found it.".to_owned()));
    m
}

fn awaiting(size: Size) -> Model {
    let mut m = Model::new(size);
    agent(&mut m, AgentEvent::Text("All done here.".to_owned()));
    agent(&mut m, AgentEvent::TurnEnded(TurnEnd::MaxTokens));
    m
}

/// Awaiting input with a prompt typed: the cursor sits inside the word
/// "cockpit", after a paste that carried a newline.
fn prompted(size: Size) -> Model {
    let mut m = awaiting(size);
    for c in "now wire the prompt line into the cockpit".chars() {
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    update(&mut m, Msg::Paste("\nand test it".to_owned()));
    for _ in 0..15 {
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Left)));
    }
    m
}

/// Awaiting input with `read @` typed and the selection moved down one:
/// the file popup is open above the prompt over the whole workspace.
fn completing(size: Size) -> Model {
    let mut m = Model::new(size).with_workspace_files(
        [
            "src/main.rs",
            "src/cockpit/prompt.rs",
            "src/cockpit/view.rs",
            "README.md",
        ]
        .map(str::to_owned)
        .to_vec(),
    );
    agent(&mut m, AgentEvent::Text("All done here.".to_owned()));
    agent(&mut m, AgentEvent::TurnEnded(TurnEnd::EndTurn));
    for c in "read @".chars() {
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Down)));
    m
}

fn laid_out(m: &Model) -> Vec<Widget> {
    let resolved = layout(&view(m, &clock()), m.size());
    let mut ids: Vec<Widget> = resolved
        .placed()
        .iter()
        .filter_map(|p| p.id.and_then(Widget::from_id))
        .collect();
    ids.sort_by_key(|w| w.id());
    ids
}

#[test]
fn every_widget_is_laid_out_once_and_the_dock_buttons_only_on_the_front_card() {
    // Checkpoint and TraceGraph sit on the Trace tab, the tech-tree and
    // the chunk buttons on the Roadmap tab, the launcher's buttons on
    // the Home tab, the rail's mini-DAG and gauge need a seeded roadmap,
    // the header's activity monitor needs a busy frame behind it, the
    // undock a ≥160-column screen and the project card's headline a
    // terminal that renders a scaled run — not the chrome.
    let trace_only = |w: &Widget| {
        matches!(
            w,
            Widget::Checkpoint
                | Widget::TraceGraph
                | Widget::RoadmapGraph
                | Widget::RoadmapStart
                | Widget::RoadmapComplete
                | Widget::LaunchNext
                | Widget::LaunchLoop
                | Widget::LaunchGates
                | Widget::LaunchRoadmap
                | Widget::CardProject
                | Widget::CardRoadmap
                | Widget::CardSessions
                | Widget::CardActions
                | Widget::CardHeadline
                | Widget::RailGraph
                | Widget::RailGauge
                | Widget::HeaderSpark
                | Widget::Undock
                // The Moment's own button, laid out only on a plan ask
                // that still has pictures to show.
                | Widget::ShowImages
                // The terminal dock's grip and screen, laid out only
                // while a running card's terminal is raised.
                | Widget::TermDockGrip
                | Widget::TermDock
        )
    };
    // The crumbs, the composer, the Brief's controls, the ⌘, surface's
    // verbs, the shelf's Export and the Report's verbs are the *native*
    // arrangement's chrome; a
    // terminal never lays them out.
    let native_only = |w: &Widget| {
        Widget::CRUMBS.contains(w)
            || Widget::BRIEF.contains(w)
            || matches!(
                w,
                Widget::Composer
                    | Widget::GalaxyField
                    | Widget::GalaxyNew
                    | Widget::SettingsClose
                    | Widget::SettingsOpenFile
                    | Widget::ShelfExport
                    | Widget::ShelfShare
                    | Widget::ReportCommit
                    | Widget::ReportGates
                    | Widget::ReportCost
                    | Widget::ReportTrace
                    // The meter's Raise is the window rail's verb;
                    // a terminal's meter is
                    // a reading.
                    | Widget::BudgetRaise
                    // The identity band's chunk chip;
                    // a terminal's cap
                    // is a string.
                    | Widget::SessionChunk
            )
            || Widget::FAULTS.contains(w)
            || Widget::BIRTH.contains(w)
            || Widget::LOOP_VERBS.contains(w)
    };
    let chrome: Vec<Widget> = Widget::ALL
        .into_iter()
        .filter(|w| {
            *w != Widget::Chunk
                && !trace_only(w)
                && !native_only(w)
                && !Widget::DOCK_BUTTONS.contains(w)
                && !Widget::LOOP.contains(w)
        })
        .collect();
    assert_eq!(laid_out(&model()), chrome, "no chunk started yet");
    let m = Scenes::permission(Size::new(80, 24));
    let with_card: Vec<Widget> = Widget::ALL
        .into_iter()
        .filter(|w| {
            *w != Widget::Chunk
                && !trace_only(w)
                && !native_only(w)
                && !matches!(
                    w,
                    Widget::Never | Widget::Accept | Widget::Reject | Widget::Clear
                )
                && !Widget::LOOP.contains(w)
        })
        .collect();
    assert_eq!(
        laid_out(&m),
        with_card,
        "Never is not offered; the other card types' buttons stay off"
    );
}

#[test]
fn a_started_chunk_reaches_the_phase_rail_and_a_collapse_drops_the_divider() {
    let mut m = model();
    update(
        &mut m,
        Msg::Engine(crate::engine::EngineEvent::local(
            crate::harness::fixtures::frame(
                "roadmap",
                "chunk_changed",
                serde_json::json!({"chunk": {"id": "tui2-2a1-rail", "status": "in_progress"}}),
            ),
        )),
    );
    let widgets = laid_out(&m);
    assert!(widgets.contains(&Widget::Chunk), "{widgets:?}");
    assert!(widgets.contains(&Widget::RailDivider));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(crate::input::Button::Left), 1, 3),
            target: Some(Widget::RailToggle.id()),
        },
    );
    let collapsed = laid_out(&m);
    assert!(collapsed.contains(&Widget::Rail));
    assert!(collapsed.contains(&Widget::RailToggle));
    assert!(
        !collapsed.contains(&Widget::RailDivider),
        "no divider while collapsed: {collapsed:?}"
    );
}

#[test]
fn the_phase_rail_sits_on_row_one_in_order() {
    let resolved = layout(&view(&model(), &clock()), Size::new(80, 24));
    let phases: Vec<(Widget, u16, u16)> = resolved
        .placed()
        .iter()
        .filter_map(|p| {
            p.id.and_then(Widget::from_id)
                .map(|w| (w, p.rect.x, p.rect.y))
        })
        .filter(|(w, _, _)| Widget::PHASES.contains(w))
        .collect();
    assert_eq!(phases.len(), 4);
    assert!(phases.iter().all(|&(_, _, y)| y == 1));
    assert!(phases.windows(2).all(|w| w[0].1 < w[1].1), "{phases:?}");
}

#[test]
fn the_stage_rect_is_the_geometry_the_model_pages_and_wraps_by() {
    for size in [Size::new(80, 24), Size::new(120, 40), Size::new(20, 8)] {
        let m = Scenes::streaming(size);
        let resolved = layout(&view(&m, &clock()), size);
        let stage = resolved
            .placed()
            .iter()
            .find(|p| p.id == Some(Widget::Stage.id()))
            .expect("the stage is laid out")
            .rect;
        // The stage viewport carries no padding of its own: the panel's
        // border ring insets it, so the rect is the model's geometry
        // exactly.
        assert_eq!(usize::from(stage.height), m.stage_rows(), "{size:?}");
        // A screen too short for any stage row clips the viewport to an
        // empty rect, whose width says nothing.
        if m.stage_rows() > 0 {
            assert_eq!(stage.width, m.stage_width(), "{size:?}");
        }
        assert!(
            m.turn()
                .rows()
                .iter()
                .all(|r| crate::render::width(&r.text) <= m.stage_width())
        );
    }
}

#[test]
fn the_header_names_the_turn_state_and_the_diagnostics_hide_behind_debug() {
    let replies = |m: &mut Model| {
        update(m, Msg::Reply(Reply::Background(Color::Rgb(30, 30, 46))));
        update(m, Msg::Reply(Reply::KeyboardFlags(3)));
        update(
            m,
            Msg::Reply(Reply::CellPixels {
                width: 10,
                height: 20,
            }),
        );
    };
    let mut m = model();
    replies(&mut m);
    let mut lines = Vec::new();
    texts(
        &view(&m, &FrameClock::new(17, Duration::from_millis(3200))),
        &mut lines,
    );
    assert_eq!(lines[0], "▲ magistr", "the brand slot leads the bar");
    assert_eq!(lines[1], "idle");
    assert_eq!(lines[3], "00:03", "the session clock rides the right edge");
    assert_eq!(
        &lines[lines.len() - 6..],
        [" enter ", "send", " ↑↓ ", "history", " ctrl-d ", "quit"],
        "no terminal diagnostics without the toggle"
    );
    agent(&mut m, AgentEvent::Text("hi".to_owned()));
    let mut lines = Vec::new();
    texts(&view(&m, &clock()), &mut lines);
    assert_eq!(lines[1], "▘ streaming", "the spinner leads the state");
    assert_eq!(m.turn_state(), TurnState::Streaming);
    assert_eq!(
        &lines[lines.len() - 4..],
        [" ctrl-c ", "cancel", " ctrl-d ", "quit"]
    );

    let mut debug = model().with_debug(true);
    replies(&mut debug);
    let mut lines = Vec::new();
    texts(
        &view(&debug, &FrameClock::new(17, Duration::from_millis(3200))),
        &mut lines,
    );
    assert_eq!(lines[0], "▲ magistr");
    assert_eq!(lines[3], "80×24 · frame 17 · 00:03");
    assert_eq!(
        lines.last().map(String::as_str),
        Some("focus yes · theme dark · background #1e1e2e · keyboard kitty 3 · cell 10×20px")
    );
    assert_eq!(
        &lines[lines.len() - 7..lines.len() - 1],
        [" enter ", "send", " ↑↓ ", "history", " ctrl-d ", "quit"],
        "the chips keep the hints ahead of the diagnostics"
    );
}

/// The one animated mark on screen: the header's spinner turns a quarter
/// per [`Model::SPIN_PERIOD`] while the agent talks, comes back around after four,
/// and is simply absent when nothing streams — which is exactly when the
/// runtime stops producing ticks.
#[test]
fn the_header_spinner_turns_on_the_tick_and_only_while_streaming() {
    use crate::theme::Icons;

    let mut m = model();
    assert!(!m.is_animating(), "an idle cockpit animates nothing");
    let mut lines = Vec::new();
    texts(&view(&m, &clock()), &mut lines);
    assert_eq!(lines[1], "idle", "no spinner without a turn");

    agent(&mut m, AgentEvent::Text("hi".to_owned()));
    assert!(m.is_animating());
    let mut seen = Vec::new();
    for step in 0..5u64 {
        assert_eq!(m.ticks(), step * Model::SPIN_STEPS);
        let mut lines = Vec::new();
        texts(&view(&m, &clock()), &mut lines);
        seen.push(lines[1].clone());
        // A quarter turn is SPIN_STEPS ticks of the runtime's faster
        // motion tick.
        for _ in 0..Model::SPIN_STEPS {
            update(&mut m, Msg::Tick);
        }
    }
    assert_eq!(
        seen,
        Icons::SPINNER
            .iter()
            .chain(std::iter::once(&Icons::SPINNER[0]))
            .map(|frame| format!("{frame} streaming"))
            .collect::<Vec<_>>()
    );

    agent(&mut m, AgentEvent::TurnEnded(TurnEnd::EndTurn));
    assert!(!m.is_animating(), "the turn ended: the mark stops");
    let mut lines = Vec::new();
    texts(&view(&m, &clock()), &mut lines);
    assert_eq!(lines[1], "awaiting input");
}

#[test]
fn the_header_leads_with_the_session_title() {
    let m = model().with_title("magistr — magistr · acp · claude-sonnet-5");
    let mut lines = Vec::new();
    texts(&view(&m, &clock()), &mut lines);
    assert_eq!(lines[0], "▲ magistr", "the brand slot stays first");
    assert_eq!(lines[1], "magistr — magistr · acp · claude-sonnet-5");
    assert_eq!(lines[2], "idle");
}

#[test]
fn the_prompt_line_shows_the_mark_the_text_around_the_cursor_and_the_cursor_cell() {
    let m = prompted(Size::new(80, 24));
    assert_eq!(m.prompt_rows(), 2, "the pasted newline grew the box");
    let root = view(&m, &clock());
    let mut lines = Vec::new();
    texts(&root, &mut lines);
    let footer = lines.len() - 6;
    assert_eq!(
        &lines[footer - 6..footer],
        [
            "› ",
            "now wire the prompt line into the cock",
            "p",
            "it",
            "  ",
            "and test it"
        ],
        "two rows: the cursor's line and the pasted continuation"
    );
    let mut found = Vec::new();
    styles(&root, &mut found);
    let pen = Pen::of(&m.theme().palette());
    assert!(found.contains(&pen.prompt_mark));
    assert!(found.contains(&pen.cursor));
    // At the end of the line the cursor is a blank cell that survives
    // the wrapper's trimming.
    let mut at_end = prompted(Size::new(80, 24));
    update(&mut at_end, Msg::Key(KeyEvent::new(KeyCode::End)));
    let mut lines = Vec::new();
    texts(&view(&at_end, &clock()), &mut lines);
    let footer = lines.len() - 6;
    assert_eq!(
        &lines[footer - 5..footer],
        [
            "› ",
            "now wire the prompt line into the cockpit",
            "\u{a0}",
            "  ",
            "and test it"
        ],
        "end stops at the cursor's line, not the text's"
    );
    // A narrow terminal scrolls the cursor's line so the cursor stays
    // on screen; the other lines clip. Twenty columns fit one footer
    // chip whole, so the bar behind the prompt is two texts, not six.
    update(&mut at_end, Msg::Resize(Size::new(20, 10)));
    let mut lines = Vec::new();
    texts(&view(&at_end, &clock()), &mut lines);
    let footer = lines.len() - 2;
    assert_eq!(
        &lines[footer - 5..footer],
        ["› ", " into the cockpit", "\u{a0}", "  ", "and test it"],
        "17 columns after the mark, the cursor's own included"
    );
    let screen = screen(&at_end);
    assert!(screen.contains("›  into the cockpit"), "{screen}");
    assert!(screen.contains("  and test it"), "{screen}");
}

#[test]
fn the_permission_strip_names_the_tool_and_offers_its_buttons_in_their_pens() {
    let m = Scenes::permission(Size::new(80, 24));
    assert_eq!(m.stage_rows(), 14, "one card inside the needs-you panel");
    let root = view(&m, &clock());
    let mut lines = Vec::new();
    texts(&root, &mut lines);
    assert!(
        lines.contains(&"⚠ Bash needs permission".to_owned()),
        "{lines:?}"
    );
    assert!(lines.contains(&"[A] Allow".to_owned()));
    assert!(lines.contains(&"[\u{21e7}A] Always".to_owned()));
    assert!(lines.contains(&"[D] Deny".to_owned()));
    assert!(!lines.iter().any(|l| l.contains("Never")));
    assert_eq!(
        &lines[lines.len() - 6..],
        [" A ", "allow", " \u{21e7}A ", "always", " D ", "deny"],
        "{lines:?}"
    );
    let mut found = Vec::new();
    styles(&root, &mut found);
    let pen = Pen::of(&m.theme().palette());
    for style in [&pen.permission, &pen.allow, &pen.always, &pen.deny] {
        assert!(found.contains(style));
    }
    let resolved = layout(&root, m.size());
    let buttons: Vec<(Widget, u16, u16)> = resolved
        .placed()
        .iter()
        .filter_map(|p| {
            p.id.and_then(Widget::from_id)
                .map(|w| (w, p.rect.x, p.rect.y))
        })
        .filter(|(w, _, _)| w.answers().is_some())
        .collect();
    assert_eq!(buttons.len(), 3);
    assert!(buttons.iter().all(|&(_, _, y)| y == 20), "{buttons:?}");
    assert!(buttons.windows(2).all(|w| w[0].1 < w[1].1), "{buttons:?}");
    let screen = screen(&m);
    assert!(
        screen.contains("⚠ Bash needs permission [A] Allow [\u{21e7}A] Always [D] Deny"),
        "{screen}"
    );
}

#[test]
fn an_empty_stage_says_so_and_unknown_replies_read_as_such_in_debug() {
    let mut lines = Vec::new();
    texts(&view(&model(), &clock()), &mut lines);
    assert!(lines.contains(&"nothing yet — waiting for the agent".to_owned()));
    assert_eq!(
        &lines[lines.len() - 6..],
        [" enter ", "send", " ↑↓ ", "history", " ctrl-d ", "quit"]
    );
    let mut lines = Vec::new();
    texts(&view(&model().with_debug(true), &clock()), &mut lines);
    assert_eq!(
        lines.last().map(String::as_str),
        Some("focus yes · theme dark · background unknown · keyboard legacy")
    );
}

#[test]
fn the_stage_shows_the_turn_rows_in_their_pens() {
    // Settled: the rows have finished arriving, so every pen is the pen
    // the row keeps. A whole shimmer period puts the gutter
    // back where it started too, so this stays an assertion about pens
    // rather than about which step the wave is on.
    let mut m = tool_calls(Size::new(80, 24));
    Scenes::ticks(&mut m, SHIMMER_STEPS);
    const {
        assert!(
            SHIMMER_STEPS >= Motion::ARRIVAL_STEPS,
            "the arrival is over"
        )
    };
    let root = view(&m, &clock());
    let pen = Pen::of(&m.theme().palette());
    let mut found = Vec::new();
    styles(&root, &mut found);
    assert_eq!(
        &found[..24],
        &[
            pen.base.clone(),
            pen.footer.clone(),
            pen.header.clone(),
            pen.always.clone(),
            pen.footer.clone(),
            pen.footer.clone(),
            pen.phase_other.clone(),
            pen.footer.clone(),
            pen.phase_other.clone(),
            pen.footer.clone(),
            pen.phase_other.clone(),
            pen.footer.clone(),
            pen.phase_other.clone(),
            pen.stage.clone(),
            pen.note.clone(),
            pen.rail.clone(),
            pen.note.clone(),
            pen.rail.clone(),
            pen.tab_inactive.clone(),
            pen.tab_active.clone(),
            pen.tab_inactive.clone(),
            pen.tab_inactive.clone(),
            pen.tab_inactive.clone(),
            pen.stage.clone()
        ],
        "root, the header bar (brand, the streaming state, the clock), the \
         strip and its four phases (none current) between their separators, \
         the rail panel (its surface, its toggle in the note pen — the plan \
         title lives in the frame now — progress and signals), the unstyled \
         divider, the tab bar with Turn bright among the five tabs, stage, \
         then the rows"
    );
    let top = m.top_row();
    let rows: Vec<Style> = m
        .visible_rows()
        .iter()
        .enumerate()
        .flat_map(|(i, r)| {
            let accent = m.turn().accent_at_row(top + i);
            [
                pen.row(r.kind).clone(),
                pen.accent(accent).clone(),
                pen.row(r.kind).clone(),
            ]
        })
        .collect();
    assert_eq!(
        &found[24..24 + rows.len()],
        &rows[..],
        "each row is its band, its accent gutter, then its text"
    );
    assert_eq!(found.last(), Some(&pen.footer));
    assert!(rows.contains(&pen.tool_failed));
    assert!(rows.contains(&pen.tool_done));
    assert!(rows.contains(&pen.tool_running));
    assert!(rows.contains(&pen.tool_pending));
}

#[test]
fn a_light_theme_changes_every_pen_and_names_itself() {
    let dark = model();
    let root = view(&dark, &clock());
    let light = model()
        .with_theme(Theme::forced(ThemeMode::Light))
        .with_debug(true);
    let light_root = view(&light, &clock());
    assert_eq!(
        light_root.cell_style(),
        Some(&Pen::of(&ThemeMode::Light.palette()).base)
    );
    assert_ne!(light_root.cell_style(), root.cell_style());
    let mut lines = Vec::new();
    texts(&light_root, &mut lines);
    assert!(
        lines
            .last()
            .is_some_and(|l| l.contains("theme light (MAGISTR_THEME)")),
        "{lines:?}"
    );
}

#[test]
fn the_stage_costs_the_screen_not_the_turn() {
    let mut m = model();
    for i in 0..10_000 {
        agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
    }
    assert_eq!(m.turn().len(), 10_000);
    let root = view(&m, &clock());
    let nodes = count(&root);
    assert!(
        nodes <= 3 * m.stage_rows() + 45,
        "{nodes} nodes for a 10K-row turn on a {}-row stage: three per row (the band, its accent gutter, its text) plus the chrome (root, the header bar's five nodes, the strip with its phase block's eight, the left rail panel and its five parts, the divider, the tab bar and its five tabs, stage, the dock's divider row and its three segments, the prompt box and its row's three parts, the footer bar and its chips)",
        m.stage_rows()
    );
    let mut lines = Vec::new();
    texts(&root, &mut lines);
    assert!(lines.contains(&"line 9999".to_owned()), "follows the tail");
    assert!(!lines.contains(&"line 0".to_owned()));
}

#[test]
fn a_scrolled_window_shows_its_row_range_in_the_header() {
    let mut m = model();
    for i in 0..100 {
        agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
    }
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::PageUp)));
    Scenes::settle(&mut m);
    let mut lines = Vec::new();
    texts(&view(&m, &clock()), &mut lines);
    assert_eq!(lines[1], "▘ streaming");
    assert_eq!(lines[2], "rows 70–85/100", "{lines:?}");
    assert!(lines.contains(&"line 69".to_owned()));
    assert!(!lines.contains(&"line 99".to_owned()));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::ScrollDown, 10, 10),
            target: Some(Widget::Stage.id()),
        },
    );
    Scenes::settle(&mut m);
    let mut lines = Vec::new();
    texts(&view(&m, &clock()), &mut lines);
    assert_eq!(lines[2], "rows 73–88/100", "{lines:?}");
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::End)));
    Scenes::settle(&mut m);
    let mut lines = Vec::new();
    texts(&view(&m, &clock()), &mut lines);
    assert!(
        !lines.iter().any(|l| l.starts_with("rows ")),
        "back at the tail the range disappears: {lines:?}"
    );
    assert!(lines.contains(&"line 99".to_owned()));
}

#[test]
fn snap_placeholder_cockpit_80x24() {
    let resolved = layout(&view(&model(), &clock()), Size::new(80, 24));
    insta::assert_snapshot!(placement(&resolved));
}

#[test]
fn snap_streaming_turn_80x24() {
    // Settled, like the PNG stills: a snapshot pins the *surface*, and
    // an unsettled streaming model is caught at the foot of the activity
    // monitor's arrival sweep with its slot still blank. The transients
    // are reviewed on the filmstrips instead.
    let mut m = Scenes::streaming(Size::new(80, 24));
    Scenes::settle(&mut m);
    assert_eq!(m.turn_state(), TurnState::Streaming);
    assert!(m.turn().rows().iter().any(|r| r.kind == RowKind::Code));
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_streaming_turn_120x40() {
    let mut m = Scenes::streaming(Size::new(120, 40));
    Scenes::settle(&mut m);
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_tool_calls_80x24() {
    insta::assert_snapshot!(screen(&tool_calls(Size::new(80, 24))));
}

#[test]
fn snap_tool_calls_120x40() {
    insta::assert_snapshot!(screen(&tool_calls(Size::new(120, 40))));
}

/// A turn punctuated by the engine feed: task and think events around a
/// passed check and a failed one whose details wait behind the fold.
fn gated(size: Size) -> Model {
    use crate::engine::EngineEvent;
    fn feed(m: &mut Model, family: &str, kind: &str, payload: serde_json::Value) {
        update(
            m,
            Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
                family, kind, payload,
            ))),
        );
    }
    let mut m = Model::new(size);
    agent(&mut m, AgentEvent::Text("Running the gates.".to_owned()));
    feed(
        &mut m,
        "ship",
        "task_started",
        serde_json::json!({"task": {"id": "verify", "title": "run the gates"}}),
    );
    feed(
        &mut m,
        "think",
        "step_appended",
        serde_json::json!({"step": {"step_number": 7, "purpose": "verify the slice"}}),
    );
    feed(
        &mut m,
        "ship",
        "check_recorded",
        serde_json::json!({"check": {
            "name": "cargo check", "passed": true, "exit_code": 0, "duration_ms": 812
        }}),
    );
    feed(
        &mut m,
        "ship",
        "check_recorded",
        serde_json::json!({"check": {
            "name": "cargo nextest run", "passed": false, "exit_code": 100,
            "duration_ms": 12345,
            "details": "FAIL turn::stream::a_check_is_one_row\nFAIL cockpit::update::a_click_expands\n2 tests failed"
        }}),
    );
    agent(&mut m, AgentEvent::Text("Two failures to fix.".to_owned()));
    m
}

#[test]
fn snap_check_cards_80x24() {
    let m = gated(Size::new(80, 24));
    assert!(
        m.turn()
            .rows()
            .iter()
            .any(|r| r.kind == RowKind::Check(true)),
        "a passed check row renders"
    );
    assert!(
        m.turn()
            .rows()
            .iter()
            .any(|r| r.kind == RowKind::Check(false)),
        "a failed check row renders"
    );
    assert!(m.turn().rows().iter().any(|r| r.kind == RowKind::Event));
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_check_cards_120x40() {
    insta::assert_snapshot!(screen(&gated(Size::new(120, 40))));
}

#[test]
fn snap_check_card_expanded_80x24() {
    let mut m = gated(Size::new(80, 24));
    let collapsed = m.turn().len();
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(
        m.turn().len() > collapsed,
        "Enter on an empty prompt expands the newest card"
    );
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_check_card_expanded_120x40() {
    let mut m = gated(Size::new(120, 40));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_awaiting_input_80x24() {
    let m = awaiting(Size::new(80, 24));
    assert_eq!(m.turn_state(), TurnState::AwaitingInput);
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_awaiting_input_120x40() {
    insta::assert_snapshot!(screen(&awaiting(Size::new(120, 40))));
}

#[test]
fn snap_prompt_line_80x24() {
    let m = prompted(Size::new(80, 24));
    assert_eq!(m.turn_state(), TurnState::AwaitingInput);
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_prompt_line_120x40() {
    insta::assert_snapshot!(screen(&prompted(Size::new(120, 40))));
}

/// A plan approval request, with a checkpoint, three steps,
/// a picture and a named scope.
fn plan_ask(size: Size) -> Model {
    plan_ask_on(Scenes::streaming(size))
}

/// The same ask, raised on a session the caller already built — so a
/// test can ask what a Moment does to a foot that has a phase on it.
fn plan_ask_on(mut m: Model) -> Model {
    use crate::agent::{AskPlan, PermissionChoice, PermissionKind};
    agent(
        &mut m,
        AgentEvent::PermissionRequest {
            id: 1,
            title: "Re-bless 12 golden screenshots".to_owned(),
            choices: vec![
                PermissionChoice::new("allow", PermissionKind::AllowOnce),
                PermissionChoice::new("allow_always", PermissionKind::AllowAlways),
                PermissionChoice::new("reject", PermissionKind::RejectOnce),
            ],
            edit: false,
            risk: RiskTier::High,
            plan: Some(
                AskPlan::new([
                    "Render the 6 surfaces in both themes",
                    "Compare with goldens. Only status-bar rows may change",
                    "Update matching goldens. Reject all others",
                ])
                .scoped("goldens"),
            ),
        },
    );
    m
}

#[test]
fn snap_moment_plan_card_80x24() {
    let m = plan_ask(Size::new(80, 24));
    assert!(m.moment().is_some(), "a plan ask raises the Moment");
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_moment_plan_card_120x40() {
    insta::assert_snapshot!(screen(&plan_ask(Size::new(120, 40))));
}

#[test]
fn the_moment_shows_the_plan_the_checkpoint_and_the_scope() {
    let m = plan_ask(Size::new(120, 40));
    let text = screen(&m);
    assert!(text.contains("PLAN · APPROVAL REQUIRED"), "{text}");
    assert!(text.contains("Checkpoint cp-1 saved"), "{text}");
    assert!(
        text.contains("1. Render the 6 surfaces in both themes"),
        "{text}"
    );
    assert!(text.contains("3. Update matching goldens"), "{text}");
    assert!(text.contains("⇧A · ALWAYS ALLOW (GOLDENS)"), "{text}");
    assert!(text.contains("Add a condition."), "{text}");
}

#[test]
fn a_moment_with_no_checkpoint_names_none_rather_than_a_placeholder() {
    use crate::agent::{AskPlan, PermissionChoice, PermissionKind};
    let mut m = Scenes::streaming(Size::new(120, 40));
    agent(
        &mut m,
        AgentEvent::PermissionRequest {
            id: 1,
            title: "Re-bless the goldens".to_owned(),
            choices: vec![PermissionChoice::new("allow", PermissionKind::AllowOnce)],
            edit: false,
            // Not High: the spine mints nothing, so the card has no
            // checkpoint to name.
            risk: RiskTier::Medium,
            plan: Some(AskPlan::new(["Render", "Compare"])),
        },
    );
    let text = screen(&m);
    assert!(text.contains("PLAN · APPROVAL REQUIRED"), "{text}");
    assert!(!text.contains("Checkpoint"), "{text}");
    assert!(
        !text.contains("ALWAYS ALLOW"),
        "no always choice was offered: {text}"
    );
}

#[test]
fn show_images_is_offered_only_while_the_ask_still_has_pictures() {
    use crate::agent::{AskPlan, ImageBlock, PermissionChoice, PermissionKind};
    let mut m = Scenes::streaming(Size::new(120, 40));
    agent(
        &mut m,
        AgentEvent::PermissionRequest {
            id: 1,
            title: "Re-bless the goldens".to_owned(),
            choices: vec![PermissionChoice::new("allow", PermissionKind::AllowOnce)],
            edit: false,
            risk: RiskTier::Medium,
            // The bytes need not decode: what is under test is that
            // the button is offered while the card still holds a block
            // and gone once it has handed it over. An undecodable one
            // lands as the note row `picture` already writes for it.
            plan: Some(AskPlan::new(["Render"]).with_images(vec![ImageBlock {
                mime_type: "image/png".to_owned(),
                data: "not-a-png".to_owned(),
                uri: None,
            }])),
        },
    );
    assert!(laid_out(&m).contains(&Widget::ShowImages));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char(' '))));
    assert!(
        !laid_out(&m).contains(&Widget::ShowImages),
        "the button leaves with the pictures"
    );
}

#[test]
fn the_session_dims_behind_the_moment_and_never_blanks() {
    let after = screen(&plan_ask(Size::new(120, 40)));
    assert!(
        after.contains("Re-bless 12 golden screenshots"),
        "the card is on screen: {after}"
    );
    // Whatever the stage was showing is still there — the dim moves
    // foregrounds and never grounds, so the session behind the decision
    // is quieter and not gone.
    assert!(after.contains("implement the turn stage"), "{after}");
    assert!(
        after.contains("I should read the roadmap chunk first"),
        "{after}"
    );
}

#[test]
fn snap_permission_strip_80x24() {
    let m = Scenes::permission(Size::new(80, 24));
    assert_eq!(m.permissions_pending(), 1);
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_permission_strip_120x40() {
    insta::assert_snapshot!(screen(&Scenes::permission(Size::new(120, 40))));
}

/// The plain-text cockpit: MinimalText decided at startup,
/// the banner on its own row under the status strip, a plan task on the
/// rail, agent text on the stage and the permission card in the dock —
/// the pixel regions simply absent.
fn minimal_text(size: Size) -> Model {
    use crate::capability::{Mode, TextReason};
    let mut m = Scenes::permission(size);
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
            "task_added",
            serde_json::json!({"task_id": "prove", "title": "snapshot both sizes"}),
        ),
        (
            "task_started",
            serde_json::json!({"task": {"id": "impl", "title": "wire the banner"}}),
        ),
    ] {
        update(
            &mut m,
            Msg::Engine(crate::engine::EngineEvent::local(
                crate::harness::fixtures::frame("ship", kind, payload),
            )),
        );
    }
    update(
        &mut m,
        Msg::Mode(Mode::MinimalText(TextReason::TmuxNoPassthrough)),
    );
    m
}

#[test]
fn the_banner_renders_only_in_minimal_text() {
    let with = screen(&minimal_text(Size::new(80, 24)));
    assert!(with.contains("Plain-text mode"), "{with}");
    let without = screen(&Scenes::permission(Size::new(80, 24)));
    assert!(
        !without.contains("Plain-text mode"),
        "no banner before a mode arrives: {without}"
    );
}

#[test]
fn snap_minimal_text_80x24() {
    let s = screen(&minimal_text(Size::new(80, 24)));
    assert!(s.contains("Plain-text mode"), "{s}");
    insta::assert_snapshot!(s);
}

#[test]
fn snap_minimal_text_120x40() {
    insta::assert_snapshot!(screen(&minimal_text(Size::new(120, 40))));
}

#[test]
fn the_status_strip_shows_the_iteration_the_phases_the_gates_and_the_clock() {
    use crate::cockpit::{LoopEvent, Phase};
    let m = Scenes::looping(Size::new(80, 24));
    assert_eq!(m.phase(), Some(Phase::Execute));
    let root = view(&m, &clock());
    let mut lines = Vec::new();
    texts(&root, &mut lines);
    assert_eq!(
        &lines[4..17],
        [
            "a",
            "iteration 2/5",
            "Research",
            "▸",
            "Plan",
            "▸",
            "Execute",
            "▸",
            "Verify",
            "✓ check",
            "✗ test",
            // The gate still running spins: the spinner's
            // first frame at step 0.
            "▘ lint",
            "1m 23s"
        ],
        "the started chunk leads the strip: {lines:?}"
    );
    assert_eq!(
        &lines[lines.len() - 6..],
        [
            " space ",
            "pause / resume",
            " ctrl-c ",
            "cancel",
            " ctrl-d ",
            "quit"
        ],
        "{lines:?}"
    );
    let pen = Pen::of(&m.theme().palette());
    let mut found = Vec::new();
    styles(&root, &mut found);
    assert_eq!(
        &found[2..30],
        &[
            pen.header.clone(),
            pen.always.clone(),
            pen.footer.clone(),
            pen.footer.clone(),
            pen.permission.clone(),
            pen.phase_other.clone(),
            pen.footer.clone(),
            pen.phase_other.clone(),
            pen.footer.clone(),
            pen.phase_current.clone(),
            pen.footer.clone(),
            pen.phase_other.clone(),
            pen.gate_pass.clone(),
            pen.gate_fail.clone(),
            pen.gate_running.clone(),
            pen.stage.clone(),
            pen.note.clone(),
            pen.rail.clone(),
            // The mini-DAG block's two summary lines, then the signals.
            pen.note.clone(),
            pen.note.clone(),
            pen.note.clone(),
            pen.rail.clone(),
            pen.tab_inactive.clone(),
            pen.tab_active.clone(),
            pen.tab_inactive.clone(),
            pen.tab_inactive.clone(),
            pen.tab_inactive.clone(),
            pen.stage.clone(),
        ],
        "the brand then the streaming state lead the bar; execute is the current \
         phase; the LEDs wear their outcome; the tab bar shows Turn"
    );
    let resolved = layout(&root, m.size());
    let on_row_one: Vec<Widget> = resolved
        .placed()
        .iter()
        .filter(|p| p.rect.y == 1 && p.rect.height == 1)
        .filter_map(|p| p.id.and_then(Widget::from_id))
        .collect();
    assert_eq!(
        on_row_one,
        [
            Widget::Status,
            Widget::Chunk,
            Widget::Iteration,
            Widget::Research,
            Widget::Plan,
            Widget::Execute,
            Widget::Verify,
            Widget::Gates,
            Widget::Elapsed
        ]
    );
    let screen = screen(&m);
    assert!(
        screen.contains(
            "a  iteration 2/5  Research▸Plan▸Execute▸Verify  ✓ check ✗ test ▘ lint  1m 23s"
        ),
        "{screen}"
    );
    assert!(
        screen.contains("starting iteration 2 — 3/10 chunks done"),
        "{screen}"
    );
    // Paused and ended read on the strip and in the footer.
    let mut paused = Scenes::looping(Size::new(80, 24));
    update(&mut paused, Msg::Key(KeyEvent::new(KeyCode::Char(' '))));
    let mut lines = Vec::new();
    texts(&view(&paused, &clock()), &mut lines);
    assert_eq!(lines[5], "paused 2/5", "after the chunk's name");
    assert_eq!(
        &lines[lines.len() - 10..],
        [
            " space ",
            "pause / resume",
            " s ",
            "step",
            " x ",
            "skip",
            " esc ",
            "abort",
            " ctrl-d ",
            "quit"
        ],
        "{lines:?}"
    );
    let mut ended = Scenes::looping(Size::new(80, 24));
    update(
        &mut ended,
        Msg::Loop(LoopEvent::Ended("max iterations (5) reached".to_owned())),
    );
    let mut lines = Vec::new();
    texts(&view(&ended, &clock()), &mut lines);
    assert_eq!(lines[5], "ended 2/5", "after the chunk's name");
    assert_eq!(
        &lines[lines.len() - 4..],
        [" ctrl-c ", "cancel", " ctrl-d ", "quit"],
        "{lines:?}"
    );
    // Without a loop the strip is the four phases and nothing else; the
    // rail panel's four lines sit between the strip and the empty stage
    // (its plan title lives in the frame, so the first line is the
    // toggle alone). The header bar's own texts (brand, state, spacer,
    // clock) sit ahead of them.
    let mut lines = Vec::new();
    texts(&view(&model(), &clock()), &mut lines);
    assert_eq!(
        &lines[4..11],
        ["Research", "▸", "Plan", "▸", "Execute", "▸", "Verify"]
    );
    assert_eq!(
        &lines[11..15],
        ["◂", "no objective yet", "roadmap 0/0 done", "signals 0"]
    );
    assert_eq!(
        &lines[15..20],
        ["Home", "Turn", "Changes", "Trace", "Roadmap"],
        "the tab bar sits between the rail and the stage"
    );
    assert!(lines[20].starts_with("nothing yet"));
}

#[test]
fn snap_loop_status_80x24() {
    insta::assert_snapshot!(screen(&Scenes::looping(Size::new(80, 24))));
}

#[test]
fn snap_loop_status_120x40() {
    insta::assert_snapshot!(screen(&Scenes::looping(Size::new(120, 40))));
}

/// A turn with every inline construct and a fenced Rust block.
fn markdown(size: Size) -> Model {
    let mut m = Model::new(size);
    agent(
        &mut m,
        AgentEvent::Text(
            "Some **bold**, _emphasis_, `inline code` and [a link](https://example.com).\n\
             ```rust\nfn main() {\n    println!(\"hi\");\n}\n```\nAfter the block"
                .to_owned(),
        ),
    );
    m
}

#[test]
fn inline_markdown_renders_as_styled_runs_and_code_as_highlighted_lines() {
    use crate::turn::{SpanStyle, Voice};
    let mut m = markdown(Size::new(80, 24));
    Scenes::ticks(&mut m, Motion::ARRIVAL_STEPS);
    let mut lines = Vec::new();
    let root = view(&m, &clock());
    texts(&root, &mut lines);
    assert!(
        lines.iter().any(|l| l == "bold" || l.starts_with("bold,")),
        "the markers are stripped: {lines:?}"
    );
    assert!(!lines.iter().any(|l| l.contains("**")), "{lines:?}");
    assert!(!lines.iter().any(|l| l.contains("](")), "{lines:?}");
    let pen = Pen::of(&m.theme().palette());
    let mut found = Vec::new();
    styles(&root, &mut found);
    let prose = RowKind::Prose(Voice::Agent);
    assert!(found.contains(&pen.span(prose, &SpanStyle::Bold)));
    assert!(found.contains(&pen.span(prose, &SpanStyle::Italic)));
    assert!(found.contains(&pen.code), "the inline code run's pen");
    assert!(
        found
            .iter()
            .any(|s| s.link.as_deref() == Some("https://example.com")),
        "the link span carries its url"
    );
    assert!(
        found
            .iter()
            .any(|s| s.bg == pen.code.bg && s.fg != pen.code.fg && s.link.is_none()),
        "a syntect colour sits on the code surface"
    );
}

#[test]
fn snap_markdown_inline_80x24() {
    insta::assert_snapshot!(screen(&markdown(Size::new(80, 24))));
}

#[test]
fn snap_markdown_inline_120x40() {
    insta::assert_snapshot!(screen(&markdown(Size::new(120, 40))));
}

#[test]
fn snap_debug_chrome_80x24() {
    insta::assert_snapshot!(screen(&markdown(Size::new(80, 24)).with_debug(true)));
}

#[test]
fn the_dock_is_one_quiet_line_when_nothing_waits() {
    let m = model();
    assert_eq!(m.dock().len(), 0);
    let mut lines = Vec::new();
    texts(&view(&m, &clock()), &mut lines);
    assert!(lines.contains(&"nothing needs you".to_owned()), "{lines:?}");
}

/// A dock with a proposal in front of a blocker: the front card carries
/// its buttons, the queued card is text alone.
fn docked(size: Size) -> Model {
    let mut m = model_at(size);
    for chunk in [
        serde_json::json!({"chunk": {
            "id": "tui2-3", "status": "backlog",
            "reprioritize": {"suggested_priority": 300, "reason": "later"},
        }}),
        serde_json::json!({"chunk": {
            "id": "tui2-4", "status": "blocked",
            "blocked_by": {"kind": "awaiting_human", "reason": "needs a decision"},
        }}),
    ] {
        update(
            &mut m,
            Msg::Engine(crate::engine::EngineEvent::local(
                crate::harness::fixtures::frame("roadmap", "chunk_changed", chunk),
            )),
        );
    }
    m
}

fn model_at(size: Size) -> Model {
    Model::new(size)
}

#[test]
fn the_dock_grows_a_row_per_card_and_buttons_sit_on_the_front_card_only() {
    let m = docked(Size::new(80, 24));
    assert_eq!(m.dock().len(), 2);
    assert_eq!(m.stage_rows(), 13, "two cards inside the panel ring");
    let mut lines = Vec::new();
    let root = view(&m, &clock());
    texts(&root, &mut lines);
    assert!(
        lines.contains(&"◆ tui2-3: priority → 300 (later)".to_owned()),
        "{lines:?}"
    );
    assert!(lines.contains(&"[y] accept".to_owned()));
    assert!(lines.contains(&"[n] reject".to_owned()));
    assert!(
        lines.contains(&"⊗ tui2-4 blocked: needs a decision".to_owned()),
        "the queued card shows as text"
    );
    assert!(
        !lines.contains(&"[c] clear".to_owned()),
        "the queued card has no buttons yet"
    );
    assert_eq!(
        &lines[lines.len() - 4..],
        [" y ", "accept", " n ", "reject"],
        "{lines:?}"
    );
    let widgets = laid_out(&m);
    assert!(widgets.contains(&Widget::Accept));
    assert!(widgets.contains(&Widget::Reject));
    assert!(!widgets.contains(&Widget::Clear));
}

fn rendered_dock_click(m: &Model, widget: Widget) -> Msg {
    let resolved = layout(&view(m, &clock()), m.size());
    let rect = resolved
        .placed()
        .iter()
        .find(|placed| placed.id == Some(widget.id()))
        .expect("the restored card has its action button")
        .rect;
    let mut hits = crate::hit::HitRegistry::new();
    hits.register_layout(&resolved, 0);
    let target = hits.hit(rect.x, rect.y);
    assert_eq!(target, Some(widget.id()));
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), rect.x, rect.y),
        target,
    }
}

#[test]
fn refused_dock_answers_paint_the_error_and_restore_actionable_cards_in_both_themes() {
    use crate::cockpit::{Cmd, EngineCmd, ProposalKind};
    use crate::engine::EngineEvent;

    for theme in [ThemeMode::Light, ThemeMode::Dark] {
        for (widget, key) in [
            (Widget::Accept, 'y'),
            (Widget::Reject, 'n'),
            (Widget::Clear, 'c'),
        ] {
            for click in [false, true] {
                let mut m = Model::new(Size::new(120, 40)).with_theme(Theme::forced(theme));
                let front = if widget == Widget::Clear {
                    serde_json::json!({"id": "original", "blocked_by": {
                        "kind": "awaiting_human", "reason": "original decision"
                    }})
                } else {
                    serde_json::json!({"id": "original", "reprioritize": {
                        "suggested_priority": 300, "reason": "original decision"
                    }})
                };
                for chunk in [
                    front,
                    serde_json::json!({"id": "queued", "blocked_by": {
                        "kind": "awaiting_human", "reason": "queued decision"
                    }}),
                ] {
                    update(
                        &mut m,
                        Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
                            "roadmap",
                            "chunk_changed",
                            serde_json::json!({"chunk": chunk}),
                        ))),
                    );
                }
                let before: Vec<_> = m.dock().cards().cloned().collect();
                let input = if click {
                    rendered_dock_click(&m, widget)
                } else {
                    Msg::Key(KeyEvent::new(KeyCode::Char(key)))
                };
                let cmd = if widget == Widget::Clear {
                    EngineCmd::ClearBlocker {
                        chunk: "original".to_owned(),
                    }
                } else {
                    EngineCmd::ResolveProposal {
                        chunk: "original".to_owned(),
                        kind: ProposalKind::Reprioritize,
                        accept: widget == Widget::Accept,
                    }
                };
                assert_eq!(update(&mut m, input), vec![Cmd::Engine(cmd.clone())]);
                assert!(!screen(&m).contains("original decision"));
                update(
                    &mut m,
                    Msg::Engine(EngineEvent::PortFailed {
                        cmd,
                        what: "answer".to_owned(),
                        error: "decision refused".to_owned(),
                    }),
                );
                assert_eq!(m.dock().cards().cloned().collect::<Vec<_>>(), before);
                let painted = screen(&m);
                assert!(
                    painted.contains("engine answer: decision refused"),
                    "{painted}"
                );
                let original = painted
                    .find("original decision")
                    .expect("restored card painted");
                let queued = painted
                    .find("queued decision")
                    .expect("queued card painted");
                assert!(original < queued, "{painted}");
                let actions: &[Widget] = if widget == Widget::Clear {
                    &[Widget::Clear]
                } else {
                    &[Widget::Accept, Widget::Reject]
                };
                for action in actions {
                    let input = rendered_dock_click(&m, *action);
                    let mut retry = m.clone();
                    let commands = update(&mut retry, input);
                    assert!(matches!(commands.as_slice(), [Cmd::Engine(_)]));
                    assert_eq!(
                        retry.dock().cards().cloned().collect::<Vec<_>>(),
                        before[1..]
                    );
                    assert!(screen(&retry).contains("engine answer: decision refused"));
                }
            }
        }
    }
}

#[test]
fn a_switched_tab_shows_its_placeholder_and_the_bar_marks_it() {
    use crate::input::{KeyEvent as Key, Modifiers as Mods};
    let mut m = Scenes::streaming(Size::new(80, 24));
    update(&mut m, Msg::Key(Key::new(KeyCode::Tab)));
    let mut lines = Vec::new();
    let root = view(&m, &clock());
    texts(&root, &mut lines);
    // The pinned heading is the empty tab's whole answer — there is no
    // note under it saying the same thing a second time.
    assert!(
        lines.contains(&"changes · nothing journaled yet".to_owned()),
        "{lines:?}"
    );
    assert!(!lines.contains(&"Still streaming".to_owned()));
    let pen = Pen::of(&m.theme().palette());
    let mut found = Vec::new();
    styles(&root, &mut found);
    assert!(found.contains(&pen.phase_current), "Changes is bright");
    update(&mut m, Msg::Key(Key::new(KeyCode::Tab).with(Mods::SHIFT)));
    let mut lines = Vec::new();
    texts(&view(&m, &clock()), &mut lines);
    assert!(lines.iter().any(|l| l.contains("Still streaming")));
}

#[test]
fn snap_dock_cards_80x24() {
    insta::assert_snapshot!(screen(&docked(Size::new(80, 24))));
}

#[test]
fn snap_tab_changes_80x24() {
    let mut m = Scenes::streaming(Size::new(80, 24));
    Scenes::settle(&mut m);
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Tab)));
    insta::assert_snapshot!(screen(&m));
}

/// A turn where the agent edited a file: prose, then the edit card.
fn edited(size: Size) -> Model {
    use crate::agent::EditDiff;
    let mut m = Model::new(size);
    agent(
        &mut m,
        AgentEvent::Text("Renaming the helper.\n".to_owned()),
    );
    agent(
        &mut m,
        AgentEvent::ToolCall {
            id: "e1".to_owned(),
            title: "edit src/main.rs".to_owned(),
            verb: ToolVerb::Edit,
            status: ToolStatus::Completed,
            diff: Some(EditDiff {
                path: "src/main.rs".to_owned(),
                old_text: "fn main() {\n    old_helper();\n    keep();\n}\n".to_owned(),
                new_text: "fn main() {\n    new_helper();\n    keep();\n}\n".to_owned(),
            }),
            images: Vec::new(),
            record: Default::default(),
        },
    );
    agent(&mut m, AgentEvent::Text("Done.".to_owned()));
    m
}

#[test]
fn snap_edit_card_80x24() {
    let m = edited(Size::new(80, 24));
    assert_eq!(m.turn().len(), 3, "the card is one collapsed row");
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_edit_card_120x40() {
    insta::assert_snapshot!(screen(&edited(Size::new(120, 40))));
}

#[test]
fn snap_edit_card_expanded_80x24() {
    let mut m = edited(Size::new(80, 24));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(
        m.turn().rows().iter().any(|r| r.kind == RowKind::DiffAdd),
        "the diff is on the stage"
    );
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_edit_card_expanded_120x40() {
    let mut m = edited(Size::new(120, 40));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn the_popup_rows_sit_above_the_prompt_selected_in_the_cursors_pen() {
    let m = completing(Size::new(80, 24));
    let root = view(&m, &clock());
    let resolved = layout(&root, m.size());
    let rows: Vec<(usize, u16)> = resolved
        .placed()
        .iter()
        .filter_map(|p| Widget::completion_index(p.id?).map(|i| (i, p.rect.y)))
        .collect();
    let prompt = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(Widget::Prompt.id()))
        .expect("the prompt is laid out")
        .rect;
    assert_eq!(rows.len(), 4, "one hittable row per candidate");
    assert!(
        rows.iter()
            .all(|&(i, y)| y < prompt.y && y == rows[0].1 + i as u16),
        "the popup stacks above the prompt: {rows:?} vs prompt y {}",
        prompt.y
    );
    let mut found = Vec::new();
    styles(&root, &mut found);
    let pen = Pen::of(&m.theme().palette());
    assert!(found.contains(&pen.cursor), "the selected row is inverted");
    let mut lines = Vec::new();
    texts(&root, &mut lines);
    assert_eq!(
        &lines[lines.len() - 6..],
        [" ↑↓ ", "select", " enter ", "accept", " esc ", "close"],
        "the footer explains the popup's keys"
    );
}

#[test]
fn snap_completion_popup_80x24() {
    insta::assert_snapshot!(screen(&completing(Size::new(80, 24))));
}

#[test]
fn snap_completion_popup_120x40() {
    insta::assert_snapshot!(screen(&completing(Size::new(120, 40))));
}

/// Awaiting input with a three-line prompt typed via Alt-Enter and the
/// command popup opened by `/` at the third line's start.
fn multiline_command(size: Size) -> Model {
    let mut m = awaiting(size);
    for c in "fix the tests".chars() {
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Enter).with(crate::input::Modifiers::ALT)),
    );
    for c in "then run".chars() {
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Enter).with(crate::input::Modifiers::SHIFT)),
    );
    for c in "/va".chars() {
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    m
}

#[test]
fn snap_multiline_command_popup_80x24() {
    let m = multiline_command(Size::new(80, 24));
    assert_eq!(m.prompt_rows(), 3, "the box grew to its lines");
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_multiline_command_popup_120x40() {
    insta::assert_snapshot!(screen(&multiline_command(Size::new(120, 40))));
}

#[test]
fn the_changes_list_names_each_entry_with_counts_and_verdict() {
    let m = Scenes::changed(Size::new(120, 40));
    let mut lines = Vec::new();
    texts(&view(&m, &clock()), &mut lines);
    assert!(
        lines.contains(&"▸ src/lib.rs · +2 −1 · parse ok".to_owned()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"▸ src/parser.rs · +4 −0 · parse failed".to_owned()),
        "{lines:?}"
    );
}

#[test]
fn an_expanded_change_lays_out_its_buttons_and_diff() {
    let mut m = Scenes::changed(Size::new(120, 40));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 5),
            target: Some(Widget::change_row(0)),
        },
    );
    let root = view(&m, &clock());
    let resolved = layout(&root, m.size());
    let placed: Vec<_> = resolved.placed().iter().filter_map(|p| p.id).collect();
    assert!(placed.contains(&Widget::change_open(0)));
    assert!(placed.contains(&Widget::change_undo(0)));
    assert!(placed.contains(&Widget::change_approve(0)));
    let mut lines = Vec::new();
    texts(&root, &mut lines);
    assert!(lines.contains(&"  -fn old() {}".to_owned()), "{lines:?}");
    assert!(lines.contains(&"  +fn renamed() {}".to_owned()));
}

#[test]
fn snap_changes_list_120x40() {
    insta::assert_snapshot!(screen(&Scenes::changed(Size::new(120, 40))));
}

#[test]
fn snap_changes_diff_120x40() {
    let mut m = Scenes::changed(Size::new(120, 40));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 5),
            target: Some(Widget::change_row(0)),
        },
    );
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_changes_runs_120x40() {
    use crate::workbench::RunRow;
    let mut m = Scenes::changed(Size::new(120, 40));
    let row = |name: &str, passed: bool, detail: &str| RunRow {
        name: name.to_owned(),
        passed,
        detail: detail.to_owned(),
    };
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Ran {
            command: "test".to_owned(),
            rows: vec![
                row("cockpit::update::tests::keys", true, "12ms"),
                row(
                    "cockpit::view::tests::snap_edit_card",
                    false,
                    "snapshot mismatch",
                ),
                row("turn::stream::tests::wrap", true, "3ms"),
            ],
        }),
    );
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn snap_changes_viewer_120x40() {
    let mut m = Scenes::changed(Size::new(120, 40));
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Opened {
            path: "src/lib.rs".to_owned(),
            content: "fn renamed() {}\npub use renamed as old;\n".to_owned(),
        }),
    );
    insta::assert_snapshot!(screen(&m));
}

// ── The Trace tab ──────────────────────────────────────────────────────

#[test]
fn snap_trace_tail_80x24() {
    let m = Scenes::long_trace(Size::new(80, 24));
    let (total, height) = m.trace_window();
    assert!(
        total > height,
        "the trace outgrows the stage: {total} > {height}"
    );
    let screen = screen(&m);
    assert!(
        screen.contains("think trace"),
        "the heading is pinned: {screen}"
    );
    assert!(screen.contains("think 20"), "the tail follows: {screen}");
    assert!(
        !screen.contains("think 1 ·"),
        "the head scrolled off: {screen}"
    );
    insta::assert_snapshot!(screen);
}

#[test]
fn snap_trace_scrolled_home_80x24() {
    let mut m = Scenes::long_trace(Size::new(80, 24));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Home)));
    Scenes::settle(&mut m);
    assert_eq!(m.trace_top(), 0, "Home pins the body's first row");
    let screen = screen(&m);
    assert!(
        screen.contains("think trace"),
        "the heading is still pinned: {screen}"
    );
    assert!(screen.contains("think 1 ·"), "the head shows: {screen}");
    assert!(
        !screen.contains("think 20"),
        "the tail scrolled off: {screen}"
    );
    insta::assert_snapshot!(screen);
}

#[test]
fn the_wheel_and_the_page_keys_move_the_trace_not_the_turn() {
    let mut m = Scenes::long_trace(Size::new(80, 24));
    let turn_top = m.top_row();
    let tail = m.trace_top();
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::ScrollUp, 10, 8),
            target: None,
        },
    );
    Scenes::settle(&mut m);
    assert!(m.trace_top() < tail, "the wheel left the tail");
    assert_eq!(m.top_row(), turn_top, "the turn's own window stayed put");

    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::PageUp)));
    Scenes::settle(&mut m);
    assert_eq!(m.trace_top(), 0, "a page from there reaches the head");
    assert_eq!(m.top_row(), turn_top, "still the turn's own window");

    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::End)));
    Scenes::settle(&mut m);
    assert_eq!(m.trace_top(), tail, "End follows the tail again");
}

#[test]
fn snap_trace_feed_120x40() {
    let m = Scenes::traced(Size::new(120, 40));
    assert_eq!(m.tab(), StageTab::Trace);
    let screen = screen(&m);
    assert!(screen.contains("think trace"), "{screen}");
    assert!(screen.contains("[Checkpoint]"), "{screen}");
    assert!(
        screen.contains("revised by 5"),
        "the revision hint shows: {screen}"
    );
    assert!(
        screen.contains("alt path (merged) from 2"),
        "the branch list shows: {screen}"
    );
    assert!(screen.contains("★"), "the pin star shows: {screen}");
    insta::assert_snapshot!(screen);
}

#[test]
fn snap_trace_step_expanded_120x40() {
    let mut m = Scenes::traced(Size::new(120, 40));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 8),
            target: Some(Widget::trace_row(0)),
        },
    );
    assert!(m.trace().is_expanded(1));
    let screen = screen(&m);
    for field in [
        "purpose · hypothesis",
        "context · the trace tab",
        "thought · the feed can drive the whole surface",
        "outcome · it can",
        "next action · wire the view",
        "rationale · one fold path for feed and port",
        "confidence · 0.62",
        "uncertainty · no branch case yet",
        "estimated total · 4",
        "depends on · 7, 9",
        "execution ref · task:trace-state",
        "[Unpin]",
    ] {
        assert!(screen.contains(field), "{field} missing:\n{screen}");
    }
    insta::assert_snapshot!(screen);
}

#[test]
fn snap_trace_ship_timeline_120x40() {
    let mut m = Model::new(Size::new(120, 40));
    Scenes::engine_frame(
        &mut m,
        "ship",
        "action_recorded",
        serde_json::json!({"task_id": "wire", "action": {
            "type": "test", "description": "red first"}}),
    );
    Scenes::engine_frame(
        &mut m,
        "ship",
        "check_recorded",
        serde_json::json!({"check": {"name": "check", "passed": true, "exit_code": 0,
            "results": {"duration_ms": 800}}}),
    );
    Scenes::engine_frame(
        &mut m,
        "ship",
        "check_recorded",
        serde_json::json!({"check": {"name": "test", "passed": false, "exit_code": 101,
            "results": {"duration_ms": 5400}}}),
    );
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 40, 2),
            target: Some(Widget::TabTrace.id()),
        },
    );
    let screen = screen(&m);
    assert!(screen.contains("✓ check · exit 0 · 800ms"), "{screen}");
    assert!(screen.contains("✗ test · exit 101 · 5.4s"), "{screen}");
    insta::assert_snapshot!(screen);
}

#[test]
fn the_trace_graph_block_is_laid_out_only_with_branches() {
    let m = Scenes::traced(Size::new(120, 40));
    let resolved = layout(&view(&m, &clock()), Size::new(120, 40));
    assert!(
        placement(&resolved).contains("trace graph"),
        "the graph block is placed"
    );
    let mut plain = Model::new(Size::new(120, 40));
    Scenes::engine_frame(
        &mut plain,
        "think",
        "step_appended",
        serde_json::json!({"step": {"step_number": 1}}),
    );
    update(
        &mut plain,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 40, 2),
            target: Some(Widget::TabTrace.id()),
        },
    );
    let resolved = layout(&view(&plain, &clock()), Size::new(120, 40));
    assert!(
        !placement(&resolved).contains("trace graph"),
        "no branches, no block"
    );
}

#[test]
fn snap_roadmap_list_120x40() {
    let m = Scenes::roadmapped(Size::new(120, 40));
    assert_eq!(m.tab(), StageTab::Roadmap);
    let screen = screen(&m);
    assert!(
        screen.contains("roadmap   1 done · 1 active · 1 pending · 0 backlog"),
        "the heading carries the accounting: {screen}"
    );
    assert!(screen.contains("«Engine» 1/1"), "{screen}");
    assert!(screen.contains("«TUI rewrite» 0/2"), "{screen}");
    assert!(screen.contains("▸ in progress · 1"), "{screen}");
    assert!(screen.contains("○ pending · 1"), "{screen}");
    assert!(screen.contains("▸ Rail and dock"), "{screen}");
    assert!(screen.contains("▸ Roadmap view"), "{screen}");
    assert!(
        !screen.contains("· pending ·"),
        "the status word left the rows for the section header: {screen}"
    );
    assert!(
        screen.contains("✓ done · 1") && !screen.contains("ACP transport"),
        "done collapsed to a count: {screen}"
    );
    assert!(
        screen.contains("Roadmap view              priority → 100 · sooner"),
        "the proposal row shows: {screen}"
    );
    assert!(screen.contains("[Accept]"), "{screen}");
    assert!(
        screen.contains("idea · alice — make startup faster · 0.8 · researched"),
        "the signal row shows: {screen}"
    );
    assert!(screen.contains("[Promote]"), "{screen}");
    insta::assert_snapshot!(screen);
}

#[test]
fn snap_roadmap_detail_120x40() {
    let mut m = Scenes::roadmapped(Size::new(120, 40));
    // Row 1 in list order is the pending proposal-carrying chunk.
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 20),
            target: Some(Widget::roadmap_row(1)),
        },
    );
    let screen = screen(&m);
    for needle in [
        "▾ Roadmap view",
        "[Start]",
        "The Roadmap tab of the cockpit",
        "The tech-tree, the list and the actions.",
        "• the DAG renders",
        "• clicks select",
        "deps: tui2-2-rail",
        "refs: think:190",
    ] {
        assert!(screen.contains(needle), "{needle} missing:\n{screen}");
    }
    insta::assert_snapshot!(screen);
}

#[test]
fn the_in_progress_detail_offers_complete_and_the_empty_tab_says_so() {
    let mut m = Scenes::roadmapped(Size::new(120, 40));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 19),
            target: Some(Widget::roadmap_row(0)),
        },
    );
    let screen = screen(&m);
    assert!(screen.contains("[Complete]"), "{screen}");
    assert!(!screen.contains("[Start]"), "{screen}");

    let empty = Model::new(Size::new(120, 40)).with_tab(StageTab::Roadmap);
    let screen = self::screen(&empty);
    assert!(
        screen.contains("no roadmap yet — the store is empty"),
        "{screen}"
    );
}

#[test]
fn a_hover_line_shows_under_the_tech_tree() {
    let mut m = Scenes::roadmapped(Size::new(120, 40));
    m.set_roadmap_graph_rect(Some(crate::layout::Rect::new(2, 4, 100, 12)));
    let nodes = m.roadmap().dag_nodes();
    let (col, row) = (
        (2.0 + nodes[1].x - 0.5) as u16,
        (4.0 + nodes[1].y - 0.5) as u16,
    );
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Move, col, row),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    let screen = screen(&m);
    assert!(
        screen.contains("▲ Rail and dock —"),
        "the hover line shows: {screen}"
    );
}

#[test]
fn a_tree_selected_done_chunk_shows_its_card_under_the_graph() {
    let mut m = Scenes::roadmapped(Size::new(120, 40));
    // The DAG can select a done chunk the list no longer rows.
    m.set_roadmap_graph_rect(Some(crate::layout::Rect::new(2, 4, 100, 12)));
    let nodes = m.roadmap().dag_nodes();
    let done = nodes
        .iter()
        .position(|node| node.status == "done")
        .expect("the done chunk is on the tree");
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(
                MouseKind::Down(Button::Left),
                (2.0 + nodes[done].x - 0.5) as u16,
                (4.0 + nodes[done].y - 0.5) as u16,
            ),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert_eq!(m.roadmap().selected().map(|c| c.id.as_str()), Some("acp-2"));
    let screen = screen(&m);
    assert!(
        screen.contains("✓ ACP transport —"),
        "the card shows under the graph: {screen}"
    );
}

/// A think step landing as the engines would broadcast it.
fn think_step_frame(m: &mut Model, number: u32) {
    update(
        m,
        Msg::Engine(crate::engine::EngineEvent::local(
            crate::harness::fixtures::frame(
                "think",
                "step_appended",
                serde_json::json!({"step": {
                    "step_number": number, "purpose": "hypothesis", "confidence": 0.7}}),
            ),
        )),
    );
}

#[test]
fn snap_undocked_trace_180x50() {
    let mut m = Scenes::roadmapped(Size::new(180, 50));
    assert_eq!(m.undocked_tab(), Some(StageTab::Trace));
    think_step_frame(&mut m, 7);
    let screen = screen(&m);
    assert!(screen.contains("⇱ Trace"), "{screen}");
    assert!(screen.contains("✦ think 7"), "{screen}");
    assert!(
        screen.contains("▲ Rail and dock"),
        "the rail's summary shows beside it: {screen}"
    );
    insta::assert_snapshot!(screen);
}

#[test]
fn snap_undock_placement_180x50() {
    let resolved = layout(
        &view(&Scenes::roadmapped(Size::new(180, 50)), &clock()),
        Size::new(180, 50),
    );
    insta::assert_snapshot!(placement(&resolved));
}

#[test]
fn the_undock_needs_width_and_never_doubles_the_active_tab() {
    let narrow = Scenes::roadmapped(Size::new(159, 50));
    assert_eq!(narrow.undocked_tab(), None);
    let resolved = layout(&view(&narrow, &clock()), Size::new(159, 50));
    assert!(
        !placement(&resolved).contains("undock"),
        "no third column below 160 columns"
    );
    let on_trace = Model::new(Size::new(180, 50)).with_tab(StageTab::Trace);
    assert_eq!(
        on_trace.undocked_tab(),
        Some(StageTab::Changes),
        "the Trace tab on stage undocks the journal instead"
    );
}

#[test]
fn mouse_hits_resolve_across_all_three_columns() {
    let mut m = Scenes::roadmapped(Size::new(180, 50));
    think_step_frame(&mut m, 7);
    let resolved = layout(&view(&m, &clock()), m.size());
    let mut hits = crate::hit::HitRegistry::new();
    hits.register_layout(&resolved, 0);
    let rect_of = |id: crate::WidgetId| {
        resolved
            .placed()
            .iter()
            .find(|p| p.id == Some(id))
            .map(|p| p.rect)
            .expect("placed")
    };
    // The left column: the rail's toggle.
    let rail = rect_of(Widget::RailToggle.id());
    assert_eq!(hits.hit(rail.x, rail.y), Some(Widget::RailToggle.id()));
    // The middle: the roadmap list's first row.
    let row = rect_of(Widget::roadmap_row(0));
    assert_eq!(hits.hit(row.x + 2, row.y), Some(Widget::roadmap_row(0)));
    // The right column: the undocked trace step — and the click lands.
    let step = rect_of(Widget::trace_row(0));
    let target = hits.hit(step.x + 2, step.y);
    assert_eq!(target, Some(Widget::trace_row(0)));
    assert!(!m.trace().is_expanded(7));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), step.x + 2, step.y),
            target,
        },
    );
    assert!(m.trace().is_expanded(7), "the undocked row took the click");
}

#[test]
fn the_rail_swaps_its_two_line_summary_for_the_pixel_block() {
    let mut m = Scenes::roadmapped(Size::new(120, 40));
    let cells = screen(&m);
    assert!(cells.contains("▲ Rail and dock"), "{cells}");
    assert!(cells.contains("1 before · 1 after"), "{cells}");
    assert!(cells.contains("▰"), "the progress bar renders: {cells}");
    assert!(cells.contains("1/3"), "the bar carries the count: {cells}");
    // The runtime reported a placed rect: pixels cover the block and
    // the text summary leaves.
    m.set_rail_graph_rect(Some(crate::layout::Rect::new(1, 10, 20, 6)));
    let swapped = screen(&m);
    assert!(!swapped.contains("1 before · 1 after"), "{swapped}");
    assert!(
        swapped.contains("▰"),
        "the progress bar stays in cells: {swapped}"
    );
}

// ── The launcher (Home tab) and the accelerator sheet ───────────────

/// The launcher on a terminal whose probe answered kitty's text sizing
/// handshake.
fn launcher_scaled(size: Size) -> Model {
    let mut m = Scenes::launcher(size);
    update(&mut m, Msg::TextSizing(true));
    m
}

/// The rect `widget` was placed at, if it was placed at all.
fn rect_of(m: &Model, widget: Widget) -> Option<crate::layout::Rect> {
    layout(&view(m, &clock()), m.size())
        .placed()
        .iter()
        .find_map(|p| (p.id == Some(widget.id())).then_some(p.rect))
}

#[test]
fn without_text_sizing_the_project_card_is_the_four_labelled_rows_it_always_was() {
    let m = Scenes::launcher(Size::new(120, 40));
    assert!(m.headline().is_none(), "no headline without the protocol");
    assert!(rect_of(&m, Widget::CardHeadline).is_none());
    let s = screen(&m);
    assert!(s.contains("name"), "the name keeps its label row: {s}");
    assert!(s.contains("magistr"), "{s}");
}

#[test]
fn text_sizing_reserves_two_rows_and_drops_the_name_label() {
    let m = launcher_scaled(Size::new(120, 40));
    assert_eq!(m.headline(), Some("magistr"));
    let rect = rect_of(&m, Widget::CardHeadline).expect("the slot is laid out");
    assert_eq!(rect.height, crate::render::Headline::ROWS);
    assert!(rect.width >= 2, "wide enough for one scaled character");
    let s = screen(&m);
    assert!(
        !s.contains("name     magistr"),
        "the label row is gone — the headline is the name: {s}"
    );
    assert!(s.contains("Rust / Cargo"), "the other rows stay: {s}");
}

#[test]
fn the_reserved_rows_are_blank_so_the_scaled_block_owns_them() {
    // The terminal draws the run across both rows; anything of ours in
    // them would be painted over, then put back by the next diff.
    let m = launcher_scaled(Size::new(120, 40));
    let rect = rect_of(&m, Widget::CardHeadline).expect("the slot is laid out");
    let size = m.size();
    let root = view(&m, &clock());
    let resolved = layout(&root, size);
    let mut buffer = Buffer::new(size.cols, size.rows);
    paint(&root, &resolved, &mut buffer);
    for row in rect.y..rect.bottom() {
        for col in rect.x..rect.right() {
            let symbol = buffer.get(col, row).map(|cell| cell.symbol.to_string());
            assert_eq!(symbol.as_deref(), Some(" "), "({col}, {row}) is blank");
        }
    }
}

#[test]
fn a_project_with_no_config_wears_no_headline() {
    // The card points at `magistr init` instead; there is no name yet.
    let mut m = Model::new(Size::new(120, 40)).with_tab(StageTab::Home);
    update(&mut m, Msg::TextSizing(true));
    update(
        &mut m,
        Msg::Job {
            id: crate::job::JobId::new(0),
            output: crate::job::JobOutput::Launcher(Box::new(crate::cockpit::LauncherData {
                project: "magistr".to_owned(),
                has_config: false,
                ..crate::cockpit::LauncherData::default()
            })),
        },
    );
    assert!(m.headline().is_none());
    assert!(rect_of(&m, Widget::CardHeadline).is_none());
}

#[test]
fn a_tab_away_from_home_lays_out_no_headline() {
    let mut m = launcher_scaled(Size::new(120, 40));
    m = m.with_tab(StageTab::Turn);
    assert!(rect_of(&m, Widget::CardHeadline).is_none());
}

#[test]
fn snap_launcher_80x24() {
    let m = Scenes::launcher(Size::new(80, 24));
    let s = screen(&m);
    assert!(s.contains("Rust / Cargo"), "{s}");
    assert!(s.contains("master, clean"), "{s}");
    assert!(s.contains("ACP (available)"), "{s}");
    assert!(s.contains("▰"), "the progress bar renders: {s}");
    assert!(s.contains("2/4"), "{s}");
    assert!(
        s.contains("nothing ready"),
        "the pending chunk is dep-gated: {s}"
    );
    assert!(s.contains("next iteration"), "{s}");
    assert!(s.contains("gates"), "{s}");
    insta::assert_snapshot!(s);
}

#[test]
fn snap_launcher_120x40() {
    insta::assert_snapshot!(screen(&Scenes::launcher(Size::new(120, 40))));
}

/// The elision rule itself: whole chips only, dropped from the tail.
#[test]
fn the_footer_takes_whole_chips_and_drops_the_tail() {
    // The Home context costs 86 columns — six chips (18, 8, 9, 11, 17,
    // 13) and the five gaps between them.
    let home = Binding::HINT_HOME;
    let whole: usize =
        home.iter().map(chip_cols).sum::<usize>() + usize::from(Space::GAP) * (home.len() - 1);
    assert_eq!(whole, 86, "the Home context's own width");
    assert_eq!(footer_chips(home, 200), home.len(), "a wide bar shows all");
    assert_eq!(
        footer_chips(home, whole),
        home.len(),
        "an exact fit shows all"
    );
    assert_eq!(
        footer_chips(home, whole - 1),
        home.len() - 1,
        "one column short drops the whole trailing chip, not its label"
    );
    assert_eq!(
        footer_chips(home, 80),
        5,
        "the 80-column screen loses ctrl-d quit"
    );
    assert_eq!(
        footer_chips(home, chip_cols(&home[0])),
        1,
        "the head survives at its own width"
    );
    assert_eq!(
        footer_chips(home, chip_cols(&home[0]) - 1),
        0,
        "a bar too narrow for even the head shows nothing rather than half a chip"
    );
    assert_eq!(footer_chips(&[], 80), 0, "an empty context has no chips");
}

/// The rendered bar: at 80 columns the Home footer ends on a whole
/// chip, and the binding it dropped is back at 120.
#[test]
fn the_narrow_home_footer_ends_on_a_whole_chip() {
    let narrow = screen(&Scenes::launcher(Size::new(80, 24)));
    let footer = narrow.lines().next_back().unwrap_or_default();
    assert!(
        footer.contains("keyboard help"),
        "the last chip that fits keeps its label: {footer:?}"
    );
    assert!(
        !footer.contains("ctrl-d"),
        "the chip that did not fit is gone whole, cap and all: {footer:?}"
    );
    let wide = screen(&Scenes::launcher(Size::new(120, 40)));
    assert!(
        wide.contains("ctrl-d ") && wide.contains("quit"),
        "a wide screen still shows every chip"
    );
}

#[test]
fn snap_launcher_unconfigured_80x24() {
    let mut m = Model::new(Size::new(80, 24)).with_tab(StageTab::Home);
    update(
        &mut m,
        Msg::Job {
            id: crate::job::JobId::new(0),
            output: crate::job::JobOutput::Launcher(Box::new(crate::cockpit::LauncherData {
                project: "fresh".to_owned(),
                has_config: false,
                ..Default::default()
            })),
        },
    );
    let s = screen(&m);
    assert!(s.contains("not configured"), "{s}");
    assert!(
        s.contains("magistr init"),
        "the card points at the CLI interview: {s}"
    );
    insta::assert_snapshot!(s);
}

#[test]
fn snap_launcher_loading_80x24() {
    // Before the job's round-trip the cards say they are loading.
    let m = Model::new(Size::new(80, 24)).with_tab(StageTab::Home);
    let s = screen(&m);
    assert!(s.contains("loading project status…"), "{s}");
    insta::assert_snapshot!(s);
}

#[test]
fn snap_accelerator_sheet_80x24() {
    let mut m = Scenes::launcher(Size::new(80, 24));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char('?'))));
    assert!(m.is_sheet_open());
    insta::assert_snapshot!(screen(&m));
}

#[test]
fn the_registrys_row_count_is_what_the_sheet_body_paints() {
    // Binding::sheet_rows is what the scroll window is measured
    // against; sheet_body is what the window is taken from. They walk
    // Binding::ALL separately, so pin them to each other.
    let pen = Pen::for_theme(&Theme::default());
    assert_eq!(
        sheet_body(&pen).len(),
        crate::cockpit::Binding::sheet_rows()
    );
}

#[test]
fn every_binding_is_reachable_in_the_80x24_sheet() {
    // The registry outgrew one small screen, so
    // reachability is what the whole scroll shows rather than what one
    // frame does: page to the end and require the union of the frames
    // to hold every binding.
    let mut m = Scenes::launcher(Size::new(80, 24));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char('?'))));
    let (total, height) = m.sheet_window();
    assert!(
        height < total,
        "the registry should run past an 80x24 stage ({height} of {total})"
    );
    assert_eq!(m.sheet_top(), 0, "the sheet opens at its first group");
    let mut seen = screen(&m);
    for page in 0..total {
        if m.sheet_top() + height >= total {
            break;
        }
        let was = m.sheet_top();
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::PageDown)));
        Scenes::settle(&mut m);
        assert!(
            m.sheet_top() > was,
            "page {page} did not move the sheet past row {was}"
        );
        seen.push_str(&screen(&m));
    }
    for binding in crate::cockpit::Binding::ALL {
        assert!(
            seen.contains(binding.keys),
            "scrolling the 80x24 sheet never showed the keys of '{} {}'",
            binding.keys,
            binding.does
        );
        assert!(
            seen.contains(binding.does),
            "scrolling the 80x24 sheet never showed '{}'",
            binding.does
        );
    }
}

#[test]
fn the_sheets_own_keys_close_it_rather_than_scrolling_it() {
    // Esc and `?` stay the sheet's, and a scroll that reached the end
    // must not have turned End into a way out of the overlay.
    for code in [KeyCode::Esc, KeyCode::Char('?')] {
        let mut m = Scenes::launcher(Size::new(80, 24));
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char('?'))));
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::End)));
        Scenes::settle(&mut m);
        assert!(m.is_sheet_open(), "End left the sheet open");
        update(&mut m, Msg::Key(KeyEvent::new(code)));
        assert!(!m.is_sheet_open(), "{code:?} should close the sheet");
    }
}

#[test]
fn reopening_the_sheet_puts_it_back_at_the_top() {
    // A help overlay reopened is a fresh question, not the place the
    // last one was left.
    let mut m = Scenes::launcher(Size::new(80, 24));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char('?'))));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::End)));
    Scenes::settle(&mut m);
    assert!(m.sheet_top() > 0, "End moved the sheet");
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Esc)));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char('?'))));
    assert_eq!(m.sheet_top(), 0);
}

#[test]
fn the_sheet_says_where_it_stands_only_when_it_overflows() {
    let mut small = Scenes::launcher(Size::new(80, 24));
    update(&mut small, Msg::Key(KeyEvent::new(KeyCode::Char('?'))));
    let (total, height) = small.sheet_window();
    assert!(
        screen(&small).contains(&format!("1–{height} of {total}")),
        "the small sheet should say where it stands: {}",
        screen(&small)
    );

    // The registry grew past 62 rows with the loop's step and
    // checkpoint verbs, past 64 with the
    // pane's follow key and past 66 with the pane's
    // kill and copy keys; 68 holds all 58.
    let mut tall = Scenes::launcher(Size::new(120, 68));
    update(&mut tall, Msg::Key(KeyEvent::new(KeyCode::Char('?'))));
    let (total, height) = tall.sheet_window();
    assert!(height >= total, "the whole registry fits 120x68");
    assert!(
        !screen(&tall).contains(" of "),
        "a sheet that fits says nothing about its position: {}",
        screen(&tall)
    );
}

#[test]
fn the_sheet_lists_every_registered_binding() {
    // The sheet is generated from the registry: at a tall enough size
    // every binding's keys and description are on the screen.
    let mut m = Scenes::launcher(Size::new(120, 68));
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char('?'))));
    let s = screen(&m);
    for binding in crate::cockpit::Binding::ALL {
        assert!(
            s.contains(binding.keys),
            "the sheet is missing the keys of '{} {}': {s}",
            binding.keys,
            binding.does
        );
        assert!(
            s.contains(binding.does),
            "the sheet is missing '{}': {s}",
            binding.does
        );
    }
}

#[test]
fn the_launcher_buttons_are_hittable() {
    let m = Scenes::launcher(Size::new(120, 40));
    let root = view(&m, &clock());
    let resolved = layout(&root, m.size());
    let mut hits = crate::hit::HitRegistry::new();
    hits.register_layout(&resolved, 0);
    for widget in [
        Widget::LaunchNext,
        Widget::LaunchLoop,
        Widget::LaunchGates,
        Widget::LaunchRoadmap,
    ] {
        let rect = resolved
            .placed()
            .iter()
            .find(|p| p.id == Some(widget.id()))
            .map(|p| p.rect)
            .unwrap_or_else(|| panic!("{} was not laid out", widget.label()));
        assert_eq!(
            hits.hit(rect.x, rect.y),
            Some(widget.id()),
            "{} does not take its click",
            widget.label()
        );
    }
}

#[test]
fn the_launcher_grid_fills_the_stage_at_120x40() {
    // The dashboard acceptance: the four cards cover at least 75% of
    // the stage's cells — the 60%-dead-space text column is gone.
    let m = Scenes::launcher(Size::new(120, 40));
    let resolved = layout(&view(&m, &clock()), m.size());
    let rect = |id: crate::widget_id::WidgetId| {
        resolved
            .placed()
            .iter()
            .find(|p| p.id == Some(id))
            .map(|p| p.rect)
            .unwrap_or_else(|| panic!("widget {id:?} was not laid out"))
    };
    let stage = rect(Widget::Stage.id());
    let covered: u32 = [
        Widget::CardProject,
        Widget::CardRoadmap,
        Widget::CardSessions,
        Widget::CardActions,
    ]
    .into_iter()
    .map(|card| {
        let r = rect(card.id());
        u32::from(r.width) * u32::from(r.height)
    })
    .sum();
    let area = u32::from(stage.width) * u32::from(stage.height);
    assert!(
        covered * 4 >= area * 3,
        "the cards cover {covered} of the stage's {area} cells"
    );
}

/// The painted style of the cell at `(col, row)`.
fn cell_style(model: &Model, col: u16, row: u16) -> Style {
    let root = view(model, &FrameClock::new(7, Duration::from_millis(1500)));
    let resolved = layout(&root, model.size());
    let mut buffer = Buffer::new(model.size().cols, model.size().rows);
    paint(&root, &resolved, &mut buffer);
    buffer.get(col, row).expect("cell exists").style.clone()
}

#[test]
fn the_session_rows_are_hittable_and_light_on_hover() {
    let mut m = Scenes::launcher(Size::new(120, 40));
    let resolved = layout(&view(&m, &clock()), m.size());
    let rect = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(Widget::launch_session(0)))
        .map(|p| p.rect)
        .expect("the first session row is laid out");
    let mut hits = crate::hit::HitRegistry::new();
    hits.register_layout(&resolved, 0);
    assert_eq!(hits.hit(rect.x, rect.y), Some(Widget::launch_session(0)));
    let before = cell_style(&m, rect.x, rect.y);
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Move, rect.x, rect.y),
            target: Some(Widget::launch_session(0)),
        },
    );
    let after = cell_style(&m, rect.x, rect.y);
    assert_ne!(before, after, "the hovered row changes its paint");
}

#[test]
fn the_selected_session_row_is_raised_and_an_unresumable_one_says_so_on_the_card() {
    let mut m = Scenes::launcher(Size::new(120, 40));
    let resolved = layout(&view(&m, &clock()), m.size());
    let rect = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(Widget::launch_session(0)))
        .map(|p| p.rect)
        .expect("the first session row is laid out");
    let before = cell_style(&m, rect.x, rect.y);
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Down)));
    assert_eq!(m.session_selected(), Some(0));
    assert_ne!(
        before,
        cell_style(&m, rect.x, rect.y),
        "the arrows raise the selected row the way a hover does"
    );

    // The second row's session is gone from disk: activating it paints
    // the reason on the card and leaves the cockpit standing.
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), rect.x, rect.y),
            target: Some(Widget::launch_session(1)),
        },
    );
    assert!(!m.quit_requested(), "an unresumable row never quits");
    assert!(
        screen(&m).contains("no longer resumable"),
        "the card reports it:\n{}",
        screen(&m)
    );
}

// ── The screenshot gate: golden images ────────────────────

/// The settling step count really does clear every transient and land on
/// both periodic motions' starting phase.
#[test]
fn the_settling_step_count_clears_every_transient_and_keeps_both_phases() {
    assert_eq!(Scenes::SETTLE_STEPS % SHIMMER_STEPS, 0, "the shimmer wraps");
    assert_eq!(
        Scenes::SETTLE_STEPS % Motion::GLOW_STEPS,
        0,
        "the glow breathes"
    );
    assert_eq!(
        Scenes::SETTLE_STEPS % (Model::SPIN_STEPS * 4),
        0,
        "the spinner turns"
    );
    for (name, steps) in [
        ("arrival", Motion::ARRIVAL_STEPS),
        ("transition", Motion::TRANSITION_STEPS),
        ("sweep", Motion::SWEEP_STEPS),
        ("glide", Eased::STEPS),
    ] {
        assert!(Scenes::SETTLE_STEPS > steps, "{name} is over by then");
    }
}

/// The anti-flash guard: the panel conversion must never show
/// fewer information rows (rows carrying at least one alphanumeric
/// glyph) than the unframed layout did at 120x40. Baselines measured on
/// the pre-panel layout — chrome may add rows, never eat them.
#[test]
fn panel_chrome_keeps_density_at_120x40() {
    let info = |m: &Model| {
        screen(m)
            .lines()
            .filter(|line| line.chars().any(char::is_alphanumeric))
            .count()
    };
    let launcher_rows = info(&Scenes::launcher(Size::new(120, 40)));
    let streaming_rows = info(&Scenes::streaming(Size::new(120, 40)));
    assert!(launcher_rows >= 19, "launcher: {launcher_rows} info rows");
    assert!(
        streaming_rows >= 18,
        "streaming: {streaming_rows} info rows"
    );
}

// ── the pointer raises whatever it is over ────────────────

/// Every id-carrying rect the hit registry would register for `model`,
/// widest first so a parent is answered before its children.
fn registered(model: &Model) -> Vec<WidgetId> {
    let mut ids: Vec<WidgetId> = layout(&view(model, &clock()), model.size())
        .placed()
        .iter()
        .filter(|placed| !placed.rect.is_empty())
        .filter_map(|placed| placed.id)
        .collect();
    ids.dedup();
    ids
}

/// `model` painted at its size.
fn buffer_of(model: &Model) -> Buffer {
    let root = view(model, &clock());
    let resolved = layout(&root, model.size());
    let mut buffer = Buffer::new(model.size().cols, model.size().rows);
    paint(&root, &resolved, &mut buffer);
    buffer
}

/// The registered ids that answer the pointer with nothing, by name.
/// Every one is chrome — a bar, a panel, a pixel block, the prompt line,
/// a launcher card — and tinting one because the pointer crossed its
/// padding would be noise rather than feedback. Written down rather than
/// derived, so a widget that becomes registered without a hover decision
/// turns up as a diff here.
const CHROME: [&str; 22] = [
    "CardActions",
    "CardProject",
    "CardRoadmap",
    "CardSessions",
    "Chunk",
    "Dock",
    "Execute",
    "Footer",
    "Header",
    "HeaderBrand",
    "HeaderSpark",
    "Plan",
    "Prompt",
    "Rail",
    "RailGauge",
    "RailGraph",
    "Research",
    "RoadmapGraph",
    "Stage",
    "Status",
    "TraceGraph",
    "Verify",
];

/// The chunk's paint sweep: on every surface, the pointer is moved onto
/// every widget the hit registry registers, and each one either answers
/// — at least one painted cell differs from the un-hovered frame — or is
/// named in [`CHROME`]. The two sets together are everything registered,
/// so nothing can go quiet by omission.
#[test]
fn every_registered_widget_either_answers_the_pointer_or_is_named_chrome() {
    let size = Size::new(120, 40);
    let mut silent = Vec::new();
    let mut chrome = std::collections::BTreeSet::new();
    for (name, make) in SURFACES {
        let plain = buffer_of(&make(size));
        for id in registered(&make(size)) {
            if HoverRole::of(id) == HoverRole::Inert {
                let widget = Widget::from_id(id).expect("chrome is a named widget");
                chrome.insert(format!("{widget:?}"));
                continue;
            }
            if buffer_of(&Scenes::hovering(make, size, id)) == plain {
                silent.push(format!(
                    "{name}: {id:?} ({:?}) paints the same",
                    HoverRole::of(id)
                ));
            }
        }
    }
    assert!(
        silent.is_empty(),
        "{} hittables answer nothing:\n{}",
        silent.len(),
        silent.join("\n")
    );
    let expected: std::collections::BTreeSet<String> =
        CHROME.iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(chrome, expected, "the inert set moved");
}

/// Hover is paint, never layout: the resolved rects of a hovered frame
/// are the rects of the frame under it, widget for widget, on every
/// surface and for every hittable on it.
#[test]
fn hover_never_moves_a_single_rect() {
    let size = Size::new(120, 40);
    for (name, make) in SURFACES {
        let plain = placement(&layout(&view(&make(size), &clock()), size));
        for id in registered(&make(size)) {
            if HoverRole::of(id) == HoverRole::Inert {
                continue;
            }
            let model = Scenes::hovering(make, size, id);
            let hovered = placement(&layout(&view(&model, &clock()), size));
            assert_eq!(plain, hovered, "{name}: {id:?} moved the layout");
        }
    }
}

/// A hovered widget's cells still read: the raise re-floors its
/// foreground against the new ground and the lift is held to the floor,
/// so no hover puts a glyph under WCAG AA large text in either palette.
#[test]
fn no_hover_puts_a_glyph_under_the_contrast_floor() {
    use magistr_theme::{ThemeColor, math};
    let rgb = |color: Color| match color {
        Color::Rgb(r, g, b) => Some(ThemeColor::new(r, g, b)),
        Color::Default | Color::Indexed(_) => None,
    };
    let size = Size::new(120, 40);
    let mut failures = Vec::new();
    for (name, make) in SURFACES {
        for (theme_name, mode) in [("mocha", ThemeMode::Dark), ("latte", ThemeMode::Light)] {
            let themed: Surface = make;
            for id in registered(&themed(size)) {
                if HoverRole::of(id) == HoverRole::Inert {
                    continue;
                }
                let model = Scenes::hovering(themed, size, id).with_theme(Theme::forced(mode));
                for cell in buffer_of(&model).cells() {
                    let Some(ch) = cell.symbol.chars().next() else {
                        continue;
                    };
                    if ch.is_whitespace() || STRUCTURAL.contains(ch) {
                        continue;
                    }
                    let (Some(fg), Some(bg)) = (rgb(cell.style.fg), rgb(cell.style.bg)) else {
                        continue;
                    };
                    let ratio = math::contrast_ratio(fg, bg);
                    if ratio < Pen::MIN_CONTRAST {
                        failures.push(format!(
                            "{name} {theme_name} hovering {id:?}: {ch:?} at {ratio:.2}"
                        ));
                    }
                }
            }
        }
    }
    failures.sort();
    failures.dedup();
    assert!(
        failures.is_empty(),
        "{} hovered cells below the floor:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ── The pixel path's own stills ────────
//
// The cell stills above are what a cockpit on xterm or under tmux
// shows; these are what one on kitty, Ghostty or WezTerm shows, and
// they are rasterized by decoding the frame's own graphics APC rather
// than by drawing the surfaces a second time. Only the surfaces with a
// pixel branch are worth a second still: the launcher's brand mark and
// activity sparkline, the Roadmap tab's tech-tree and rail gauge, and
// the Trace tab's branch graph.

// ---------------------------------------------------------------------
// the hierarchy audit — as facts about the painted frame, not
// a grep over the view's source.
// ---------------------------------------------------------------------

/// A surface fixture at a size.
type Surface = fn(Size) -> Model;

/// The six surfaces the goldens cover, by name.
const SURFACES: [(&str, Surface); 10] = [
    ("launcher", Scenes::launcher),
    // The terminal pane is the one surface whose hues are a program's
    // own: swept here so every cell of it still pays
    // the floor, following and scrolled alike.
    ("terminal-pane", Scenes::terminal_pane),
    ("terminal-pane-scrolled", Scenes::terminal_pane_scrolled),
    ("streaming", Scenes::streaming),
    ("permission", Scenes::permission),
    ("roadmap", Scenes::roadmapped),
    ("changes", Scenes::changed),
    ("trace", Scenes::traced),
    // The two native readings of the Sky. Swept here with the rest so
    // the Brief's four tinted grounds and the murmur rows pay the same
    // contrast floor every other pen does — the bands are new colours,
    // and a new colour is exactly what a floor is for.
    ("brief", Scenes::briefed),
    ("rest", Scenes::resting),
];

/// Glyphs that are structure rather than text — see
/// [`Pen::STRUCTURAL`], which the native painter's own sweep reads too.
const STRUCTURAL: &str = Pen::STRUCTURAL;

/// Every text leaf on every surface sits under a style set by something
/// other than the screen itself: no text is left to the painter's
/// default pen, and none merely inherits the root's base — every
/// surface's chrome names the pen its text wears.
#[test]
fn every_text_leaf_on_every_surface_wears_a_named_pen() {
    fn walk(node: &Node, styled_below_root: bool, depth: usize, bare: &mut Vec<String>) {
        let styled = styled_below_root || (depth > 0 && node.cell_style().is_some());
        if let Some(text) = node.text_metrics()
            && !styled
            && !text.text().trim().is_empty()
        {
            bare.push(text.text().to_owned());
        }
        for child in node.child_nodes() {
            walk(child, styled, depth + 1, bare);
        }
    }
    for (name, make) in SURFACES {
        let model = make(Size::new(120, 40));
        let root = view(&model, &clock());
        let mut bare = Vec::new();
        walk(&root, false, 0, &mut bare);
        assert!(
            bare.is_empty(),
            "{name}: text on the root's pen only: {bare:?}"
        );
    }
}

/// Every painted glyph reads at WCAG AA large-text contrast against the
/// cell it sits on, in both palettes and under a CVD remapping —
/// structure (the ring, the bars) excepted — and nothing on screen is
/// the terminal's default colour.
#[test]
fn every_painted_glyph_reads_at_the_floor_in_every_palette_and_a_cvd_mode() {
    use magistr_theme::{CvdMode, ThemeColor, math};
    let rgb = |color: Color| match color {
        Color::Rgb(r, g, b) => Some(ThemeColor::new(r, g, b)),
        Color::Default | Color::Indexed(_) => None,
    };
    let themes = [
        ("mocha", Theme::forced(ThemeMode::Dark)),
        ("latte", Theme::forced(ThemeMode::Light)),
        (
            "mocha/deuteranopia",
            Theme::forced(ThemeMode::Dark).with_cvd(Some(CvdMode::Deuteranopia)),
        ),
        (
            "latte/tritanopia",
            Theme::forced(ThemeMode::Light).with_cvd(Some(CvdMode::Tritanopia)),
        ),
    ];
    let mut failures = Vec::new();
    for (surface, make) in SURFACES {
        for (theme_name, theme) in themes {
            let model = make(Size::new(120, 40)).with_theme(theme);
            let root = view(&model, &clock());
            let resolved = layout(&root, model.size());
            let mut buffer = Buffer::new(120, 40);
            paint(&root, &resolved, &mut buffer);
            for (index, cell) in buffer.cells().iter().enumerate() {
                let Some(ch) = cell.symbol.chars().next() else {
                    continue;
                };
                if ch.is_whitespace() || STRUCTURAL.contains(ch) {
                    continue;
                }
                let (col, row) = (index % 120, index / 120);
                let (Some(fg), Some(bg)) = (rgb(cell.style.fg), rgb(cell.style.bg)) else {
                    failures.push(format!(
                        "{surface} {theme_name} ({col},{row}) {ch:?}: default colour"
                    ));
                    continue;
                };
                let ratio = math::contrast_ratio(fg, bg);
                if ratio < Pen::MIN_CONTRAST {
                    failures.push(format!(
                        "{surface} {theme_name} ({col},{row}) {ch:?}: {ratio:.2} ({fg} on {bg})"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} cells below the floor:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The Nerd vocabulary reaches the screen end to end: the brand mark,
/// the tool head's status glyph and the prompt's mark are the Nerd
/// codepoints when the theme asks for them — and the Unicode ones
/// otherwise, which is what every golden is rasterized from.
#[test]
fn the_icon_mode_decides_every_glyph_on_screen() {
    use crate::theme::{IconMode, Icons};
    for (mode, icons) in [
        (IconMode::Nerd, &Icons::NERD),
        (IconMode::Unicode, &Icons::UNICODE),
    ] {
        // Reduced motion, so the running tool wears its running mark
        // rather than the (shared) spinner's frame.
        let model = tool_calls(Size::new(120, 40)).with_theme(
            Theme::forced(ThemeMode::Dark)
                .with_icons(mode)
                .with_motion(crate::theme::MotionMode::Reduced),
        );
        let screen = screen(&model);
        for (name, glyph) in [
            ("brand", icons.brand),
            ("pending", icons.pending),
            ("running", icons.running),
            ("done", icons.done),
            ("failed", icons.failed),
            ("prompt", icons.prompt),
            ("active", icons.active),
        ] {
            assert!(
                screen.contains(glyph),
                "{mode:?}: {name} {glyph:?} not on screen"
            );
        }
        let other = if mode == IconMode::Nerd {
            &Icons::UNICODE
        } else {
            &Icons::NERD
        };
        assert!(
            !screen.contains(other.brand),
            "{mode:?}: the other set's brand is on screen"
        );
    }
}

// ---------------------------------------------------------------------
// the motion layer — every animation a pure function of
// Model::ticks, frozen to its final frame under reduced motion.
// ---------------------------------------------------------------------

/// The painted cells of `model`'s frame: symbol and style, in order.
fn painted(model: &Model) -> Vec<(String, Style)> {
    let root = view(model, &clock());
    let resolved = layout(&root, model.size());
    let mut buffer = Buffer::new(model.size().cols, model.size().rows);
    paint(&root, &resolved, &mut buffer);
    buffer
        .cells()
        .iter()
        .map(|cell| (cell.symbol.to_string(), cell.style.clone()))
        .collect()
}

/// The column of the gutter bar on screen row `row`, if it has one.
fn gutter_col(model: &Model, row: usize) -> Option<u16> {
    let line = screen(model).lines().nth(row)?.to_owned();
    line.chars()
        .position(|ch| ch == '▎')
        .and_then(|at| u16::try_from(at).ok())
}

/// The column `needle` starts at on screen row `row` — counted in
/// cells, not bytes, since the chrome is full of multi-byte glyphs.
fn col_of(model: &Model, row: u16, needle: &str) -> u16 {
    let screen = screen(model);
    let line = screen
        .lines()
        .nth(usize::from(row))
        .unwrap_or_else(|| panic!("no row {row} on screen:\n{screen}"));
    let byte = line
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not on row {row}: {line:?}"));
    u16::try_from(line[..byte].chars().count()).expect("cols fit u16")
}

/// The screen row whose text contains `needle`.
fn row_of(model: &Model, needle: &str) -> u16 {
    let screen = screen(model);
    let row = screen
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} not on screen:\n{screen}"));
    u16::try_from(row).expect("rows fit u16")
}

/// The streaming block's gutter carries the shimmer — its cells move
/// with the tick and vary down the block — while a settled block's
/// gutter above it stands still.
#[test]
fn the_streaming_block_shimmers_on_its_gutter_and_only_there() {
    let mut m = Scenes::streaming(Size::new(120, 40));
    assert!(m.is_animating());
    let pen = Pen::for_theme(&m.theme());
    let tail = row_of(&m, "Still streaming");
    // The row above the tail: one step apart on the wave, so the two
    // never share a phase (the wave is symmetric, the offsets are not).
    let head = tail - 1;
    let thought = row_of(&m, "I should read the roadmap");
    let col = gutter_col(&m, usize::from(tail)).expect("the tail row has a gutter");
    assert_eq!(gutter_col(&m, usize::from(thought)), Some(col));

    let frames: Vec<(Style, Style, Style)> = (0..4)
        .map(|_| {
            let frame = (
                cell_style(&m, col, tail),
                cell_style(&m, col, head),
                cell_style(&m, col, thought),
            );
            Scenes::ticks(&mut m, 3);
            frame
        })
        .collect();
    assert!(
        frames.windows(2).any(|w| w[0].0 != w[1].0),
        "the tail row's gutter moves with the tick"
    );
    assert!(
        frames.iter().any(|f| f.0 != f.1),
        "the wave varies down the block"
    );
    assert!(
        frames.iter().all(|f| f.2 == pen.accent_thought),
        "the settled thought block above is still"
    );
    for frame in &frames {
        assert_eq!(
            frame.0.bg, pen.accent_agent.bg,
            "only the bar's colour moves"
        );
        assert_eq!(frame.0.attrs, pen.accent_agent.attrs);
    }
}

/// A tab switch pulses the tab switched to, once, and leaves it exactly
/// the active pen after; the other tabs never move.
#[test]
fn a_tab_switch_pulses_the_active_tab_then_settles() {
    use crate::input::KeyCode;
    let mut m = model();
    let pen = Pen::for_theme(&m.theme());
    update(&mut m, Msg::Key(crate::input::KeyEvent::new(KeyCode::Tab)));
    assert_eq!(m.tab(), StageTab::Changes);
    let row = row_of(&m, "Changes");
    let line = screen(&m)
        .lines()
        .nth(usize::from(row))
        .expect("row")
        .to_owned();
    let byte = line.find("Changes").expect("the tab label");
    let col = u16::try_from(line[..byte].chars().count()).expect("cols fit");
    assert_eq!(
        cell_style(&m, col, row),
        pen.tab_active,
        "at the first step the pulse is at rest"
    );
    Scenes::ticks(&mut m, Motion::TRANSITION_STEPS / 2);
    let crest = cell_style(&m, col, row);
    assert_ne!(crest, pen.tab_active, "midway the tab is lifted");
    assert_eq!(crest.attrs, pen.tab_active.attrs);
    Scenes::ticks(&mut m, Motion::TRANSITION_STEPS);
    assert_eq!(cell_style(&m, col, row), pen.tab_active, "then settles");
    assert!(!m.is_animating(), "and the tick stops");
}

/// A card's arrival pulses the front card's text and flashes the ring,
/// both at rest on the first and the last step.
#[test]
fn a_dock_arrival_pulses_the_card_and_settles() {
    let mut m = Scenes::permission(Size::new(120, 40));
    let pen = Pen::for_theme(&m.theme());
    assert_eq!(m.motion().dock_transition((m.ticks()) as f32), Some(0.0));
    let row = row_of(&m, "Bash needs permission");
    let line = screen(&m)
        .lines()
        .nth(usize::from(row))
        .expect("row")
        .to_owned();
    let col =
        u16::try_from(line.chars().position(|ch| ch == '⚠').expect("the mark")).expect("cols fit");
    assert_eq!(
        cell_style(&m, col, row),
        pen.permission,
        "at rest on arrival"
    );
    Scenes::ticks(&mut m, Motion::TRANSITION_STEPS / 2);
    assert_ne!(cell_style(&m, col, row), pen.permission, "lifted midway");
    Scenes::ticks(&mut m, Motion::TRANSITION_STEPS);
    assert_eq!(cell_style(&m, col, row), pen.permission, "settled");
}

// ---------------------------------------------------------------------
// arrival and finish feel — what appears fades in, what
// finishes pops once, and a settled frame is the frame it always was.
// ---------------------------------------------------------------------

/// A turn row that just arrived paints from the dim pen and reaches its
/// own pen three ticks later; the rows that were already there never
/// move, and the settled stage is byte-identical to the stage the same
/// model draws with motion off.
#[test]
fn arriving_rows_fade_in_from_the_dim_pen_and_settle_to_the_frame_they_always_were() {
    use crate::theme::MotionMode;
    let mut m = tool_calls(Size::new(120, 40));
    Scenes::settle(&mut m);
    let settled = painted(&m);
    let dim = Pen::for_theme(&m.theme()).note.fg;
    // A row arriving on a settled stage.
    let quiet = row_of(&m, "Found it.");
    agent(&mut m, AgentEvent::Text("\nOne more line.".to_owned()));
    let row = row_of(&m, "One more line.");
    let col = col_of(&m, row, "One");
    assert_eq!(cell_style(&m, col, row).fg, dim, "the dim end of the fade");
    let before = cell_style(&m, 4, quiet);
    Scenes::ticks(&mut m, 1);
    let midway = cell_style(&m, col, row).fg;
    assert_ne!(midway, dim, "moved off the dim end");
    assert_eq!(
        cell_style(&m, 4, quiet),
        before,
        "a row that was already there does not fade"
    );
    Scenes::ticks(&mut m, Motion::ARRIVAL_STEPS);
    assert_eq!(
        cell_style(&m, col, row).fg,
        Pen::for_theme(&m.theme()).stage.fg,
        "settled on its own pen"
    );
    assert_eq!(
        painted(&m).len(),
        settled.len(),
        "the same frame, cell for cell"
    );

    // And the whole settled stage is the un-animated stage. Measured on
    // a turn that has ENDED: while one streams the spinner turns, and
    // reduced motion stands that down too, which is a different promise
    // from this one.
    let still = awaiting(Size::new(120, 40))
        .with_theme(Theme::forced(ThemeMode::Dark).with_motion(MotionMode::Reduced));
    let mut moving = awaiting(Size::new(120, 40)).with_theme(Theme::forced(ThemeMode::Dark));
    assert_ne!(
        painted(&moving),
        painted(&still),
        "the rows are still arriving"
    );
    Scenes::settle(&mut moving);
    assert_eq!(
        painted(&moving),
        painted(&still),
        "a settled turn is the turn reduced motion draws at once"
    );
}

/// A gate that settles flashes once on its own pen — the pass bright,
/// the failure in the error colour — and both are back at rest at the
/// end of the wave. The plan task that completes and the roadmap counter
/// a completed chunk moves do the same.
#[test]
fn a_settled_gate_a_completed_task_and_a_completed_chunk_each_pop_once() {
    let mut m = Scenes::gates_popping();
    let pen = Pen::for_theme(&m.theme());
    let strip = row_of(&m, "lint");
    let (pass, fail) = (col_of(&m, strip, "lint"), col_of(&m, strip, "fmt"));
    assert_eq!(cell_style(&m, pass, strip), pen.gate_pass, "at rest");
    assert_eq!(cell_style(&m, fail, strip), pen.gate_fail, "at rest");
    Scenes::ticks(&mut m, Motion::TRANSITION_STEPS / 2);
    let lifted_pass = cell_style(&m, pass, strip);
    let lifted_fail = cell_style(&m, fail, strip);
    assert_ne!(lifted_pass.fg, pen.gate_pass.fg, "the pass pops bright");
    assert_ne!(lifted_fail.fg, pen.gate_fail.fg, "the failure flashes");
    assert_eq!(lifted_fail.bg, pen.gate_fail.bg, "still the error pen");
    Scenes::ticks(&mut m, Motion::TRANSITION_STEPS);
    assert_eq!(cell_style(&m, pass, strip), pen.gate_pass, "settled");
    assert_eq!(cell_style(&m, fail, strip), pen.gate_fail, "settled");

    // A plan task completing pops the rail row it sits on. The plan is
    // built here rather than taken from the fixture, so the row can be
    // found by its title.
    let ship = |m: &mut Model, kind: &str, payload: serde_json::Value| {
        update(
            m,
            Msg::Engine(crate::engine::EngineEvent::local(
                crate::harness::fixtures::frame("ship", kind, payload),
            )),
        );
    };
    for id in ["wire the pens", "wire the view"] {
        ship(
            &mut m,
            "task_added",
            serde_json::json!({"task_id": id, "title": id}),
        );
    }
    Scenes::settle(&mut m);
    let neighbour = row_of(&m, "wire the pens");
    let quiet_neighbour = cell_style(&m, 4, neighbour);
    ship(
        &mut m,
        "task_completed",
        serde_json::json!({"task_id": "wire the view"}),
    );
    let task = row_of(&m, "wire the view");
    let rest = cell_style(&m, 4, task);
    Scenes::ticks(&mut m, Motion::TRANSITION_STEPS / 2);
    let popped = cell_style(&m, 4, task);
    assert_ne!(popped, rest, "the completed task pops");
    assert_eq!(
        cell_style(&m, 4, neighbour),
        quiet_neighbour,
        "its neighbour stays put"
    );
    assert!(
        popped.attrs.contains(crate::render::Attrs::BOLD),
        "raised and bold — the checklist pen has no weight of its own"
    );
    Scenes::ticks(&mut m, Motion::TRANSITION_STEPS);
    assert_eq!(cell_style(&m, 4, task), rest, "settled");

    // And the counter a completed chunk moves pops the same way. The
    // rail's line is the one carrying the ratio, not the loop note on
    // the stage that also says "done".
    update(
        &mut m,
        Msg::Engine(crate::engine::EngineEvent::Seed(
            magistr_engine::NativeSnapshot {
                progress: crate::harness::fixtures::progress(3, 10),
                ..magistr_engine::NativeSnapshot::default()
            },
        )),
    );
    Scenes::settle(&mut m);
    let counter = row_of(&m, "3/10 done");
    let quiet = cell_style(&m, 4, counter);
    update(
        &mut m,
        Msg::Engine(crate::engine::EngineEvent::local(
            crate::harness::fixtures::frame(
                "roadmap",
                "chunk_completed",
                serde_json::json!({"chunk": {"id": "x"}}),
            ),
        )),
    );
    Scenes::ticks(&mut m, Motion::TRANSITION_STEPS / 2);
    assert_ne!(cell_style(&m, 4, counter), quiet, "the counter pops");
    Scenes::ticks(&mut m, Motion::TRANSITION_STEPS);
    assert_eq!(cell_style(&m, 4, counter), quiet, "settled");
    // Nothing keeps ticking once every pop — and the longer glide the
    // same completion started — is over.
    Scenes::ticks(&mut m, Eased::STEPS);
    assert!(!m.motion().is_settling((m.ticks()) as f32));
}

/// The rail's bar fills to a new value over the glide rather than
/// jumping, and reads the settled value at the end.
#[test]
fn the_progress_bar_eases_to_its_new_value() {
    let mut m = model();
    update(
        &mut m,
        Msg::Engine(crate::engine::EngineEvent::Seed(
            magistr_engine::NativeSnapshot {
                progress: crate::harness::fixtures::progress(2, 8),
                ..magistr_engine::NativeSnapshot::default()
            },
        )),
    );
    let bar = |m: &Model| {
        let mut lines = Vec::new();
        texts(&view(m, &clock()), &mut lines);
        lines
            .into_iter()
            .find(|line| line.starts_with('▰') || line.starts_with('▱'))
            .expect("the bar")
    };
    let fill = |bar: &str| bar.chars().filter(|ch| *ch == '▰').count();
    let before = bar(&m);
    assert!(before.ends_with(" 2/8"), "{before}");
    for _ in 0..4 {
        update(
            &mut m,
            Msg::Engine(crate::engine::EngineEvent::local(
                crate::harness::fixtures::frame(
                    "roadmap",
                    "chunk_completed",
                    serde_json::json!({"chunk": {"id": "x"}}),
                ),
            )),
        );
    }
    let start = bar(&m);
    assert!(
        start.ends_with(" 6/8"),
        "the label is settled at once: {start}"
    );
    assert_eq!(fill(&start), fill(&before), "the fill starts where it was");
    assert!(m.is_animating());
    Scenes::ticks(&mut m, 2);
    let midway = fill(&bar(&m));
    assert!(
        midway > fill(&before),
        "midway the fill has grown: {midway}"
    );
    Scenes::ticks(&mut m, Eased::STEPS);
    let settled = bar(&m);
    assert!(fill(&settled) > midway, "and lands: {settled}");
    assert_eq!(
        fill(&settled),
        fill(&progress_bar(
            0.75,
            usize::from(rail_gauge_cols(&m)),
            &Icons::UNICODE
        )),
        "at the integer fill"
    );
    // A completion also opens the Sky's ship moment, which outlasts
    // the fill's glide — the tick stops when the last of them does.
    Scenes::ticks(&mut m, ShipRun::STEPS);
    assert!(!m.is_animating());
}

/// Under reduced motion every surface is its final frame: the tick
/// never runs, and even when steps are forced through nothing on screen
/// moves — while with motion on, the streaming surface does.
#[test]
fn reduced_motion_freezes_every_surface_to_its_final_frame() {
    use crate::theme::MotionMode;
    let reduced = Theme::forced(ThemeMode::Dark).with_motion(MotionMode::Reduced);
    for (name, make) in SURFACES {
        let mut m = make(Size::new(120, 40)).with_theme(reduced);
        assert!(!m.is_animating(), "{name}: nothing animates");
        let first = painted(&m);
        Scenes::ticks(&mut m, Eased::STEPS + Motion::TRANSITION_STEPS);
        let moved: Vec<_> = painted(&m)
            .into_iter()
            .zip(&first)
            .enumerate()
            .filter(|(_, (after, before))| after != *before)
            .map(|(at, (after, _))| (at % 120, at / 120, after.0))
            .take(5)
            .collect();
        assert!(
            moved.is_empty(),
            "{name}: a forced step moves nothing, but {moved:?} moved"
        );
        assert_eq!(m.motion().tab_transition((m.ticks()) as f32), None);
        assert_eq!(m.motion().dock_transition((m.ticks()) as f32), None);
    }
    let mut moving =
        Scenes::streaming(Size::new(120, 40)).with_theme(Theme::forced(ThemeMode::Dark));
    let first = painted(&moving);
    Scenes::ticks(&mut moving, 3);
    assert_ne!(painted(&moving), first, "with motion on, the frame moves");
}

// ---------------------------------------------------------------------
// motion goldens — every animation as a filmstrip a person can
// look at, rather than a `assert_ne!` on a Pen colour.
// ---------------------------------------------------------------------

/// , the cell half: the chunk in progress breathes on the same
/// raised cosine the pixel halo swells on — and comes back to exactly
/// its own pen at the foot of the wave, which is what leaves every
/// settled still where it was.
#[test]
fn the_chunk_in_progress_breathes_on_the_cell_path_and_settles_on_its_own_pen() {
    let clock = clock();
    let pens = |m: &Model| {
        let mut out = Vec::new();
        styles(&view(m, &clock), &mut out);
        out
    };
    let mut m = Scenes::roadmapped(Size::new(80, 24));
    let foot = pens(&m);
    Scenes::ticks(&mut m, Motion::GLOW_STEPS / 2);
    assert_ne!(pens(&m), foot, "the crest of the breath must read");
    Scenes::ticks(&mut m, Motion::GLOW_STEPS / 2);
    assert_eq!(
        pens(&m),
        foot,
        "and a whole period later it is itself again"
    );
}

/// The same surface under reduced motion never breathes at all: the tick
/// never runs, and `in_motion` hands the view no phase to read.
#[test]
fn the_chunk_in_progress_holds_still_under_reduced_motion() {
    use crate::theme::MotionMode;
    let clock = clock();
    let pens = |m: &Model| {
        let mut out = Vec::new();
        styles(&view(m, &clock), &mut out);
        out
    };
    let mut m = Scenes::roadmapped(Size::new(80, 24))
        .with_theme(Theme::default().with_motion(MotionMode::Reduced));
    let foot = pens(&m);
    Scenes::ticks(&mut m, Motion::GLOW_STEPS / 2);
    assert_eq!(pens(&m), foot, "reduced motion is structural, not per-test");
}

/// the three chrome slots the pixel layer covers. Each keeps
/// its width on both paths — so nothing on the bar or in the rail moves
/// when the startup probe answers — and each stands its cell drawing
/// down for the surface only when the probe found a protocol.
mod pixel_chrome {
    use super::*;

    /// The same model with the pixel layer switched on.
    fn with_pixels(mut m: Model) -> Model {
        use crate::capability::Mode;
        update(
            &mut m,
            Msg::Mode(Mode::Graphics {
                pointer_shapes: true,
            }),
        );
        m
    }

    /// A model that has painted two busy frames, so the header's
    /// activity monitor has news to show — settled, so its arrival
    /// sweep is over and the ramp is whole.
    fn busy() -> Model {
        let mut m = unsettled_busy();
        Scenes::settle(&mut m);
        m
    }

    /// The same two busy frames, on the very step the ribbon grew — so
    /// the monitor is at the foot of its sweep with nothing drawn yet.
    fn unsettled_busy() -> Model {
        let mut m = model();
        for n in 1..=2 {
            for _ in 0..n {
                update(&mut m, Msg::Agent(AgentEvent::Text("chunk".to_owned())));
            }
            update(&mut m, Msg::FrameDue);
        }
        m
    }

    /// The row of `screen` holding `needle`, trimmed of its right pad.
    fn row_with(model: &Model, needle: &str) -> String {
        screen(model)
            .lines()
            .find(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no row holds {needle:?}"))
            .trim_end()
            .to_owned()
    }

    /// The text the leaf carrying `widget` draws.
    fn slot(model: &Model, widget: Widget) -> String {
        fn walk(node: &Node, id: crate::widget_id::WidgetId) -> Option<String> {
            if node.widget_id() == Some(id) {
                return node.text_metrics().map(|t| t.text().to_owned());
            }
            node.child_nodes().iter().find_map(|child| walk(child, id))
        }
        walk(&view(model, &clock()), widget.id())
            .unwrap_or_else(|| panic!("{widget:?} draws no text"))
    }

    #[test]
    fn the_brand_glyph_gives_its_cell_up_to_the_pixel_mark() {
        let cells = row_with(&model(), "magistr");
        assert!(
            cells.starts_with(&format!("{} magistr", Icons::UNICODE.brand)),
            "the glyph leads the bar in cells: {cells:?}"
        );
        let pixels = row_with(&with_pixels(model()), "magistr");
        assert!(
            pixels.starts_with("  magistr"),
            "and the cell is blank for the mark: {pixels:?}"
        );
        assert_eq!(
            cells.chars().count(),
            pixels.chars().count(),
            "the slot is the same width either way"
        );
    }

    /// the monitor draws itself in from the
    /// left rather than appearing whole. The cell ramp cuts between
    /// whole columns on the same curve the pixel line sweeps on, the
    /// slot never changes width, and the last frame is exactly the
    /// string a settled header always drew — which is what leaves every
    /// PNG golden where it is.
    #[test]
    fn the_activity_monitor_draws_itself_in_from_the_left_and_settles_whole() {
        let mut m = unsettled_busy();
        let width = usize::from(SPARK_COLS);
        let ramp = |m: &Model| slot(m, Widget::HeaderSpark);

        let first = ramp(&m);
        assert_eq!(first.chars().count(), width, "the slot keeps its width");
        assert_eq!(
            first.trim(),
            "",
            "nothing is drawn at the foot of the sweep"
        );

        // The drawn columns only ever grow, and always from the left.
        let mut drawn = 0;
        for _ in 0..Motion::SWEEP_STEPS {
            Scenes::ticks(&mut m, 1);
            let row = ramp(&m);
            assert_eq!(row.chars().count(), width, "the slot keeps its width");
            let now = row.trim_end().chars().count();
            assert!(now >= drawn, "the sweep never goes backwards: {row:?}");
            assert!(
                !row.trim_end().contains(' '),
                "it draws in from the left, with no gap: {row:?}"
            );
            drawn = now;
        }
        assert_eq!(
            ramp(&m),
            ramp(&busy()),
            "the sweep ends on the frame the tick never touched"
        );
    }

    /// Under reduced motion there is no sweep at all: the monitor is its
    /// final frame on the frame it arrives.
    #[test]
    fn reduced_motion_shows_the_activity_monitor_whole_at_once() {
        let reduced = Theme::forced(ThemeMode::Dark).with_motion(crate::theme::MotionMode::Reduced);
        let mut m = model().with_theme(reduced);
        for n in 1..=2 {
            for _ in 0..n {
                update(&mut m, Msg::Agent(AgentEvent::Text("chunk".to_owned())));
            }
            update(&mut m, Msg::FrameDue);
        }
        assert_eq!(
            slot(&m, Widget::HeaderSpark),
            slot(&busy(), Widget::HeaderSpark),
            "no tick, no sweep — the ramp is whole on arrival"
        );
    }

    #[test]
    fn the_activity_monitor_arrives_with_its_news_and_then_swaps_to_pixels() {
        // Nothing has happened: no monitor, and the header is complete.
        let idle = view(&model(), &clock());
        let mut lines = Vec::new();
        texts(&idle, &mut lines);
        assert!(
            !lines
                .iter()
                .any(|line| line.contains(Icons::UNICODE.spark[0])),
            "an idle session has no activity to monitor: {lines:?}"
        );
        // Two busy frames later the ramp is on the bar, its tallest cell
        // on the busier frame.
        let m = busy();
        let cells = row_with(&m, "magistr");
        let ramp: String = cells
            .chars()
            .filter(|c| Icons::SPARK.contains(&c.to_string().as_str()))
            .collect();
        assert_eq!(
            ramp,
            format!("{}{}", Icons::SPARK[0].repeat(6), "▅█"),
            "the quiet frames before the session pad the left, then one \
             event and two: half the ramp, then its top"
        );
        // With pixels the same slot is blank for the sparkline.
        let pixels = row_with(&with_pixels(busy()), "magistr");
        assert!(
            !pixels
                .chars()
                .any(|c| Icons::SPARK.contains(&c.to_string().as_str())),
            "the cells stand down for the surface: {pixels:?}"
        );
        assert_eq!(cells.chars().count(), pixels.chars().count());
    }

    /// the monitor plots what the session spends.
    /// Until the agent reports usage that is the busy-frame event count;
    /// once it does, the same eight cells carry tokens-per-turn. Both
    /// ramps scale to their own peak, so the swap shows as a different
    /// *shape* in a slot that never changes width.
    #[test]
    fn the_monitor_swaps_the_event_count_for_tokens_once_the_agent_reports_usage() {
        // Three busy frames of one event each: the drawn tail is flat,
        // because the series is.
        let mut m = model();
        for _ in 0..3 {
            update(&mut m, Msg::Agent(AgentEvent::Text("chunk".to_owned())));
            update(&mut m, Msg::FrameDue);
        }
        Scenes::settle(&mut m);
        let events = slot(&m, Widget::HeaderSpark);
        let tail = |ramp: &str, n: usize| {
            ramp.chars()
                .skip(usize::from(SPARK_COLS) - n)
                .map(|c| c.to_string())
                .collect::<Vec<String>>()
        };
        assert_eq!(
            tail(&events, 3),
            vec![Icons::SPARK[7]; 3],
            "one event a frame is three cells of the same height: {events:?}"
        );

        // The same session, now with the agent's usage behind two turns:
        // a small one, then a big one. `used` is the context *level*, so
        // the samples are 2k and 38k — and 2k against that peak is the
        // bottom of the ramp.
        let turn = |m: &mut Model, used: u64| {
            update(
                m,
                Msg::Agent(AgentEvent::Usage {
                    used,
                    size: 200_000,
                    cost_cents: None,
                    elapsed_secs: None,
                }),
            );
            update(m, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
        };
        turn(&mut m, 2_000);
        turn(&mut m, 40_000);
        Scenes::settle(&mut m);
        let tokens = slot(&m, Widget::HeaderSpark);
        assert_eq!(
            tokens.chars().count(),
            events.chars().count(),
            "the slot is the same width either way"
        );
        assert_eq!(
            tail(&tokens, 2),
            vec![Icons::SPARK[0].to_owned(), Icons::SPARK[7].to_owned()],
            "the tokens took the slot over from the event count: {tokens:?}"
        );
    }

    #[test]
    fn the_rail_gauge_swaps_its_ramp_for_the_surface_and_keeps_the_label() {
        let mut m = model();
        update(
            &mut m,
            Msg::Engine(crate::engine::EngineEvent::Seed(
                magistr_engine::NativeSnapshot {
                    progress: crate::harness::fixtures::progress(1, 4),
                    ..magistr_engine::NativeSnapshot::default()
                },
            )),
        );
        let cells = slot(&m, Widget::RailGauge);
        assert!(
            cells.contains(Icons::UNICODE.bar_full) && cells.contains(Icons::UNICODE.bar_empty),
            "the ▰▱ ramp draws the fill: {cells:?}"
        );
        assert!(cells.ends_with(" 1/4"), "with its label: {cells:?}");
        let pixels = slot(&with_pixels(m.clone()), Widget::RailGauge);
        assert!(
            !pixels.contains(Icons::UNICODE.bar_full) && !pixels.contains(Icons::UNICODE.bar_empty),
            "the bar's cells are blank for the gauge: {pixels:?}"
        );
        assert!(pixels.ends_with(" 1/4"), "the label stays text: {pixels:?}");
        assert_eq!(
            cells.chars().count(),
            pixels.chars().count(),
            "the row is the same width either way"
        );
        // And the driver's slice of that row covers the bar, not the
        // label: the four cells of " 1/4" are left to the text.
        let row = layout(&view(&m, &clock()), m.size())
            .placed()
            .iter()
            .find_map(|p| (p.id == Some(Widget::RailGauge.id())).then_some(p.rect))
            .expect("the gauge row is laid out");
        assert_eq!(rail_gauge_cols(&m), row.width - 4);
    }
}

/// A 80x24 Roadmap tab holding more chunks than the stage fits.
fn long_roadmap(size: Size) -> Model {
    let mut m = Model::new(size).with_tab(StageTab::Roadmap);
    let chunks: Vec<serde_json::Value> = (0..110)
        .map(|i| {
            serde_json::json!({
                "id": format!("c{i:03}"), "name": format!("chunk-{i:03}"),
                "title": format!("the {i}th claim"), "status": "pending",
                "priority": 100 + i, "group": "TUI rewrite",
            })
        })
        .collect();
    update(
        &mut m,
        Msg::Engine(crate::engine::EngineEvent::Seed(
            magistr_engine::NativeSnapshot {
                chunks: crate::harness::fixtures::chunks_from(chunks),
                ..magistr_engine::NativeSnapshot::default()
            },
        )),
    );
    m
}

#[test]
fn the_roadmap_body_scrolls_under_its_pinned_heading_and_tech_tree() {
    let mut m = long_roadmap(Size::new(80, 24));
    let head = screen(&m);
    assert!(head.contains("roadmap   0 done"), "the heading: {head}");
    assert!(head.contains("«TUI rewrite» 0/110"), "the legend: {head}");
    assert!(head.contains("chunk-000"), "the head of the list: {head}");
    assert!(!head.contains("chunk-109"), "and not its tail: {head}");

    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::End)));
    Scenes::settle(&mut m);
    let tail = screen(&m);
    assert!(
        tail.contains("roadmap   0 done") && tail.contains("«TUI rewrite» 0/110"),
        "the heading and the tech-tree stayed pinned: {tail}"
    );
    assert!(tail.contains("chunk-109"), "the tail of the list: {tail}");
    assert!(!tail.contains("chunk-000"), "the head scrolled off: {tail}");
}

#[test]
fn a_detail_card_opened_at_the_far_end_paints_once_the_window_reaches_it() {
    let mut m = long_roadmap(Size::new(80, 24));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 20),
            target: Some(Widget::roadmap_row(109)),
        },
    );
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::End)));
    Scenes::settle(&mut m);
    let screen = screen(&m);
    assert!(screen.contains("▾ chunk-109"), "the card is open: {screen}");
    assert!(screen.contains("the 109th claim"), "and painted: {screen}");
}

/// A model whose turn holds one image: a 40×40 PNG, red on the left and
/// blue on the right, which is 4×2 cells at the default 10×20 cell.
fn pictured(size: Size) -> Model {
    let mut pixmap = tiny_skia::Pixmap::new(40, 40).expect("a pixmap allocates");
    for (index, pixel) in pixmap.pixels_mut().iter_mut().enumerate() {
        let color = if index % 40 < 20 {
            tiny_skia::PremultipliedColorU8::from_rgba(255, 0, 0, 255)
        } else {
            tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 255, 255)
        };
        *pixel = color.expect("opaque colour is premultiplied");
    }
    let png = pixmap.encode_png().expect("a pixmap encodes");
    let mut data = Vec::new();
    crate::base64::encode(&png, &mut data);
    let mut m = Model::new(size);
    agent(&mut m, AgentEvent::Text("here it is:\n".to_owned()));
    agent(
        &mut m,
        AgentEvent::Image(ImageBlock {
            mime_type: "image/png".to_owned(),
            data: String::from_utf8(data).expect("base64 is ascii"),
            uri: None,
        }),
    );
    m
}

/// The painted cells of the stage rows that hold the image, as
/// `(symbol, fg, bg)`.
fn image_cells(model: &Model) -> Vec<(String, Color, Color)> {
    let size = model.size();
    let root = view(model, &clock());
    let resolved = layout(&root, size);
    let mut buffer = Buffer::new(size.cols, size.rows);
    paint(&root, &resolved, &mut buffer);
    let mut rows = Vec::new();
    for index in 0..model.visible_rows().len() {
        if model.visible_rows()[index].kind != RowKind::Image {
            continue;
        }
        let Some(rect) = resolved
            .placed()
            .iter()
            .find(|p| p.id == Some(Widget::stage_row(index)))
            .map(|p| p.rect)
        else {
            continue;
        };
        for col in 0..4 {
            let cell = buffer
                .get(rect.x + Accent::GUTTER_COLS + col, rect.y)
                .expect("a painted cell");
            rows.push((cell.symbol.to_string(), cell.style.fg, cell.style.bg));
        }
    }
    rows
}

#[test]
fn without_graphics_an_image_is_drawn_in_halfblocks() {
    let m = pictured(Size::new(80, 24));
    assert!(!m.has_pixels(), "the probe found no graphics protocol");
    let cells = image_cells(&m);
    assert_eq!(cells.len(), 8, "four columns over two rows");
    for (symbol, _, _) in &cells {
        assert_eq!(symbol, "▀", "each cell is two image rows");
    }
    assert_eq!(cells[0].1, Color::Rgb(255, 0, 0), "the left is red");
    assert_eq!(cells[0].2, Color::Rgb(255, 0, 0));
    assert_eq!(cells[3].1, Color::Rgb(0, 0, 255), "the right is blue");
    assert_eq!(cells[3].2, Color::Rgb(0, 0, 255));
}

#[test]
fn with_graphics_the_image_rows_are_left_blank_for_the_surface() {
    let mut m = pictured(Size::new(80, 24));
    update(
        &mut m,
        Msg::Mode(crate::capability::Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    assert!(m.has_pixels());
    let cells = image_cells(&m);
    assert_eq!(cells.len(), 8, "the same eight cells, the same width");
    for (symbol, _, _) in &cells {
        assert_eq!(symbol, " ", "the driver composites over them");
    }
}

#[test]
fn the_stage_shows_an_image_block() {
    insta::assert_snapshot!(screen(&pictured(Size::new(80, 24))));
}

/// A Changes tab with a journal far longer than the stage.
fn long_changes(size: Size) -> Model {
    let mut m = Model::new(size);
    m.tab = StageTab::Changes;
    let entries: Vec<crate::workbench::JournalEntry> = (0..120)
        .map(|n| crate::workbench::JournalEntry {
            id: format!("e{n}"),
            path: format!("src/file-{n:03}.rs"),
            added: 1,
            removed: 1,
            parse_ok: true,
            diff: "@@ -1 +1 @@\n-old\n+new".to_owned(),
        })
        .collect();
    update(&mut m, Msg::Workbench(WorkbenchEvent::Journal { entries }));
    m
}

#[test]
fn the_changes_body_count_is_the_rows_the_tab_paints() {
    // The window the keys move and the rows the view paints are the same
    // list, so the count the model keeps must be the view's own.
    let mut m = long_changes(Size::new(80, 24));
    let pen = Pen::for_theme(&m.theme());
    assert_eq!(changes_body(&m, &pen).len(), m.changes().body_rows());
    m.changes.toggle(0);
    assert_eq!(changes_body(&m, &pen).len(), m.changes().body_rows());
    m.changes.set_runs(
        "test",
        vec![crate::workbench::RunRow {
            name: "a::fails".to_owned(),
            passed: false,
            detail: String::new(),
        }],
    );
    assert_eq!(changes_body(&m, &pen).len(), m.changes().body_rows());
    m.changes.open_viewer("src/lib.rs", vec!["a".to_owned()]);
    assert_eq!(changes_body(&m, &pen).len(), m.changes().body_rows());
}

#[test]
fn a_long_changes_list_scrolls_instead_of_clipping() {
    let mut m = long_changes(Size::new(80, 24));
    let head = screen(&m);
    assert!(head.contains("src/file-000.rs"), "the head: {head}");
    assert!(!head.contains("src/file-119.rs"), "not its tail: {head}");

    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::End)));
    Scenes::settle(&mut m);
    let tail = screen(&m);
    assert!(tail.contains("src/file-119.rs"), "the tail: {tail}");
    assert!(
        !tail.contains("src/file-000.rs"),
        "the head scrolled off: {tail}"
    );
}

/// The pinned block and the windowed body together are exactly the rows
/// the tab paints, so the window the keys move can never measure a list
/// the view is not drawing.
#[test]
fn the_changes_pinned_and_body_counts_are_the_rows_the_tab_paints() {
    let pen = Pen::for_theme(&Model::new(Size::new(80, 24)).theme());
    // Tall enough that the whole body fits: painted == pinned + body.
    let mut m = long_changes(Size::new(80, 200));
    let fits = |m: &Model| {
        let (total, height) = m.changes_window();
        assert!(total <= height, "the case is meant to fit");
        assert_eq!(
            changes_rows(m, &pen).len(),
            m.changes().pinned_rows() + m.changes().body_rows()
        );
    };
    fits(&m);
    m.changes.toggle(0);
    fits(&m);
    m.changes.set_runs(
        "test",
        vec![
            crate::workbench::RunRow {
                name: "a::fails".to_owned(),
                passed: false,
                detail: String::new(),
            },
            crate::workbench::RunRow {
                name: "b::passes".to_owned(),
                passed: true,
                detail: String::new(),
            },
        ],
    );
    fits(&m);
    m.changes
        .open_viewer("src/lib.rs", vec!["a".to_owned(), "b".to_owned()]);
    fits(&m);

    // And when it does not fit, the paint is the pinned block plus one
    // window — never more than the stage has rows for.
    let m = long_changes(Size::new(80, 24));
    let (total, height) = m.changes_window();
    assert!(height < total, "the case is meant to overflow");
    assert_eq!(
        changes_rows(&m, &pen).len(),
        m.changes().pinned_rows() + height
    );
}

/// What the tab pins stays put wherever the journal is scrolled to. The
/// run verdict is the reason this matters: a gate's answer lands at the
/// far end of the journal's rows, so without a pin it is invisible
/// behind a hundred files — you run `test` and the stage does not move.
#[test]
fn the_changes_heading_and_run_verdict_stay_pinned_while_the_list_scrolls() {
    let mut m = long_changes(Size::new(80, 24));
    m.changes.set_runs(
        "test",
        vec![
            crate::workbench::RunRow {
                name: "a::fails".to_owned(),
                passed: false,
                detail: String::new(),
            },
            crate::workbench::RunRow {
                name: "b::passes".to_owned(),
                passed: true,
                detail: String::new(),
            },
        ],
    );
    assert_eq!(m.changes().pinned_rows(), 2);
    let heading = "changes · 120 files";
    let verdict = "run test · 1 failed of 2";

    for (label, key) in [
        ("at the top", None),
        ("paged down", Some(KeyCode::PageDown)),
        ("at the end", Some(KeyCode::End)),
        ("back home", Some(KeyCode::Home)),
    ] {
        if let Some(key) = key {
            update(&mut m, Msg::Key(KeyEvent::new(key)));
            Scenes::settle(&mut m);
        }
        let screen = screen(&m);
        assert!(screen.contains(heading), "{label}: the heading: {screen}");
        assert!(screen.contains(verdict), "{label}: the verdict: {screen}");
    }
    // The run's own result rows are body rows, so they are only on
    // screen at the end the journal puts them at.
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Home)));
    Scenes::settle(&mut m);
    assert!(!screen(&m).contains("a::fails"), "the rows scrolled off");
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::End)));
    Scenes::settle(&mut m);
    assert!(screen(&m).contains("a::fails"), "and End reaches them");
}

/// The Changes tab over a journal whose one edit left conflict markers.
fn conflicted(size: Size) -> Model {
    use crate::workbench::JournalEntry;
    let mut m = Model::new(size);
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Tab)));
    assert_eq!(m.tab(), StageTab::Changes);
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![JournalEntry {
                id: "e1".to_owned(),
                path: "src/lib.rs".to_owned(),
                added: 3,
                removed: 0,
                parse_ok: false,
                diff: "@@ -1,1 +1,5 @@\n+<<<<<<< HEAD\n+fn ours() {}\n+=======\n+fn theirs() {}\n+>>>>>>> branch".to_owned(),
            }],
        }),
    );
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 5),
            target: Some(Widget::change_row(0)),
        },
    );
    m
}

#[test]
fn a_conflict_hunk_is_marked_with_the_curly_error_rule() {
    let m = conflicted(Size::new(120, 40));
    let pen = Pen::for_theme(&m.theme());
    let root = view(&m, &clock());
    let resolved = layout(&root, m.size());
    let mut buffer = Buffer::new(m.size().cols, m.size().rows);
    paint(&root, &resolved, &mut buffer);
    // Every painted row as one string; each glyph here is one cell wide,
    // so a char index is a column.
    let rows: Vec<(u16, String)> = (0..m.size().rows)
        .map(|row| {
            let text = (0..m.size().cols)
                .filter_map(|col| buffer.get(col, row).map(|cell| cell.symbol.to_string()))
                .collect();
            (row, text)
        })
        .collect();
    let rule_at = |row: u16, col: usize| {
        buffer
            .get(u16::try_from(col).expect("col"), row)
            .expect("cell exists")
            .style
            .clone()
    };
    let mut marked = 0;
    for marker in ["<<<<<<< HEAD", "=======", ">>>>>>> branch"] {
        let (row, at) = rows
            .iter()
            .find_map(|(row, text)| text.find(marker).map(|at| (*row, at)))
            .unwrap_or_else(|| panic!("{marker:?} is on the screen"));
        let style = rule_at(row, at);
        assert_eq!(
            style.attrs.underline(),
            Some(Underline::Curly),
            "{marker:?} wears the diagnostic rule"
        );
        assert_eq!(
            style.underline_color, pen.diff_conflict.underline_color,
            "{marker:?}'s rule carries the error colour the pen names"
        );
        marked += 1;
    }
    assert_eq!(marked, 3, "every marker of the conflict is called out");
    for ordinary in ["fn ours() {}", "fn theirs() {}"] {
        let (row, at) = rows
            .iter()
            .find_map(|(row, text)| text.find(ordinary).map(|at| (*row, at)))
            .unwrap_or_else(|| panic!("{ordinary:?} is on the screen"));
        assert_eq!(
            rule_at(row, at).attrs.underline(),
            None,
            "an ordinary added line between the markers wears no rule"
        );
    }
}

#[test]
fn the_active_tab_is_the_one_wearing_a_rule() {
    let m = Model::new(Size::new(120, 40));
    let root = view(&m, &clock());
    let resolved = layout(&root, m.size());
    let mut buffer = Buffer::new(m.size().cols, m.size().rows);
    paint(&root, &resolved, &mut buffer);
    let ruled: Vec<(u16, u16)> = (0..m.size().rows)
        .flat_map(|row| (0..m.size().cols).map(move |col| (col, row)))
        .filter(|&(col, row)| {
            buffer
                .get(col, row)
                .is_some_and(|cell| cell.style.attrs.underline() == Some(Underline::Single))
        })
        .collect();
    assert!(!ruled.is_empty(), "the active tab carries a rule");
    let rows: std::collections::BTreeSet<u16> = ruled.iter().map(|&(_, row)| row).collect();
    assert_eq!(rows.len(), 1, "and it is the only plain rule on the screen");
}

// ---------------------------------------------------------------------
// the scale readout — beside the workstream legend, and only
// while the tech-tree is off the fit.
// ---------------------------------------------------------------------

#[test]
fn the_scale_reads_beside_the_workstream_legend_once_the_tree_is_zoomed() {
    let mut m = Scenes::roadmapped(Size::new(120, 40));
    m.set_roadmap_graph_rect(Some(crate::layout::Rect::new(2, 4, 100, 12)));
    assert!(
        !screen(&m).contains("zoom "),
        "a fitted tree grows no chrome: {}",
        screen(&m)
    );
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char('+'))));
    let zoomed = screen(&m);
    assert!(zoomed.contains("zoom 150%"), "{zoomed}");
    // The bands keep their column, so the readout can never sit under
    // the pixel surface placed to the legend's right.
    let band = m.roadmap().band_rows()[0].clone();
    assert!(zoomed.contains(band.trim()), "{zoomed}");
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char('0'))));
    assert!(
        !screen(&m).contains("zoom "),
        "the fit puts it away again: {}",
        screen(&m)
    );
}

#[test]
fn the_undock_column_windows_by_the_same_rows_as_the_stage() {
    for size in [Size::new(160, 24), Size::new(180, 50), Size::new(200, 40)] {
        let mut m = Scenes::roadmapped(size);
        think_step_frame(&mut m, 7);
        let resolved = layout(&view(&m, &clock()), size);
        let rect_of = |id: crate::WidgetId| {
            resolved
                .placed()
                .iter()
                .find(|p| p.id == Some(id))
                .map(|p| p.rect)
                .expect("placed")
        };
        let undock = rect_of(Widget::Undock.id());
        // The third column sits in the same body row as the stage, so it
        // is always the stage's height plus the panel ring and the tab
        // bar — and its own chrome, a cell of padding at each edge plus
        // the ⇱ heading, comes to exactly those three rows. The two
        // cancel: what the column can show is the stage's own window for
        // the tab it holds, row for row. That identity is what lets the
        // undocked tab share the tab's viewport instead of minting one
        //  — break it and the column starts clipping.
        assert_eq!(
            rect_of(Widget::Stage.id()).height + 3,
            undock.height,
            "{size:?}"
        );
        let chrome = 1 + 2 * usize::from(Space::PAD);
        assert_eq!(m.undocked_tab(), Some(StageTab::Trace));
        assert_eq!(
            m.trace_window().1 + m.trace().pinned_rows() + chrome,
            usize::from(undock.height),
            "the trace feed fills the column exactly at {size:?}"
        );
        let on_trace = m.with_tab(StageTab::Trace);
        assert_eq!(on_trace.undocked_tab(), Some(StageTab::Changes));
        assert_eq!(
            on_trace.changes_window().1 + on_trace.changes().pinned_rows() + chrome,
            usize::from(undock.height),
            "and so does the journal at {size:?}"
        );
    }
}

/// The trace fixture plus a roadmap chunk that names step 1's execution
/// ref back in its own `cross_refs` — the edge `roadmap_link` writes.
fn traced_with_chunk(size: Size) -> Model {
    let mut m = Scenes::traced(size);
    Scenes::engine_frame(
        &mut m,
        "roadmap",
        "chunk_changed",
        serde_json::json!({"chunk": {
            "id": "tui2-5b", "name": "Trace chunk jump", "title": "Jump from a step to its chunk",
            "status": "pending", "priority": 115, "group": "TUI rewrite",
            "cross_refs": ["task:trace-state"],
        }}),
    );
    m
}

/// Expand step 1 of the trace fixture.
fn expand_first_step(m: &mut Model) {
    update(
        m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 8),
            target: Some(Widget::trace_row(0)),
        },
    );
    assert!(m.trace().is_expanded(1));
}

/// Every widget id the surface registers.
fn placed_ids(m: &Model) -> Vec<crate::WidgetId> {
    layout(&view(m, &clock()), m.size())
        .placed()
        .iter()
        .filter_map(|p| p.id)
        .collect()
}

#[test]
fn an_execution_ref_that_names_a_chunk_is_a_link_and_one_that_names_none_is_not() {
    let mut linked = traced_with_chunk(Size::new(120, 40));
    expand_first_step(&mut linked);
    assert!(
        placed_ids(&linked).contains(&Widget::trace_chunk(1)),
        "the resolving ref carries the jump"
    );

    // The same fixture with no roadmap at all: the ref names nothing the
    // tab knows, so the row stays exactly the plain text it always was.
    let mut plain = Scenes::traced(Size::new(120, 40));
    expand_first_step(&mut plain);
    assert!(!placed_ids(&plain).contains(&Widget::trace_chunk(1)));
    assert!(
        screen(&plain).contains("execution ref · task:trace-state"),
        "{}",
        screen(&plain)
    );
}

#[test]
fn the_chunk_link_does_not_move_a_row() {
    // The link is the field row it always was, painted differently — the
    // scroll counts those rows, so a resolving ref must not add one.
    let mut linked = traced_with_chunk(Size::new(120, 40));
    let mut plain = Scenes::traced(Size::new(120, 40));
    expand_first_step(&mut linked);
    expand_first_step(&mut plain);
    assert_eq!(linked.trace().body_rows(), plain.trace().body_rows());
}

#[test]
fn the_strip_shows_the_judge_led_only_once_a_verdict_exists() {
    use crate::cockpit::LoopEvent;
    let mut m = Scenes::looping(Size::new(80, 24));
    let mut before = Vec::new();
    texts(&view(&m, &clock()), &mut before);
    assert!(
        !before.iter().any(|t| t.contains("judge")),
        "no judge, no LED: {before:?}"
    );
    update(
        &mut m,
        Msg::Loop(LoopEvent::Verdict {
            iteration: 2,
            ok: false,
            impossible: true,
            reason: "the acceptance contradicts itself".to_owned(),
        }),
    );
    let mut after = Vec::new();
    texts(&view(&m, &clock()), &mut after);
    let led = after
        .iter()
        .position(|t| t == "✗ judge")
        .expect("the LED is on the strip");
    let elapsed = after
        .iter()
        .position(|t| t == "1m 23s")
        .expect("the clock is on the strip");
    assert_eq!(led + 1, elapsed, "the LED sits right before the clock");
}

/// The cells of every pane row on the screen, top to bottom, with the
/// ink and ground each was painted in.
fn pane_cells(model: &Model) -> Vec<Vec<(String, Color, Color, crate::render::Attrs)>> {
    let size = model.size();
    let root = view(model, &clock());
    let resolved = layout(&root, size);
    let mut buffer = Buffer::new(size.cols, size.rows);
    paint(&root, &resolved, &mut buffer);
    let mut rows = Vec::new();
    for index in 0..model.visible_rows().len() {
        if model.visible_rows()[index].kind != RowKind::Pane {
            continue;
        }
        let Some(rect) = resolved
            .placed()
            .iter()
            .find(|p| p.id == Some(Widget::stage_row(index)))
            .map(|p| p.rect)
        else {
            continue;
        };
        rows.push(
            (rect.x..rect.x + rect.width)
                .map(|x| {
                    let cell = buffer.get(x, rect.y).expect("inside the buffer");
                    (
                        cell.symbol.to_string(),
                        cell.style.fg,
                        cell.style.bg,
                        cell.style.attrs,
                    )
                })
                .collect(),
        );
    }
    rows
}

/// The pane paints the program's own screen: its last eight rows, cut
/// to the stage, each cell in the program's hue through the palette —
/// cargo's green as the palette's green, a truecolor word as itself —
/// the cursor reversed on the tail while the run is live, and every
/// painted glyph over the contrast floor.
#[test]
fn the_pane_paints_the_programs_cells_in_its_own_hues_over_the_floor() {
    use magistr_theme::{ThemeColor, catppuccin_mocha, math};
    let palette = catppuccin_mocha();
    let mut m =
        Scenes::terminal_pane(Size::new(120, 40)).with_theme(Theme::forced(ThemeMode::Dark));
    Scenes::settle(&mut m);
    let rows = pane_cells(&m);
    assert_eq!(
        rows.len(),
        crate::turn::Turn::PANE_ROWS + 1,
        "the strip and eight pane rows on screen"
    );
    let text = |row: &Vec<(String, Color, Color, crate::render::Attrs)>| {
        row.iter().map(|(s, ..)| s.as_str()).collect::<String>()
    };
    let lines: Vec<String> = rows.iter().map(text).collect();
    // The strip under the head: the program's own report, 70%.
    let icons = Icons::UNICODE;
    assert!(
        lines[0].contains(&icons.bar_full.repeat(2)) && lines[0].contains("70%"),
        "the strip reads the OSC 9;4 report: {lines:?}"
    );
    assert!(
        lines[1].contains("warning: unused variable: `pane`"),
        "ten rows written, the last eight shown: {lines:?}"
    );
    assert!(
        lines[3].contains("    Finished `test` profile in 4.2s"),
        "{lines:?}"
    );
    assert!(
        lines[8].contains("progress [#######---]  70%  (3 tests still running)"),
        "{lines:?}"
    );
    // The time rail: a break and its stamp where the
    // build paused for two seconds, a bare tick where the tests began
    // their second, a break and its stamp after the long silence before
    // the progress row — and nothing beside the rows in between.
    assert!(
        lines[3].contains(&format!("0:02{}", icons.rail_break)),
        "{lines:?}"
    );
    assert!(lines[5].contains(icons.rail_tick), "{lines:?}");
    assert!(
        !lines[5].contains("0:"),
        "a plain second is never a number: {lines:?}"
    );
    assert!(
        lines[8].contains(&format!("0:10{}", icons.rail_break)),
        "{lines:?}"
    );
    for quiet in [1, 2, 4, 6, 7] {
        assert!(
            !lines[quiet].contains(icons.rail_tick) && !lines[quiet].contains(icons.rail_break),
            "row {quiet} is quiet: {lines:?}"
        );
    }
    // `Finished` is cargo's bold green: the palette's green, bold. The
    // row is the gutter, the rail's seven columns, four spaces, then it.
    let (_, fg, bg, attrs) = &rows[3][1 + 7 + 4];
    assert_eq!(
        *fg,
        Color::Rgb(palette.green.r, palette.green.g, palette.green.b)
    );
    assert!(attrs.contains(crate::render::Attrs::BOLD));
    assert_eq!(
        *bg,
        Color::Rgb(palette.surface0.r, palette.surface0.g, palette.surface0.b)
    );
    // The truecolor test name is itself (peach clears the floor on the
    // night ground as it is).
    let peach = rows[7]
        .iter()
        .find(|(s, ..)| s == "p")
        .expect("the peach word");
    assert_eq!(peach.1, Color::Rgb(250, 179, 135));
    // The cursor sits at the tail's end, reversed — the pen spends the
    // reverse swapping the pair, so the cell's ground is the text ink.
    let text_ink = Color::Rgb(palette.text.r, palette.text.g, palette.text.b);
    let cursor: Vec<usize> = rows[8]
        .iter()
        .enumerate()
        .filter(|(_, (_, _, bg, _))| *bg == text_ink)
        .map(|(col, _)| col)
        .collect();
    let end = lines[8].trim_end().chars().count();
    assert_eq!(
        cursor,
        [end],
        "one cursor cell after the text: {:?}",
        rows[8]
    );
    // Every glyph clears the floor.
    for (row, cells) in rows.iter().enumerate() {
        for (s, fg, bg, _) in cells {
            let ch = s.chars().next().unwrap_or(' ');
            if ch.is_whitespace() || STRUCTURAL.contains(ch) {
                continue;
            }
            let (Color::Rgb(r, g, b), Color::Rgb(br, bg_, bb)) = (fg, bg) else {
                panic!("row {row}: {s:?} painted in a non-palette colour");
            };
            let ratio =
                math::contrast_ratio(ThemeColor::new(*r, *g, *b), ThemeColor::new(*br, *bg_, *bb));
            assert!(ratio >= Pen::MIN_CONTRAST, "row {row}: {s:?} at {ratio:.2}");
        }
    }
}

/// Scrolled up inside the card, the pane holds its place, the cursor
/// leaves the view and the jump pill counts the rows that arrived.
#[test]
fn a_scrolled_pane_holds_still_and_wears_the_jump_pill() {
    let mut m = Scenes::terminal_pane_scrolled(Size::new(120, 40))
        .with_theme(Theme::forced(ThemeMode::Dark));
    Scenes::settle(&mut m);
    let rows = pane_cells(&m);
    let lines: Vec<String> = rows
        .iter()
        .map(|row| row.iter().map(|(s, ..)| s.as_str()).collect::<String>())
        .collect();
    assert!(
        lines[0].contains("100%"),
        "the strip reads the newest report even while held: {lines:?}"
    );
    assert!(
        lines[1].contains("   Compiling magistr-cockpit v0.1.0"),
        "two notches up from ten rows: the first row leads: {lines:?}"
    );
    assert!(
        lines[1].contains(&format!("0:00{}", Icons::UNICODE.rail_tick)),
        "the origin is stamped: {lines:?}"
    );
    assert!(
        lines[8].trim_end().ends_with("Latest \u{b7} +3"),
        "the pill on the last row counts three arrived rows: {lines:?}"
    );
    let palette = magistr_theme::catppuccin_mocha();
    let text_ink = Color::Rgb(palette.text.r, palette.text.g, palette.text.b);
    assert!(
        !rows.iter().flatten().any(|(_, _, bg, _)| *bg == text_ink),
        "no cursor while scrolled"
    );
    assert!(
        !lines.iter().any(|l| l.contains("##########")),
        "the rows that arrived are under the view, not in it"
    );
}

/// The terminal dock on a terminal: raised, its grip
/// row and its screen are laid out under the body above the prompt —
/// the grip naming the command and the key from the registry, the
/// docked rows the program's own — and the stage gives up exactly the
/// rows the dock took; lowered, none of it is there.
#[test]
fn the_terminal_dock_is_laid_out_over_the_stage_above_the_prompt() {
    let size = Size::new(120, 40);
    let mut down = Scenes::terminal_pane(size);
    Scenes::settle(&mut down);
    let stage = down.stage_rows();
    let before = laid_out(&down);
    assert!(!before.contains(&Widget::TermDock));
    assert!(!before.contains(&Widget::TermDockGrip));
    let up = Scenes::terminal_dock(size);
    let widgets = laid_out(&up);
    assert!(widgets.contains(&Widget::TermDock), "{widgets:?}");
    assert!(widgets.contains(&Widget::TermDockGrip));
    let rows = usize::from(up.term_dock().rows());
    assert_eq!(
        rows,
        up.dock_first_rows() as usize,
        "the first rise is the third"
    );
    assert_eq!(up.stage_rows(), stage - rows - 1, "the rows and the grip");
    let text = screen(&up);
    let hint = Binding::hint(&[Binding::DOCK]);
    let grip = text
        .lines()
        .find(|line| line.contains(&hint))
        .expect("the grip row carries the registry's key");
    assert!(
        grip.contains("cargo nextest run -p magistr-tui"),
        "the grip names the command: {grip}"
    );
    assert!(
        grip.starts_with("\u{2500}\u{2500}"),
        "the panel grammar's divider: {grip}"
    );
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.contains(&hint))
        .expect("grip");
    // The docked rows follow the grip: the run's tail, ending where the
    // program's cursor stands, above the prompt line.
    let docked: Vec<&str> = lines[at + 1..at + 1 + rows].to_vec();
    assert!(
        docked.iter().any(|line| line.contains("progress [")),
        "the tail of the run is in the dock: {docked:?}"
    );
    let resolved = layout(&view(&up, &clock()), size);
    let rect_of = |widget: Widget| {
        resolved
            .placed()
            .iter()
            .find(|p| p.id == Some(widget.id()))
            .map(|p| p.rect)
            .expect("placed")
    };
    let dock = rect_of(Widget::TermDock);
    let prompt = rect_of(Widget::Prompt);
    let stage_rect = rect_of(Widget::Stage);
    assert_eq!(usize::from(dock.height), rows + 1, "the grip and the rows");
    assert!(
        dock.y >= stage_rect.y + stage_rect.height,
        "the dock stands under the stage: {dock:?} vs {stage_rect:?}"
    );
    assert!(
        prompt.y >= dock.y + dock.height,
        "and above the prompt: {prompt:?} vs {dock:?}"
    );
}
