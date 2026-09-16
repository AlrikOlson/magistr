//! Website images rendered from fictional records through the production view.
//! cargo run -p magistr-tui --example website_capture -- <fixture.json> <out-dir>
//! scripts/website-captures.sh writes the accompanying image manifest.

use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

#[path = "support/harbor.rs"]
mod harbor;
use magistr_cockpit::cockpit::{FrameClock, Model, Msg, Size, StageTab, Surface, Widget, update};
use magistr_cockpit::harness::fixtures;
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
use magistr_cockpit::theme::{Theme, ThemeMode};
use magistr_engine::NativeSnapshot;
use magistr_engine::native::ChunkStatus;
use magistr_tui::graphics::{Decision, Passthrough, Protocol};
use magistr_tui::screenshot::Preview;
use serde_json::Value;

/// The approved large viewport, including every area and the complete message boundary.
const CAPTURE: Size = Size::new(144, 50);

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
                .with_tab(StageTab::Turn);
            update(
                &mut model,
                Msg::Mode(Decision::Graphics(Protocol::Kitty(Passthrough::Direct)).mode()),
            );
            turn(&mut model, &data["turn"], name == "turn")?;
            update(
                &mut model,
                Msg::Paste("Also test a restart immediately after the receipt commits.".into()),
            );
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
                (_, StageTab::Turn) => {}
                (_, StageTab::Session) => {
                    // The step two before the newest: its card, its
                    // relations and the branch it stands on all show.
                    let index = snapshot.steps.len().saturating_sub(3);
                    click(&mut model, Widget::trace_row(index));
                }
                ("roadmap", _) => {
                    // The chunk in progress heads the list: selecting it
                    // opens its card and lights its dependency chain.
                    click(&mut model, Widget::TabRoadmap.id());
                    click(&mut model, Widget::roadmap_row(0));
                    if model.tab() != StageTab::Roadmap
                        || !matches!(
                            model.selected(),
                            Some(magistr_cockpit::cockpit::Selected::Chunk(_))
                        )
                    {
                        return Err("roadmap work did not remain on the Roadmap page".into());
                    }
                }
                // The overview follows the newest recorded decision.
                _ => {
                    click(&mut model, Widget::LatestDecision.id());
                }
            }
            for _ in 0..8 {
                update(&mut model, Msg::Tick);
            }
            let pixels = Preview::frame(
                &model,
                &FrameClock::new(8, Duration::from_millis(1600)),
                magistr_cockpit::cockpit::view,
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
fn turn(model: &mut Model, data: &Value, detailed: bool) -> Result<(), Box<dyn Error>> {
    harbor::conversation(model, data)?;
    if detailed {
        // Open the real terminal and edit through the visible conversation cards.
        for call in ["harbor-test", "harbor-edit"] {
            let source = model
                .turn()
                .lines()
                .iter()
                .find_map(|line| match line.kind() {
                    magistr_cockpit::turn::LineKind::Tool { id, .. }
                    | magistr_cockpit::turn::LineKind::Edit { id, .. }
                        if id == call =>
                    {
                        Some(line.row_start())
                    }
                    _ => None,
                })
                .ok_or("missing Harbor tool card")?;
            let visible = source
                .checked_sub(model.top_row())
                .ok_or("Harbor card above viewport")?;
            click(model, Widget::stage_row(visible));
        }
        // Scroll the actual conversation until the expanded edit's heading is visible.
        // The capture retains the real wheel distance and scroll anchoring.
        let source = model
            .turn()
            .lines()
            .iter()
            .find_map(|line| match line.kind() {
                magistr_cockpit::turn::LineKind::Edit { id, .. } if id == "harbor-edit" => {
                    Some(line.row_start())
                }
                _ => None,
            })
            .ok_or("missing Harbor edit")?;
        let heading = model.turn().conversation().offset(source) as f32;
        for _ in 0..30 {
            if model.conversation_top() <= heading {
                break;
            }
            let resolved = Preview::layout(
                model,
                &FrameClock::new(8, Duration::from_millis(1600)),
                magistr_cockpit::cockpit::view,
            )?;
            let stage = resolved
                .placed()
                .iter()
                .find(|placed| placed.id == Some(Widget::Stage.id()))
                .ok_or("missing conversation")?
                .px;
            let x = stage.x + 3.0;
            let y = stage.y + 1.0;
            let mut hits = magistr_cockpit::hit::HitRegistry::new();
            hits.register_layout_px(&resolved, 0);
            let target = hits.hit_at(f64::from(x), f64::from(y));
            update(
                model,
                Msg::Mouse {
                    event: MouseEvent::new(MouseKind::ScrollUp, 0, 0).at_px(x as u32, y as u32),
                    target,
                },
            );
            for _ in 0..8 {
                update(model, Msg::Tick);
            }
        }
        if model.conversation_top() > heading {
            return Err("expanded edit heading stayed above the conversation".into());
        }
        if !model.reveal_conversation(source) {
            return Err("expanded edit is no longer in the conversation".into());
        }
    }
    Ok(())
}

/// Click a visible control through the same physical hit geometry as the runtime.
fn click(model: &mut Model, id: magistr_cockpit::widget_id::WidgetId) {
    let clock = FrameClock::new(8, Duration::from_millis(1600));
    let resolved =
        Preview::layout(model, &clock, magistr_cockpit::cockpit::view).expect("capture layout");
    let placed = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(id))
        .expect("capture control must be visible");
    let x = placed.px.x + placed.px.width / 2.0;
    let y = placed.px.y + placed.px.height / 2.0;
    let mut hits = magistr_cockpit::hit::HitRegistry::new();
    hits.register_layout_px(&resolved, 0);
    let target = hits.hit_at(f64::from(x), f64::from(y));
    assert_eq!(target, Some(id), "capture control is reachable");
    let cell = model.cell_size();
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(
                MouseKind::Down(Button::Left),
                (x / f32::from(cell.width)) as u16,
                (y / f32::from(cell.height)) as u16,
            )
            .at_px(x as u32, y as u32),
            target,
        },
    );
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Up(Button::Left), 0, 0).at_px(x as u32, y as u32),
            target,
        },
    );
}
