use super::*;
use crate::agent::{AgentEvent, PermissionChoice, PermissionKind};
use crate::cockpit::{CheckpointCmd, Cmd};
use crate::cockpit::{LauncherData, LauncherSession, Msg, SessionOutcome, Size, Surface, update};
use crate::input::{KeyCode, KeyEvent, Modifiers};
use crate::job::{JobId, JobOutput};
use magistr_engine::{CheckpointReason, policy::RiskTier};

fn native() -> Model {
    Model::new(Size::new(120, 40)).with_surface(Surface::Native)
}

fn key(model: &mut Model, code: KeyCode, mods: Modifiers) {
    let _ = update(model, Msg::Key(KeyEvent::new(code).with(mods)));
}

fn chord(model: &mut Model) {
    key(model, KeyCode::Char('k'), Modifiers::SUPER);
}

fn typed(model: &mut Model, text: &str) {
    for c in text.chars() {
        key(model, KeyCode::Char(c), Modifiers::NONE);
    }
}

/// The capture's own seed: the project named think-and-ship with one
/// plan-less ask pending on the dock — a badge without a Moment.
fn seeded() -> Model {
    let mut model = native();
    let _ = update(
        &mut model,
        Msg::Job {
            id: JobId::new(0),
            output: JobOutput::Launcher(Box::new(LauncherData {
                project: "think-and-ship".to_owned(),
                has_config: true,
                stack: "Rust / Cargo".to_owned(),
                git: "master, clean".to_owned(),
                backend: "ACP (available)".to_owned(),
                sessions: vec![LauncherSession {
                    id: Some("sess-1".to_owned()),
                    when: "Aug 30 14:02".to_owned(),
                    duration: "12m".to_owned(),
                    outcome: SessionOutcome::Success,
                    preview: "shipped the palette".to_owned(),
                }],
            })),
        },
    );
    let _ = update(
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
    model
}

#[test]
fn the_scorer_is_a_subsequence_matcher_that_prefers_word_starts() {
    assert!(score("thi", "Open think-and-ship").is_some());
    assert!(score("xyz", "Open think-and-ship").is_none());
    // A word-start run beats the same letters scattered mid-word.
    let start = score("thi", "think-and-ship").expect("prefix");
    let scattered = score("thi", "worth chip").expect("subsequence");
    assert!(start > scattered, "{start} vs {scattered}");
    // Case never matters: ⌘K is typed lowercase over Title Case rows.
    assert!(score("open", "Open think-and-ship").is_some());
}

#[test]
fn cmd_k_raises_the_palette_on_a_window_and_never_on_a_terminal() {
    let mut terminal = Model::new(Size::new(120, 40));
    chord(&mut terminal);
    assert!(terminal.palette().is_none(), "a terminal holds no palette");

    let mut window = native();
    chord(&mut window);
    assert!(window.palette().is_some());
    // The chord again puts it away.
    chord(&mut window);
    assert!(window.palette().is_none());
    // Esc puts it away too.
    chord(&mut window);
    key(&mut window, KeyCode::Esc, Modifiers::NONE);
    assert!(window.palette().is_none());
}

#[test]
fn typing_filters_to_the_project_place_with_its_live_badge_and_act() {
    let mut model = seeded();
    chord(&mut model);
    typed(&mut model, "thi");
    let rows = Palette::rows(&model);
    let first = rows.first().expect("a match for thi");
    assert_eq!(first.verb, "Open");
    assert_eq!(first.name.as_deref(), Some("think-and-ship"));
    assert!(first.sigil);
    assert_eq!(first.badge.as_deref(), Some("1 action required"));
    assert_eq!(first.act.as_deref(), Some("Answer pending decision"));
    assert_eq!(model.palette().expect("open").query(), "thi");
}

#[test]
fn enter_opens_the_place_and_the_palette_goes_away() {
    let mut model = seeded();
    chord(&mut model);
    typed(&mut model, "thi");
    key(&mut model, KeyCode::Enter, Modifiers::NONE);
    assert!(model.palette().is_none());
    assert_eq!(
        model.place(),
        Some(Place::Sky),
        "the project place is the sky"
    );
}

#[test]
fn cmd_enter_acts_without_switching() {
    let mut model = seeded();
    assert_eq!(model.place(), Some(Place::Session));
    chord(&mut model);
    typed(&mut model, "thi");
    key(&mut model, KeyCode::Enter, Modifiers::SUPER);
    assert!(model.palette().is_none());
    assert_eq!(model.place(), Some(Place::Session), "⌘↵ stays put");
    // The pending decision now owns the keys the palette was
    // capturing: the dock front's accelerator answers it.
    let cmds = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char('a'))));
    assert!(!cmds.is_empty(), "the answer reached the dock front");
}

#[test]
fn every_binding_is_reachable_and_a_verb_row_dispatches_its_key() {
    for binding in Binding::ALL {
        // Kill is press-and-hold: the sheet lists it,
        // the palette never dispatches it — see below.
        if *binding == Binding::KILL {
            continue;
        }
        let mut model = native();
        chord(&mut model);
        typed(&mut model, binding.does);
        let rows = Palette::rows(&model);
        // The abort row wears the window's chord: the palette is the
        // window's, where `esc` climbs.
        let keys = if *binding == Binding::ABORT {
            Binding::ABORT_CHORD.keys
        } else if *binding == Binding::DOCK {
            Binding::DOCK_CHORD.keys
        } else {
            binding.keys
        };
        assert!(
            rows.iter().any(|row| row.keys == Some(keys)),
            "'{}' finds no row for {keys}",
            binding.does,
        );
    }
    // Running one is pressing it: the sheet's own verb opens the sheet.
    let mut model = native();
    chord(&mut model);
    typed(&mut model, "keyboard help");
    let rows = Palette::rows(&model);
    assert_eq!(rows.first().and_then(|row| row.keys), Some("?"));
    key(&mut model, KeyCode::Enter, Modifiers::NONE);
    assert!(model.palette().is_none());
    assert!(model.is_sheet_open(), "the dispatched '?' opened the sheet");
}

/// The loop's verbs are rows that dispatch the verb itself — never the
/// key, which would be the prompt's letter while the loop runs — and
/// the pause row is Resume while the loop holds a boundary.
#[test]
fn a_loop_verb_row_dispatches_the_verb_not_its_key() {
    use crate::cockpit::{CheckpointCmd, Cmd, LoopCmd, LoopEvent};
    use magistr_engine::CheckpointReason;
    let mut model = native();
    let _ = update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 1,
            max: 3,
        }),
    );
    chord(&mut model);
    typed(&mut model, "step");
    let rows = Palette::rows(&model);
    assert_eq!(rows.first().and_then(|row| row.keys), Some("s"));
    let cmds = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(cmds.contains(&Cmd::Loop(LoopCmd::Step)), "{cmds:?}");
    assert!(model.prompt().is_empty(), "no letter reached the prompt");

    chord(&mut model);
    typed(&mut model, "pause");
    let cmds = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(cmds.contains(&Cmd::Loop(LoopCmd::Pause)), "{cmds:?}");
    chord(&mut model);
    typed(&mut model, "pause");
    let rows = Palette::rows(&model);
    assert_eq!(
        rows.first().map(|row| row.go.clone()),
        Some(PaletteGo::Loop(LoopCmd::Resume)),
        "held, the same row resumes"
    );
    let cmds = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(cmds.contains(&Cmd::Loop(LoopCmd::Resume)), "{cmds:?}");

    chord(&mut model);
    typed(&mut model, "checkpoint now");
    let cmds = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(
        matches!(
            cmds.as_slice(),
            [Cmd::Checkpoint(CheckpointCmd::Save {
                reason: CheckpointReason::Manual,
                ..
            })]
        ),
        "{cmds:?}"
    );
}

#[test]
fn a_chunk_place_opens_the_sky_on_that_star() {
    let mut model = native();
    model.roadmap.seed(
        &[serde_json::json!({
            "id": "gui2-2d5", "name": "palette", "title": "the palette",
            "status": "in_progress", "priority": 162,
        })],
        &[],
    );
    chord(&mut model);
    typed(&mut model, "gui2-2d5");
    let rows = Palette::rows(&model);
    let first = rows.first().expect("the chunk place");
    assert_eq!(first.name.as_deref(), Some("gui2-2d5"));
    assert_eq!(first.badge.as_deref(), Some("in progress"));
    key(&mut model, KeyCode::Enter, Modifiers::NONE);
    assert_eq!(model.place(), Some(Place::Sky));
    assert_eq!(
        model.roadmap().selected().map(|chunk| chunk.id.as_str()),
        Some("gui2-2d5")
    );
}

/// ↵ on a chunk row goes through the one open-chunk fold: the chunk
/// this window's session is on lands on its Session. ⌘↵ acts on the
/// row and the camera holds — the one opener that is not the fold's.
#[test]
fn enter_on_a_chunk_row_takes_the_fold_and_cmd_enter_holds_the_camera() {
    let seed = |model: &mut Model| {
        update(
            model,
            Msg::Engine(crate::engine::EngineEvent::Seed(crate::engine::StoreSeed {
                ship_status: serde_json::Value::Null,
                chunk: Some("gui2-2d5".to_owned()),
                done: 0,
                total: 1,
                signals_pending: 0,
                chunks: vec![serde_json::json!({
                    "id": "gui2-2d5", "name": "palette", "title": "the palette",
                    "status": "in_progress", "priority": 162,
                })],
                signals: Vec::new(),
                trace: Vec::new(),
            })),
        );
    };
    let mut model = native().with_place(Place::Sky);
    seed(&mut model);
    chord(&mut model);
    typed(&mut model, "gui2-2d5");
    key(&mut model, KeyCode::Enter, Modifiers::NONE);
    assert_eq!(
        model.place(),
        Some(Place::Session),
        "executing → its Session"
    );
    assert_eq!(
        model.roadmap().selected().map(|chunk| chunk.id.as_str()),
        Some("gui2-2d5")
    );

    let mut model = native().with_place(Place::Sky);
    seed(&mut model);
    chord(&mut model);
    typed(&mut model, "gui2-2d5");
    key(&mut model, KeyCode::Enter, Modifiers::SUPER);
    assert!(model.palette().is_none());
    assert_eq!(model.place(), Some(Place::Sky), "⌘↵ holds the camera");
    assert!(model.roadmap().selected().is_none(), "and opens nothing");
}

#[test]
fn the_choice_walks_and_clamps_and_typing_snaps_it_back() {
    let mut model = seeded();
    chord(&mut model);
    let len = Palette::rows(&model).len();
    assert!(len > 1, "an empty query lists the places");
    key(&mut model, KeyCode::Down, Modifiers::NONE);
    assert_eq!(model.palette().expect("open").chosen(), 1);
    for _ in 0..64 {
        key(&mut model, KeyCode::Down, Modifiers::NONE);
    }
    assert_eq!(model.palette().expect("open").chosen(), len - 1, "clamped");
    typed(&mut model, "t");
    assert_eq!(model.palette().expect("open").chosen(), 0, "snapped back");
    key(&mut model, KeyCode::Backspace, Modifiers::NONE);
    assert_eq!(model.palette().expect("open").query(), "");
}

/// The abort row wears the window's chord, never the terminal's `esc`
/// — a window's Esc climbs — and dispatches the verb,
/// which is nothing without a live loop.
#[test]
fn the_abort_row_wears_the_windows_chord_and_aborts() {
    use crate::cockpit::{Cmd, LoopCmd, LoopEvent};
    let mut model = native();
    chord(&mut model);
    typed(&mut model, "abort");
    let rows = Palette::rows(&model);
    assert_eq!(
        rows.first().and_then(|row| row.keys),
        Some(Binding::ABORT_CHORD.keys)
    );
    assert_eq!(
        rows.first().map(|row| row.go.clone()),
        Some(PaletteGo::Loop(LoopCmd::Abort))
    );
    assert!(
        !rows
            .iter()
            .any(|row| row.keys == Some(Binding::ABORT.keys) && row.verb == "abort"),
        "no row promises esc"
    );
    assert_eq!(
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter))),
        vec![]
    );
    let _ = update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 1,
            max: 3,
        }),
    );
    chord(&mut model);
    typed(&mut model, "abort");
    let cmds = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(cmds.contains(&Cmd::Loop(LoopCmd::Abort)), "{cmds:?}");
    assert!(model.prompt().is_empty(), "no letter reached the prompt");
}

/// The previewed checkpoint's Restore and Fork are ⌘K rows — the
/// keyboard's way to the rail's hover verbs;
/// nothing previewed, nothing offered.
#[test]
fn the_previewed_checkpoints_verbs_are_palette_rows() {
    let mut model = native();
    let id = model
        .rail_mut()
        .spine_mut()
        .open(CheckpointReason::Manual, "before golden update");
    model.rail_mut().spine_mut().saved(&id, None, true);
    chord(&mut model);
    typed(&mut model, "restore cp-1");
    assert!(
        Palette::rows(&model).is_empty(),
        "nothing previewed, nothing offered"
    );
    key(&mut model, KeyCode::Esc, Modifiers::NONE);
    model.rail_mut().spine_mut().preview(0);
    chord(&mut model);
    typed(&mut model, "restore cp-1");
    let rows = Palette::rows(&model);
    let first = rows.first().expect("the restore row");
    assert_eq!(first.verb, "Restore");
    assert_eq!(first.name.as_deref(), Some("cp-1 · before golden update"));
    assert_eq!(first.keys, None, "a verb of the rail's, not a key's");
    let cmds = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert_eq!(
        cmds,
        vec![Cmd::Checkpoint(CheckpointCmd::Restore {
            id: "cp-1".to_owned()
        })]
    );
    chord(&mut model);
    typed(&mut model, "fork cp-1");
    let cmds = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert_eq!(
        cmds,
        vec![Cmd::Checkpoint(CheckpointCmd::Fork {
            id: "cp-1".to_owned(),
            branch: None,
        })]
    );
}

/// The dock row wears the window's chord, never
/// the terminal's `ctrl-t`, and running it raises the running card's
/// screen — nothing on a session with no terminal.
#[test]
fn the_dock_row_wears_the_windows_chord_and_raises_the_dock() {
    let mut model = native();
    chord(&mut model);
    typed(&mut model, "dock terminal");
    let rows = Palette::rows(&model);
    assert_eq!(
        rows.first().and_then(|row| row.keys),
        Some(Binding::DOCK_CHORD.keys)
    );
    assert!(
        !rows.iter().any(|row| row.keys == Some(Binding::DOCK.keys)),
        "no row promises ctrl-t on a window"
    );
    assert_eq!(
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter))),
        vec![]
    );
    assert!(model.palette().is_none());
    assert!(!model.term_dock().is_up(), "nothing ran on a terminal");
}

/// The charter's offer is a palette row wearing the
/// scope and its record, and Enter accepts it by key — the same fold
/// as the rail's Delegate.
#[test]
fn a_charter_offer_is_a_delegate_row() {
    let mut model = native();
    model
        .rail_mut()
        .set_offers(vec![crate::cockpit::CharterOffer::new(
            "edit:crates/magistr-tui",
            "Edit \u{b7} crates/magistr-tui",
            14,
            14,
        )]);
    chord(&mut model);
    typed(&mut model, "delegate edit");
    let rows = Palette::rows(&model);
    let first = rows.first().expect("the offer's row");
    assert_eq!(first.verb, "Delegate");
    assert_eq!(
        first.name.as_deref(),
        Some("Edit \u{b7} crates/magistr-tui \u{b7} allowed 14 of 14")
    );
    assert_eq!(first.keys, None);
    let cmds = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert_eq!(
        cmds,
        vec![Cmd::Delegate("edit:crates/magistr-tui".to_owned())]
    );
    assert!(model.rail().offers().is_empty(), "accepted, gone");
    assert!(
        model
            .rail()
            .charter()
            .iter()
            .any(|rule| rule.subject() == "Edit \u{b7} crates/magistr-tui")
    );
}

/// A palette row is a press with no release, and a hold nobody lets
/// go of would complete on the timer: Kill has no row, however it is
/// searched for.
#[test]
fn kill_is_never_a_palette_row() {
    for query in ["kill", "k", "hold"] {
        let mut model = native();
        chord(&mut model);
        typed(&mut model, query);
        let rows = Palette::rows(&model);
        assert!(
            rows.iter().all(|row| row.keys != Some(Binding::KILL.keys)
                && !row.name.as_deref().is_some_and(|n| n.contains("kill"))),
            "'{query}' offered a kill row: {rows:?}"
        );
    }
    let mut model = native();
    chord(&mut model);
    typed(&mut model, "copy transcript");
    let rows = Palette::rows(&model);
    assert!(
        rows.iter().any(|row| row.keys == Some(Binding::COPY.keys)),
        "Copy has its row: {rows:?}"
    );
}
