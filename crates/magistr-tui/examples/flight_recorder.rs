//! Flight Recorder design study, rendered by TUI2; synthetic Harbor data only.
//!
//! cargo run -p magistr-tui --example flight_recorder -- <output-directory>

use magistr_cockpit::cockpit::{
    FrameClock, Model, Msg, Selected, Size, StageTab, Surface, Widget, update,
};
use magistr_cockpit::harness::fixtures;
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
use magistr_cockpit::layout::{Align, Length, Mark, Node, TextRole};
use magistr_cockpit::render::{Attrs, Color, Style};
use magistr_cockpit::theme::{Pen, Theme, ThemeMode};
use magistr_engine::NativeSnapshot;
use magistr_tui::screenshot::Preview;
use serde_json::{Value, json};
use std::{error::Error, path::PathBuf, time::Duration};

const INK: Color = Color::Rgb(224, 231, 241);
const MUTED: Color = Color::Rgb(158, 174, 193);
const ACCENT: Color = Color::Rgb(151, 191, 255);
const GOOD: Color = Color::Rgb(127, 217, 177);
const LINE: Color = Color::Rgb(53, 66, 84);
const RAISED: Color = Color::Rgb(27, 37, 52);

fn main() -> Result<(), Box<dyn Error>> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("expected output directory")?,
    );
    std::fs::create_dir_all(&output)?;
    let data: Value =
        serde_json::from_str(include_str!("../../../apps/website/fixtures/harbor.json"))?;
    let mut manifest = Vec::new();
    for size in [Size::new(144, 50), Size::new(128, 40)] {
        let mut model = model(size, &data)?;
        for number in [12, 9, 10, 4] {
            let index = model
                .trace()
                .steps()
                .iter()
                .position(|s| s.number == number)
                .ok_or("missing step")?;
            update(
                &mut model,
                Msg::Mouse {
                    event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                    target: Some(Widget::trace_row(index)),
                },
            );
            if model.selected() != Some(&Selected::Step(number)) {
                return Err("native selection did not resolve to the requested record".into());
            }
            let clock = FrameClock::new(8, Duration::from_millis(1600));
            let pixels = Preview::frame(&model, &clock, view)?;
            let name = format!("flight-{}-step-{number}.png", size.cols * 10);
            pixels.save_png(output.join(&name))?;
            let resolved = Preview::layout(&model, &clock, view)?;
            let mut areas = Vec::new();
            for (key, id) in [
                ("turn", Widget::Stage.id()),
                ("roadmap", Widget::RailGraph.id()),
                ("record", Widget::Inspector.id()),
                ("reasoning", Widget::TraceGraph.id()),
                ("composer", Widget::Prompt.id()),
                ("record-reference", Widget::InspectorFields.id()),
                ("send", Widget::PromptAction.id()),
                ("latest", Widget::Checkpoint.id()),
                ("step4", Widget::trace_row(3)),
                ("step9", Widget::trace_row(8)),
                ("step10", Widget::trace_row(9)),
                ("step12", Widget::trace_row(11)),
            ] {
                let rect = resolved
                    .placed()
                    .iter()
                    .find(|p| p.id == Some(id))
                    .ok_or("missing study region")?
                    .px;
                if rect.width <= 0.0
                    || rect.height <= 0.0
                    || rect.right() > pixels.width() as f32 + 0.1
                    || rect.bottom() > pixels.height() as f32 + 0.1
                {
                    return Err(format!("out-of-bounds {key}: {rect:?}").into());
                }
                areas.push(json!({"key":key,"x":rect.x,"y":rect.y,"width":rect.width,"height":rect.height}));
            }
            manifest.push(json!({"file":name,"width":pixels.width(),"height":pixels.height(),"step":number,"areas":areas}));
        }
    }
    std::fs::write(
        output.join("native-captures.json"),
        serde_json::to_vec_pretty(&json!({
            "renderer":"TUI2 runtime Surfaces → Screen → Kitty wire → decoded PNG",
            "synthetic":true,"production_shell":false,"captures":manifest
        }))?,
    )?;
    Ok(())
}

fn model(size: Size, data: &Value) -> Result<Model, Box<dyn Error>> {
    let records = |key: &str| {
        data[key]
            .as_array()
            .cloned()
            .ok_or("missing fixture records")
    };
    let mut chunks = records("chunks")?;
    // A local roadmap neighbourhood, preserving each edge between visible chunks.
    let ids = [
        "attempts",
        "leases",
        "retry-budget",
        "retry-after",
        "backoff",
    ];
    chunks.retain(|c| ids.iter().any(|id| c["id"] == *id));
    let included: Vec<_> = chunks
        .iter()
        .filter_map(|c| c["id"].as_str().map(str::to_owned))
        .collect();
    for chunk in &mut chunks {
        if let Some(deps) = chunk["deps"].as_array_mut() {
            deps.retain(|d| {
                d.as_str()
                    .is_some_and(|id| included.iter().any(|s| s == id))
            });
        }
    }
    let snapshot = NativeSnapshot {
        chunks: fixtures::chunks_from(chunks),
        steps: fixtures::steps_from(records("steps")?),
        tasks: records("tasks")?
            .into_iter()
            .map(fixtures::task_from)
            .collect(),
        objective: Some(fixtures::objective_from(data["objective"].clone())),
        ..NativeSnapshot::default()
    };
    let mut model = Model::new(size)
        .with_surface(Surface::Terminal)
        .with_title("harbor")
        .with_snapshot(&snapshot)
        .with_theme(Theme::forced(ThemeMode::Dark))
        .with_tab(StageTab::Session);
    for branch in records("branches")? {
        update(
            &mut model,
            Msg::Engine(fixtures::event("think", "branch_status_changed", branch)),
        );
    }
    for _ in 0..8 {
        update(&mut model, Msg::Tick);
    }
    Ok(model)
}

fn view(model: &Model, _: &FrameClock) -> Node {
    let background = Pen::for_theme(&model.theme()).stage.bg;
    let number = match model.selected() {
        Some(Selected::Step(n)) => *n,
        _ => 12,
    };
    let right_width = if model.size().cols >= 140 {
        440.0
    } else {
        400.0
    };
    let header = Node::row()
        .height(px(40.0))
        .shrink(0.0)
        .align_items(Align::Center)
        .child(label("MAGISTR", INK).style(style(INK).attrs(Attrs::BOLD)))
        .child(hspace(24.0))
        .child(label("Harbor  /  Reliable retries", MUTED))
        .child(flex())
        .child(chip("Checking", GOOD, background))
        .child(hspace(14.0))
        .child(label("Today  ·  14:00", MUTED));
    let right = Node::column()
        .width(px(right_width))
        .shrink(0.0)
        .child(roadmap(background))
        .child(vspace(16.0))
        .child(record(model, number, background).grow(1.0));
    Node::column()
        .padding_xy(2, 1)
        .style(style(INK).bg(background))
        .child(header)
        .child(vspace(12.0))
        .child(
            Node::row()
                .grow(1.0)
                .min_height(px(0.0))
                .child(
                    session(background, model.size().rows < 45)
                        .grow(1.0)
                        .min_width(px(0.0)),
                )
                .child(hspace(24.0))
                .child(right),
        )
        .child(vspace(16.0))
        .child(recorder(model, number, background))
        .child(vspace(16.0))
        .child(composer(background))
}

fn session(background: Color, compact: bool) -> Node {
    let session_heading = Node::row()
        .align_items(Align::Center)
        .child(heading("Bound retries across worker restarts"))
        .child(flex())
        .child(chip("Working", GOOD, background));
    let mut conversation = Node::column().viewport().grow(1.0)
        .child(label("You  ·  14:00", MUTED)).child(vspace(8.0))
        .child(prose("Make the retry budget survive worker restarts. Keep tenant budgets isolated and show me the recovery checks."))
        .child(vspace(24.0))
        .child(label("Magistr  ·  14:18", ACCENT)).child(vspace(8.0))
        .child(prose("The budget now lives beside the lease. A worker resumes from the stored next-attempt timestamp; a restart cannot create another allowance."))
        .child(vspace(12.0))
        .child(prose("Tenant isolation stays intact. I rejected the shared-pool variant after the latency check and merged the separate-counter approach."))
        .child(vspace(12.0))
        .child(Node::row().child(chip("Decision 10 · saved", ACCENT, background))
            .child(hspace(8.0)).child(chip("Decision 9 updated decision 8", MUTED, background)))
        .child(vspace(24.0))
        .child(on_ground(Node::column().padding_xy(2, 1).style(style(INK).bg(RAISED))
            .child(Node::row().child(heading("Persist retry state")).child(flex()).child(label("+34  −11", GOOD)))
            .child(vspace(6.0)).child(label("src/delivery/retry.rs", MUTED))
            .child(vspace(10.0)).child(prose("The receipt, next due time and tenant counter commit in one transaction.")), RAISED))
        .child(vspace(20.0))
        .child(label("Magistr  ·  14:20", ACCENT)).child(vspace(8.0))
        .child(prose("The focused retry checks pass. I am checking the restart interleavings before recording the final receipt."))
        .child(vspace(12.0))
        .child(Node::row().child(chip("3 checks passed", GOOD, background))
            .child(hspace(8.0)).child(chip("Restart checks · next", MUTED, background)))
        .child(vspace(20.0));
    if compact {
        conversation = Node::column().viewport().grow(1.0)
            .child(label("You · Make retries durable and keep tenant budgets isolated.", MUTED))
            .child(vspace(16.0))
            .child(label("Magistr · 14:18", ACCENT)).child(vspace(6.0))
            .child(prose("The budget, receipt and next due time now commit together. A restart resumes the existing allowance; each tenant keeps its own counter."))
            .child(vspace(12.0))
            .child(Node::row().child(chip("Decision 10 · saved", ACCENT, background)).child(hspace(8.0)).child(label("src/delivery/retry.rs  +34 −11", GOOD)))
            .child(vspace(20.0))
            .child(label("Magistr · 14:20", ACCENT)).child(vspace(6.0))
            .child(prose("The focused checks pass. I am checking the restart interleavings before recording the final receipt."))
            .child(vspace(12.0))
            .child(Node::row().child(chip("3 checks passed", GOOD, background)).child(hspace(8.0)).child(label("Restart checks · next", MUTED)));
    }
    Node::column()
        .id(Widget::Stage.id())
        .min_height(px(0.0))
        .child(session_heading)
        .child(vspace(8.0))
        .child(label(
            "Checking restart recovery  ·  12 decisions recorded",
            MUTED,
        ))
        .child(vspace(16.0))
        .child(rule())
        .child(vspace(20.0))
        .child(conversation)
}

fn roadmap(background: Color) -> Node {
    Node::column()
        .height(px(140.0))
        .shrink(0.0)
        .child(
            Node::row()
                .child(heading("Roadmap"))
                .child(flex())
                .child(label("Delivery progress", MUTED)),
        )
        .child(vspace(8.0))
        .child(
            Node::leaf()
                .id(Widget::RailGraph.id())
                .height(px(60.0))
                .style(style(INK).bg(background)),
        )
        .child(vspace(8.0))
        .child(
            Node::row()
                .child(chip("Reliable retries · in progress", ACCENT, background))
                .child(flex()),
        )
        .child(vspace(4.0))
        .child(label("Storage ready → Retry limits → Retry timing", MUTED))
}

fn record(model: &Model, number: u32, background: Color) -> Node {
    let Some(step) = model.trace().step(number) else {
        return label("No recorded decision", MUTED);
    };
    let title = format!("{:02}  {}", step.number, step.purpose);
    let relation = match step.revises {
        Some(n) => format!(
            "Revises decision {n}  ·  {}",
            step.branch_name.as_deref().unwrap_or("Main")
        ),
        None => format!(
            "{}  ·  {:.0}% confidence{}",
            step.branch_name.as_deref().unwrap_or("Main"),
            step.confidence.unwrap_or(0.0) * 100.0,
            if step.pinned { "  ·  Pinned" } else { "" }
        ),
    };
    Node::column()
        .id(Widget::Inspector.id())
        .min_height(px(0.0))
        .viewport()
        .child(rule())
        .child(vspace(8.0))
        .child(heading(title))
        .child(vspace(6.0))
        .child(label(relation, MUTED))
        .child(vspace(4.0))
        .child(prose(&step.thought))
        .child(vspace(8.0))
        .child(label("Outcome", MUTED))
        .child(vspace(4.0))
        .child(prose(&step.outcome))
        .child(vspace(8.0))
        .child(
            chip(
                match step.execution_ref.as_deref() {
                    Some("task:retry-implementation") => "Work: Save the retry schedule",
                    Some("task:retry-tests") => "Work: Check restart recovery",
                    Some("task:retry-plan") => "Work: Set the restart rules",
                    _ => "No linked work",
                },
                ACCENT,
                background,
            )
            .id(Widget::InspectorFields.id()),
        )
}

fn recorder(model: &Model, number: u32, background: Color) -> Node {
    let tiles = [
        (4, "14:06", "Throttle policy"),
        (9, "14:16", "Reject shared pools"),
        (10, "14:18", "Separate budgets"),
        (12, "14:20", "Recovery checks"),
    ];
    let mut events = Node::row().height(px(62.0)).shrink(0.0);
    for (index, (n, time, title)) in tiles.into_iter().enumerate() {
        if index > 0 {
            events = events.child(hspace(12.0));
        }
        let ink = if number == n { ACCENT } else { MUTED };
        let target = model
            .trace()
            .steps()
            .iter()
            .position(|s| s.number == n)
            .unwrap_or(0);
        events = events.child(
            Node::column()
                .grow(1.0)
                .basis(px(0.0))
                .padding_xy(1, 0)
                .style(style(ink).bg(if number == n { RAISED } else { background }))
                .id(Widget::trace_row(target))
                .child(vspace(8.0))
                .child(
                    label(format!("{time}   ·   Decision {n:02}"), ink)
                        .style(style(ink).bg(if number == n { RAISED } else { background })),
                )
                .child(vspace(4.0))
                .child(
                    heading(title).style(
                        style(INK)
                            .bg(if number == n { RAISED } else { background })
                            .attrs(Attrs::BOLD),
                    ),
                ),
        );
    }
    Node::column()
        .height(px(200.0))
        .shrink(0.0)
        .child(rule())
        .child(vspace(12.0))
        .child(
            Node::row()
                .child(heading("Flight recorder"))
                .child(hspace(20.0))
                .child(label(
                    "Retry ownership  ·  Server delays  ·  Customer limits",
                    MUTED,
                ))
                .child(flex())
                .child(
                    chip(
                        if number == 12 {
                            "Latest decision"
                        } else {
                            "Return to latest →"
                        },
                        ACCENT,
                        background,
                    )
                    .id(Widget::Checkpoint.id()),
                ),
        )
        .child(vspace(8.0))
        .child(
            Node::leaf()
                .id(Widget::TraceGraph.id())
                .height(px(60.0))
                .shrink(0.0)
                .style(style(INK).bg(background)),
        )
        .child(vspace(10.0))
        .child(events)
        .child(vspace(7.0))
        .child(label(
            "12 decisions  ·  2 alternatives considered  ·  1 decision revised",
            MUTED,
        ))
}

fn composer(background: Color) -> Node {
    on_ground(
        Node::column()
            .height(px(100.0))
            .shrink(0.0)
            .padding_xy(2, 0)
            .mark(Mark::Panel {
                border: LINE,
                title: None,
            })
            .style(style(INK).bg(RAISED))
            .child(vspace(10.0))
            .child(
                Node::row()
                    .child(label("Message Magistr", INK))
                    .child(hspace(16.0))
                    .child(label("Reliable retries", MUTED))
                    .child(flex())
                    .child(label("Draft", MUTED)),
            )
            .child(vspace(8.0))
            .child(
                Node::row()
                    .align_items(Align::Center)
                    .child(
                        Node::editor(
                            "Also test a restart immediately after the receipt commits.",
                            58,
                        )
                        .id(Widget::Prompt.id())
                        .text_role(TextRole::Steer)
                        .grow(1.0)
                        .min_width(px(0.0)),
                    )
                    .child(hspace(20.0))
                    .child(chip("Send  ↑", ACCENT, background).id(Widget::PromptAction.id())),
            )
            .child(vspace(10.0))
            .child(label(
                "Enter to send    ·    Shift + Enter for a new line    ·    Add files",
                MUTED,
            )),
        RAISED,
    )
}

fn px(value: f32) -> Length {
    Length::Px(value)
}
fn style(ink: Color) -> Style {
    Style::new().fg(ink)
}
fn label(text: impl Into<String>, ink: Color) -> Node {
    Node::text(text)
        .text_role(TextRole::Label)
        .style(style(ink))
        .min_height(px(20.0))
        .shrink(0.0)
}
fn heading(text: impl Into<String>) -> Node {
    label(text, INK)
        .text_role(TextRole::Heading)
        .style(style(INK).attrs(Attrs::BOLD))
}
fn prose(text: impl Into<String>) -> Node {
    Node::text(text)
        .text_role(TextRole::Prose)
        .style(style(INK))
        .min_width(px(0.0))
        .shrink(0.0)
}
fn chip(text: impl Into<String>, ink: Color, _background: Color) -> Node {
    label(text, ink)
        .padding_xy(1, 0)
        .style(style(ink).bg(RAISED))
}
fn on_ground(mut node: Node, color: Color) -> Node {
    node.restyle_all(&|s| s.clone().bg(color));
    node
}
fn flex() -> Node {
    Node::leaf().grow(1.0).basis(px(0.0))
}
fn hspace(width: f32) -> Node {
    Node::leaf().width(px(width)).shrink(0.0)
}
fn vspace(height: f32) -> Node {
    Node::leaf().height(px(height)).shrink(0.0)
}
fn rule() -> Node {
    Node::leaf()
        .height(px(1.0))
        .shrink(0.0)
        .style(style(LINE).bg(LINE))
}
