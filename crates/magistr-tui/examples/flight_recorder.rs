//! Flight Recorder design study, rendered by TUI2; synthetic Harbor data only.
//!
//! cargo run -p magistr-tui --example flight_recorder -- <output-directory>

use magistr_cockpit::cockpit::{
    FrameClock, Model, Msg, Selected, Size, StageTab, Surface, Widget, update,
};
use magistr_cockpit::harness::fixtures;
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
use magistr_cockpit::layout::{Align, Justify, Length, Node, TextRole};
use magistr_cockpit::render::{Attrs, Color, Style};
use magistr_cockpit::theme::{Theme, ThemeMode};
use magistr_engine::NativeSnapshot;
use magistr_tui::screenshot::Preview;
use serde_json::{Value, json};
use std::{error::Error, path::PathBuf, time::Duration};

const INK: Color = Color::Rgb(224, 231, 241);
const MUTED: Color = Color::Rgb(158, 174, 193);
const ACCENT: Color = Color::Rgb(151, 191, 255);
const GOOD: Color = Color::Rgb(127, 217, 177);
const LINE: Color = Color::Rgb(76, 76, 98);
const ROOT: Color = Color::Rgb(17, 17, 27);
const PANEL: Color = Color::Rgb(30, 30, 46);
const HEADER: Color = Color::Rgb(39, 39, 57);
const RAISED: Color = Color::Rgb(45, 45, 65);
const SELECTED: Color = Color::Rgb(49, 65, 92);

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
        for number in 1..=12 {
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
                ("step1", Widget::trace_row(0)),
                ("step2", Widget::trace_row(1)),
                ("step3", Widget::trace_row(2)),
                ("step4", Widget::trace_row(3)),
                ("step5", Widget::trace_row(4)),
                ("step6", Widget::trace_row(5)),
                ("step7", Widget::trace_row(6)),
                ("step8", Widget::trace_row(7)),
                ("step9", Widget::trace_row(8)),
                ("step10", Widget::trace_row(9)),
                ("step11", Widget::trace_row(10)),
                ("step12", Widget::trace_row(11)),
                ("conversation-panel", Widget::Stage.id()),
                ("roadmap-panel", area_id(1)),
                ("decision-panel", Widget::Inspector.id()),
                ("recorder-panel", area_id(3)),
                ("message-panel", area_id(4)),
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
            manifest.push(json!({"file":name,"width":pixels.width(),"height":pixels.height(),"step":number,"title":model.trace().step(number).map(|s| &s.purpose),"areas":areas}));
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
    let number = match model.selected() {
        Some(Selected::Step(n)) => *n,
        _ => 12,
    };
    let right_width = if model.size().cols >= 140 {
        440.0
    } else {
        408.0
    };
    let header = on_ground(
        Node::row()
            .height(px(28.0))
            .shrink(0.0)
            .align_items(Align::Center)
            .child(heading("MAGISTR"))
            .child(hspace(24.0))
            .child(label("Harbor  /  Reliable retries", MUTED))
            .child(flex())
            .child(label("Checking", GOOD))
            .child(hspace(20.0))
            .child(label("Today  ·  14:00", MUTED)),
        ROOT,
    );
    let right = Node::column()
        .width(px(right_width))
        .shrink(0.0)
        .child(roadmap())
        .child(vspace(12.0))
        .child(record(model, number).grow(1.0));
    let content = Node::column()
        .style(style(INK).bg(ROOT))
        .child(header)
        .child(vspace(12.0))
        .child(
            Node::row()
                .grow(1.0)
                .min_height(px(0.0))
                .child(session(model.size().rows < 45).grow(1.0).min_width(px(0.0)))
                .child(hspace(12.0))
                .child(right),
        )
        .child(vspace(12.0))
        .child(recorder(model, number))
        .child(vspace(12.0))
        .child(composer());
    inset(content, 20.0, 12.0).style(style(INK).bg(ROOT))
}

fn session(compact: bool) -> Node {
    let background = PANEL;
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
            .child(Node::row().child(heading("Save the retry schedule")).child(flex()).child(label("+34  −11", GOOD)))
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

    let cap = section_heading("Conversation", label("Working", GOOD));
    let contents = Node::column().min_height(px(0.0)).child(cap).child(
        inset(
            Node::column()
                .min_height(px(0.0))
                .child(heading("Bound retries across worker restarts"))
                .child(vspace(4.0))
                .child(label(
                    "Checking restart recovery  ·  12 decisions recorded",
                    MUTED,
                ))
                .child(vspace(16.0))
                .child(conversation),
            20.0,
            14.0,
        )
        .grow(1.0),
    );
    outlined(contents, PANEL, LINE).id(Widget::Stage.id())
}

fn roadmap() -> Node {
    let graph = Node::leaf()
        .id(Widget::RailGraph.id())
        .height(px(60.0))
        .shrink(0.0)
        .style(style(INK).bg(PANEL));
    let labels = Node::row()
        .justify_content(Justify::SpaceBetween)
        .child(label("Storage", GOOD))
        .child(label("Retry limits", ACCENT))
        .child(label("Retry timing", MUTED));
    outlined(
        Node::column()
            .child(section_heading("Roadmap", label("Delivery", MUTED)))
            .child(
                inset(
                    Node::column()
                        .child(vspace(5.0))
                        .child(graph)
                        .child(vspace(4.0))
                        .child(labels),
                    19.0,
                    8.0,
                )
                .grow(1.0),
            ),
        PANEL,
        LINE,
    )
    .id(area_id(1))
    .height(px(148.0))
    .shrink(0.0)
}

fn record(model: &Model, number: u32) -> Node {
    let Some(step) = model.trace().step(number) else {
        return label("No recorded decision", MUTED);
    };
    let relation = match step.revises {
        Some(n) => format!(
            "Updates decision {n}  ·  {}",
            step.branch_name.as_deref().unwrap_or("Main")
        ),
        None => format!(
            "{:.0}% confidence{}",
            step.confidence.unwrap_or(0.0) * 100.0,
            if step.pinned { "  ·  Saved" } else { "" }
        ),
    };
    let linked_work = match step.execution_ref.as_deref() {
        Some("task:retry-implementation") => "Save the retry schedule",
        Some("task:retry-tests") => "Check restart recovery",
        Some("task:retry-plan") => "Set the restart rules",
        _ => "No linked work",
    };
    let mut body = Node::column()
        .viewport()
        .child(heading(&step.purpose))
        .child(vspace(4.0))
        .child(label(relation, MUTED))
        .child(vspace(8.0))
        .child(prose(&step.thought))
        .child(vspace(8.0))
        .child(prose(format!("Outcome: {}", step.outcome)));
    if model.size().rows >= 45 {
        body = body
            .child(vspace(18.0))
            .child(heading("Why"))
            .child(vspace(4.0))
            .child(prose(&step.rationale))
            .child(vspace(16.0))
            .child(heading("Next"))
            .child(vspace(4.0))
            .child(prose(&step.next_action));
    }
    let body = body.child(flex()).child(vspace(8.0)).child(
        Node::row()
            .child(
                chip(format!("Work: {linked_work}  →"), ACCENT, PANEL)
                    .id(Widget::InspectorFields.id()),
            )
            .child(flex()),
    );
    outlined(
        Node::column()
            .min_height(px(0.0))
            .child(section_heading(
                format!("Decision {number:02}"),
                label(if number == 12 { "Latest" } else { "Selected" }, ACCENT),
            ))
            .child(inset(body, 16.0, 10.0).grow(1.0)),
        PANEL,
        LINE,
    )
    .id(Widget::Inspector.id())
    .min_height(px(0.0))
}

fn recorder(model: &Model, number: u32) -> Node {
    let mut numbers = Node::row()
        .justify_content(Justify::SpaceBetween)
        .height(px(28.0))
        .shrink(0.0);
    for (index, step) in model.trace().steps().iter().enumerate() {
        let selected = step.number == number;
        let fill = if selected { SELECTED } else { PANEL };
        let item = on_ground(
            Node::row()
                .align_items(Align::Center)
                .justify_content(Justify::Center)
                .child(label(
                    format!("{:02}", step.number),
                    if selected { INK } else { MUTED },
                )),
            fill,
        );
        numbers = numbers.child(
            outlined(item, fill, if selected { ACCENT } else { LINE })
                .width(px(32.0))
                .id(Widget::trace_row(index)),
        );
    }
    // BranchGraph's 60px height gives it 27px side padding.
    // Each 32px button is centered exactly below its own graph point.
    let numbered = Node::row()
        .child(hspace(11.0))
        .child(numbers.grow(1.0))
        .child(hspace(11.0));
    outlined(
        Node::column()
            .child(section_heading(
                "Flight recorder",
                chip(
                    if number == 12 {
                        "Latest decision"
                    } else {
                        "Return to latest →"
                    },
                    ACCENT,
                    HEADER,
                )
                .id(Widget::Checkpoint.id()),
            ))
            .child(
                inset(
                    Node::column()
                        .child(vspace(13.0))
                        .child(
                            Node::leaf()
                                .id(Widget::TraceGraph.id())
                                .height(px(60.0))
                                .shrink(0.0)
                                .style(style(INK).bg(PANEL)),
                        )
                        .child(numbered),
                    19.0,
                    4.0,
                )
                .grow(1.0),
            ),
        PANEL,
        LINE,
    )
    .id(area_id(3))
    .height(px(148.0))
    .shrink(0.0)
}

fn composer() -> Node {
    let draft = "Also test a restart immediately after the receipt commits.";
    let mut entry = on_ground(
        inset(
            Node::row()
                .align_items(Align::Center)
                .child(
                    Node::editor(draft, draft.len())
                        .id(Widget::Prompt.id())
                        .text_role(TextRole::Steer)
                        .grow(1.0)
                        .min_width(px(0.0)),
                )
                .child(hspace(16.0))
                .child(
                    on_ground(chip("Send  ↑", INK, SELECTED), SELECTED)
                        .id(Widget::PromptAction.id()),
                ),
            12.0,
            5.0,
        ),
        RAISED,
    );
    entry.restyle_at(Widget::PromptAction.id(), &|s| s.clone().bg(SELECTED));
    let body = Node::column()
        .child(outlined(entry, RAISED, LINE).height(px(36.0)).shrink(0.0))
        .child(vspace(6.0))
        .child(label(
            "Enter to send  ·  Shift + Enter for a new line  ·  Add files",
            MUTED,
        ));
    outlined(
        Node::column()
            .child(section_heading(
                "Message Magistr",
                label("Reliable retries  ·  Draft", MUTED),
            ))
            .child(inset(body, 16.0, 6.0).grow(1.0)),
        PANEL,
        LINE,
    )
    .id(area_id(4))
    .height(px(112.0))
    .shrink(0.0)
}

fn area_id(index: u32) -> magistr_cockpit::widget_id::WidgetId {
    magistr_cockpit::widget_id::WidgetId::new(900_000 + index)
}
fn px(value: f32) -> Length {
    Length::Px(value)
}
fn style(ink: Color) -> Style {
    Style::new().fg(ink).bg(PANEL)
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
fn chip(text: impl Into<String>, ink: Color, background: Color) -> Node {
    on_ground(
        inset(label(text, ink), 10.0, 2.0),
        if background == HEADER { HEADER } else { RAISED },
    )
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
fn inset(content: Node, x: f32, y: f32) -> Node {
    Node::column()
        .min_width(px(0.0))
        .min_height(px(0.0))
        .style(style(INK))
        .child(vspace(y))
        .child(
            Node::row()
                .grow(1.0)
                .min_height(px(0.0))
                .child(hspace(x))
                .child(content.grow(1.0).min_width(px(0.0)).min_height(px(0.0)))
                .child(hspace(x)),
        )
        .child(vspace(y))
}
fn outlined(content: Node, fill: Color, border: Color) -> Node {
    Node::column()
        .min_width(px(0.0))
        .min_height(px(0.0))
        .style(style(INK).bg(border))
        .child(vspace(1.0))
        .child(
            Node::row()
                .grow(1.0)
                .min_height(px(0.0))
                .child(hspace(1.0))
                .child(
                    content
                        .grow(1.0)
                        .min_width(px(0.0))
                        .min_height(px(0.0))
                        .style(style(INK).bg(fill)),
                )
                .child(hspace(1.0)),
        )
        .child(vspace(1.0))
}
fn section_heading(title: impl Into<String>, aside: Node) -> Node {
    on_ground(
        inset(
            Node::row()
                .align_items(Align::Center)
                .child(heading(title))
                .child(flex())
                .child(aside),
            16.0,
            0.0,
        ),
        HEADER,
    )
    .height(px(34.0))
    .shrink(0.0)
}
