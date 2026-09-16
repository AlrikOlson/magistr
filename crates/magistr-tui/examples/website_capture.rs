//! Reproducible website captures from a synthetic project; never reads a native store.
//!
//! `cargo run -p magistr-tui --example website_capture -- <fixture.json> <out-dir>`
//! renders four states of the fixture's project at the capture size, in
//! both themes, through the real terminal renderer: the Roadmap tab as
//! an overview (`terminal-overview-{dark,light}.png`), the Roadmap tab
//! with the chunk in progress selected so the focus chain lights
//! (`terminal-roadmap-{dark,light}.png`), and the Session view with a
//! mid-trace step selected so its card and relations show
//! (`terminal-session-{dark,light}.png`), and Turn with its conversation, test
//! terminal and selected edit diff (`terminal-turn-{dark,light}.png`).
//! `scripts/website-captures.sh` runs this and rewrites the manifest beside them.

use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use magistr_cockpit::agent::{AgentEvent, EditDiff, ToolRecord, ToolStatus, ToolVerb};
use magistr_cockpit::cockpit::{FrameClock, Model, Msg, Size, StageTab, Surface, Widget, update};
use magistr_cockpit::harness::fixtures;
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use magistr_cockpit::theme::{Theme, ThemeMode};
use magistr_engine::NativeSnapshot;
use magistr_engine::native::ChunkStatus;
use magistr_tui::screenshot::{Profile, frame};
use serde_json::Value;

/// The capture's terminal size: wide enough for the graph and a card
/// side by side with the rail, short enough to read on a page.
const CAPTURE: Size = Size::new(132, 42);

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(
        args.next()
            .ok_or("expected a synthetic fixture JSON path")?,
    );
    let output = PathBuf::from(args.next().ok_or("expected an output directory")?);
    let data: Value = serde_json::from_slice(&std::fs::read(input)?)?;
    if data["synthetic"] != true {
        return Err("website captures require an explicitly synthetic project".into());
    }
    let records = |key: &str| -> Result<Vec<Value>, Box<dyn Error>> {
        Ok(data[key]
            .as_array()
            .ok_or("missing fixture records")?
            .clone())
    };
    let chunks = fixtures::chunks_from(records("chunks")?);
    for chunk in &chunks {
        for dep in &chunk.deps {
            let dependency = chunks
                .iter()
                .find(|c| &c.id == dep)
                .ok_or("dangling dependency")?;
            if matches!(chunk.status, ChunkStatus::Done | ChunkStatus::InProgress)
                && dependency.status != ChunkStatus::Done
            {
                return Err(
                    format!("{} starts before dependency {} is done", chunk.id, dep).into(),
                );
            }
        }
    }
    let edges = chunks.iter().map(|c| c.deps.len()).sum::<usize>();
    if chunks.len() < 50 || edges < 100 {
        return Err("capture fixture must exercise a complex roadmap".into());
    }
    let done = chunks
        .iter()
        .filter(|c| c.status == ChunkStatus::Done)
        .count();
    let steps = fixtures::steps_from(records("steps")?);
    if steps.len() < 8 || !steps.iter().any(|step| step.branch_id.is_some()) {
        return Err("capture fixture must carry a branched reasoning trace".into());
    }
    let snapshot = NativeSnapshot {
        progress: fixtures::progress(done as u32, chunks.len() as u32),
        chunks,
        signals: fixtures::signals_from(records("signals")?),
        steps,
        tasks: records("tasks")?
            .into_iter()
            .map(fixtures::task_from)
            .collect(),
        objective: Some(fixtures::objective_from(data["objective"].clone())),
        ..NativeSnapshot::default()
    };
    let journal = records("changes")?
        .into_iter()
        .map(|row| {
            Ok(magistr_cockpit::workbench::JournalEntry {
                id: row["id"].as_str().ok_or("missing change id")?.into(),
                path: row["path"].as_str().ok_or("missing change path")?.into(),
                at_ms: row["at_ms"].as_i64(),
                added: row["added"].as_u64().unwrap_or(0) as u32,
                removed: row["removed"].as_u64().unwrap_or(0) as u32,
                parse_ok: row["parse_ok"].as_bool().unwrap_or(false),
                diff: row["diff"].as_str().unwrap_or_default().into(),
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    // The branch statuses arrive as the engine events that carry them —
    // a merged branch draws its way back to the main line.
    let branches = data["branches"].as_array().cloned().unwrap_or_default();
    std::fs::create_dir_all(&output)?;
    for (mode, theme) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        for (name, tab) in [
            ("overview", StageTab::Roadmap),
            ("roadmap", StageTab::Roadmap),
            ("session", StageTab::Session),
            ("turn", StageTab::Turn),
        ] {
            let mut model = Model::new(CAPTURE)
                .with_surface(Surface::Terminal)
                .with_title(data["project"].as_str().ok_or("missing project name")?)
                .with_snapshot(&snapshot)
                .with_theme(Theme::forced(theme))
                .with_tab(tab);
            update(
                &mut model,
                Msg::Workbench(magistr_cockpit::cockpit::WorkbenchEvent::Journal {
                    entries: journal.clone(),
                }),
            );
            for branch in &branches {
                update(
                    &mut model,
                    Msg::Engine(fixtures::event(
                        "think",
                        "branch_status_changed",
                        branch.clone(),
                    )),
                );
            }
            match (name, tab) {
                (_, StageTab::Turn) => turn(&mut model, &data["turn"])?,
                (_, StageTab::Session) => {
                    // The step two before the newest: its card, its
                    // relations and the branch it stands on all show.
                    let index = snapshot.steps.len().saturating_sub(3);
                    click(&mut model, Widget::trace_row(index));
                }
                ("roadmap", _) => {
                    // The chunk in progress heads the list: selecting it
                    // opens its card and lights its dependency chain.
                    click(&mut model, Widget::roadmap_row(0));
                }
                // The overview: the whole tree, nothing selected.
                _ => {}
            }
            for _ in 0..8 {
                update(&mut model, Msg::Tick);
            }
            let pixels = frame(
                &model,
                &FrameClock::new(8, Duration::from_millis(1600)),
                Profile::Kitty,
            )?;
            pixels.save_png(output.join(format!("terminal-{name}-{mode}.png")))?;
        }
    }
    eprintln!(
        "Synthetic Harbor project: {} chunks, {edges} dependencies, {done} completed, {} reasoning steps; eight terminal captures.",
        snapshot.chunks.len(),
        snapshot.steps.len()
    );
    Ok(())
}

/// Public conversation events use the same Harbor scenario as the roadmap.
fn turn(model: &mut Model, data: &Value) -> Result<(), Box<dyn Error>> {
    let text = |key: &str| -> Result<&str, Box<dyn Error>> {
        data[key]
            .as_str()
            .ok_or_else(|| format!("missing Turn field: {key}").into())
    };
    for event in [
        AgentEvent::UserText(text("prompt")?.into()),
        AgentEvent::Text(text("response")?.into()),
        AgentEvent::ToolCall {
            id: "harbor-read".into(),
            title: text("read_title")?.into(),
            verb: ToolVerb::Read,
            status: ToolStatus::Completed,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        },
        AgentEvent::ToolCall {
            id: "harbor-edit".into(),
            title: format!("Edit {}", text("edit_path")?),
            verb: ToolVerb::Edit,
            status: ToolStatus::Completed,
            diff: Some(EditDiff {
                path: text("edit_path")?.into(),
                old_text: text("old_text")?.into(),
                new_text: text("new_text")?.into(),
            }),
            images: Vec::new(),
            record: Default::default(),
        },
        AgentEvent::Text(text("edit_summary")?.into()),
        AgentEvent::ToolCall {
            id: "harbor-test".into(),
            title: text("command")?.into(),
            verb: ToolVerb::Run,
            status: ToolStatus::Completed,
            diff: None,
            images: Vec::new(),
            record: Box::new(ToolRecord {
                input: Some(serde_json::json!({"command": text("command")?})),
                output: Some(serde_json::json!({"passed": 3, "failed": 0})),
                terminal: Some("harbor-test-terminal".into()),
                ..ToolRecord::default()
            }),
        },
        AgentEvent::Terminal {
            terminal_id: "harbor-test-terminal".into(),
            bytes: text("terminal")?.as_bytes().to_vec(),
        },
    ] {
        update(model, Msg::Agent(event));
    }
    // Open the test terminal, then select the edit immediately before it.
    // The test output stays in Turn while the shared inspector shows the diff.
    for key in [KeyCode::Enter, KeyCode::Up, KeyCode::Enter] {
        update(model, Msg::Key(KeyEvent::new(key)));
    }
    Ok(())
}

/// A left click on the widget — the same message the runtime posts.
fn click(model: &mut Model, target: magistr_cockpit::widget_id::WidgetId) {
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 40, 20),
            target: Some(target),
        },
    );
}
