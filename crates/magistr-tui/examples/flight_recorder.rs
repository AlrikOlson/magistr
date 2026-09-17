//! Production Flight Recorder captures from fictional Harbor records.
//! cargo run -p magistr-tui --example flight_recorder -- <output-directory>

#[path = "support/harbor.rs"]
mod harbor;
#[path = "support/requests.rs"]
mod requests;

use magistr_application::sessions::{
    AgentEvent, AskPlan, PermissionChoice, PermissionKind, TurnEnd,
};
use magistr_cockpit::cockpit::{
    FrameClock, Model, Msg, Selected, Size, StageTab, Surface, Widget, update, view,
};
use magistr_cockpit::harness::fixtures;
use magistr_cockpit::hit::HitRegistry;
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use magistr_cockpit::theme::{Theme, ThemeMode};
use magistr_cockpit::widget_id::WidgetId;
use magistr_engine::NativeSnapshot;
use magistr_engine::policy::RiskTier;
use magistr_tui::graphics::{Decision, Passthrough, Protocol};
use magistr_tui::screenshot::Preview;
use serde_json::{Value, json};
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Duration,
};

fn main() -> Result<(), Box<dyn Error>> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("expected output directory")?,
    );
    std::fs::create_dir_all(&output)?;
    let data: Value =
        serde_json::from_str(include_str!("../../../apps/website/fixtures/harbor.json"))?;
    if data["synthetic"] != true {
        return Err("captures require fictional records".into());
    }
    let mut manifest = Vec::new();
    if std::env::args().any(|arg| arg == "--motion-only") {
        return capture_motion(&data, &output);
    }
    if std::env::args().any(|arg| arg == "--requests-only") {
        return capture_requests(&data, &output);
    }
    let roadmap_only = std::env::args()
        .nth(2)
        .is_some_and(|arg| arg == "--roadmap-only");
    for size in [Size::new(144, 50), Size::new(128, 40)] {
        if roadmap_only {
            continue;
        }
        let mut model = model(size, &data, ThemeMode::Dark)?;
        capture(
            &model,
            &output,
            &format!("collapsed-{}", size.cols * 10),
            &mut manifest,
        )?;
        click(&mut model, Widget::ReasoningToggle.id())?;
        for number in 1..=12 {
            let index = model
                .trace()
                .steps()
                .iter()
                .position(|s| s.number == number)
                .ok_or("missing decision")?;
            let top = model.top_row();
            click(&mut model, Widget::trace_row(index))?;
            if model.selected() != Some(&Selected::Step(number)) || model.top_row() != top {
                return Err(
                    format!("decision {number} did not preserve conversation position").into(),
                );
            }
            capture(
                &model,
                &output,
                &format!("flight-{}-step-{number}", size.cols * 10),
                &mut manifest,
            )?;
        }
    }
    for (name, size, theme) in [
        ("light", Size::new(144, 50), ThemeMode::Light),
        ("small", Size::new(96, 34), ThemeMode::Dark),
        ("narrow", Size::new(64, 36), ThemeMode::Dark),
        ("active", Size::new(144, 50), ThemeMode::Dark),
        ("failed", Size::new(144, 50), ThemeMode::Dark),
        ("permission", Size::new(144, 50), ThemeMode::Dark),
        ("stopped", Size::new(144, 50), ThemeMode::Dark),
        ("large-font", Size::new(110, 34), ThemeMode::Dark),
        ("large-trace", Size::new(144, 50), ThemeMode::Dark),
        ("long-content", Size::new(128, 40), ThemeMode::Dark),
    ] {
        if roadmap_only {
            continue;
        }
        let mut model = model(size, &data, theme)?;
        match name {
            "failed" => {
                update(
                    &mut model,
                    Msg::Agent(AgentEvent::Error(
                        "The connection closed during the restart check. Your message is saved."
                            .into(),
                    )),
                );
            }
            "permission" => {
                update(
                    &mut model,
                    Msg::Agent(AgentEvent::PermissionRequest {
                        id: 1,
                        title: "Run the restart recovery checks".into(),
                        choices: vec![
                            PermissionChoice::new("allow", PermissionKind::AllowOnce),
                            PermissionChoice::new("deny", PermissionKind::RejectOnce),
                        ],
                        edit: false,
                        risk: RiskTier::Medium,
                        plan: None,
                    }),
                );
            }
            "stopped" => {
                update(
                    &mut model,
                    Msg::Agent(AgentEvent::TurnEnded(TurnEnd::Cancelled)),
                );
            }
            "large-font" => {
                update(
                    &mut model,
                    Msg::CellSize(Some(magistr_cockpit::pixel::CellSize {
                        width: 12,
                        height: 28,
                    })),
                );
            }
            "large-trace" => {
                for i in 0..100 {
                    update(
                        &mut model,
                        Msg::Engine(fixtures::event(
                            "think",
                            "step_appended",
                            json!({"step":{
                                "step_number":100+i*13,"purpose":format!("Check restart case {}",i+1),
                                "thought":"Check the recorded receipt before retrying.",
                                "outcome":"The receipt preserves the next attempt.",
                                "branch_id":format!("restart-{}",i%8),"branch_name":format!("Restart question {}",i%8+1),
                                "branch_from":if i>0 {100+(i-1)*13} else {12}
                            }}),
                        )),
                    );
                }
            }
            "long-content" => {
                update(&mut model,Msg::Agent(AgentEvent::Text("A restarted worker must keep the recorded receipt, delivery deadline and tenant allowance together. ".repeat(30))));
                update(&mut model,Msg::Paste("\nAlso cover the worker stopping between each write.\nKeep the expected result in the test name.".into()));
            }
            _ => {}
        }
        capture(&model, &output, name, &mut manifest)?;
        if matches!(
            name,
            "light" | "small" | "narrow" | "large-font" | "large-trace" | "long-content"
        ) {
            click(&mut model, Widget::ReasoningToggle.id())?;
            capture(&model, &output, &format!("{name}-expanded"), &mut manifest)?;
        }
    }
    for (name, size, large_text) in [
        ("commands", Size::new(144, 50), false),
        ("settings", Size::new(144, 50), false),
        ("settings-small", Size::new(64, 24), false),
        ("settings-large-font", Size::new(110, 34), true),
        ("search", Size::new(144, 50), false),
        ("help", Size::new(144, 50), false),
        ("add-files", Size::new(144, 50), false),
        ("linked-work", Size::new(144, 50), false),
        ("plan-permission", Size::new(144, 50), false),
        ("plan-permission-small", Size::new(64, 24), false),
        ("plan-permission-large-font", Size::new(110, 34), true),
    ] {
        if roadmap_only {
            continue;
        }
        let mut model = model(size, &data, ThemeMode::Dark)?;
        if large_text {
            update(
                &mut model,
                Msg::CellSize(Some(magistr_cockpit::pixel::CellSize {
                    width: 12,
                    height: 28,
                })),
            );
        }
        match name {
            "commands" => click(&mut model, Widget::Commands.id())?,
            "settings" | "settings-small" | "settings-large-font" => {
                command(&mut model, "Settings")?
            }
            "search" => {
                command(&mut model, "Search recorded work")?;
                for c in "retry".chars() {
                    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
                }
            }
            "help" => command(&mut model, "Keyboard help")?,
            "add-files" => {
                click(&mut model, Widget::PromptFiles.id())?;
                if model.completion().is_none() {
                    return Err("file chooser did not open".into());
                }
            }
            "linked-work" => {
                click(&mut model, Widget::ReasoningToggle.id())?;
                click(&mut model, Widget::LatestDecision.id())?;
                click(&mut model, Widget::LinkedWork.id())?;
                if !matches!(model.selected(), Some(Selected::Task(_))) {
                    return Err("linked work did not open its recorded task".into());
                }
            }
            _ => {
                update(
                    &mut model,
                    Msg::Agent(AgentEvent::PermissionRequest {
                        id: 2,
                        title: "Check recovery after a worker restart".into(),
                        choices: vec![
                            PermissionChoice::new("allow", PermissionKind::AllowOnce),
                            PermissionChoice::new("deny", PermissionKind::RejectOnce),
                        ],
                        edit: false,
                        risk: RiskTier::Medium,
                        plan: Some(AskPlan::new([
                            "Save the delivery receipt before scheduling the next attempt.",
                            "Restart the worker and confirm that it reads the saved schedule.",
                            "Run the retry checks for two customers with separate limits.",
                        ])),
                    }),
                );
            }
        }
        capture(&model, &output, name, &mut manifest)?;
    }
    for (theme_name, theme) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        for (size_name, size, large_text) in [
            ("full", Size::new(144, 50), false),
            ("small", Size::new(64, 36), false),
            ("large-font", Size::new(110, 34), true),
        ] {
            let mut model = model_with_roadmap(size, &data, theme, true)?;
            if large_text {
                update(
                    &mut model,
                    Msg::CellSize(Some(magistr_cockpit::pixel::CellSize {
                        width: 12,
                        height: 28,
                    })),
                );
            }
            let conversation = model.top_row();
            let draft = model.prompt().text().to_owned();
            click(&mut model, Widget::TabRoadmap.id())?;
            capture(
                &model,
                &output,
                &format!("roadmap-{size_name}-{theme_name}"),
                &mut manifest,
            )?;
            click(&mut model, Widget::roadmap_row(0))?;
            if model.tab() != StageTab::Roadmap
                || !matches!(model.selected(), Some(Selected::Chunk(_)))
                || model.top_row() != conversation
                || model.prompt().text() != draft
            {
                return Err("opening roadmap work changed the conversation or message".into());
            }
            capture(
                &model,
                &output,
                &format!("roadmap-{size_name}-{theme_name}-details"),
                &mut manifest,
            )?;
        }
    }
    let empty = Model::new(Size::new(128, 40))
        .with_surface(Surface::Terminal)
        .with_title("Harbor")
        .with_theme(Theme::forced(ThemeMode::Dark))
        .with_tab(StageTab::Turn);
    if !roadmap_only {
        capture(&empty, &output, "empty", &mut manifest)?;
    }
    std::fs::write(
        output.join("native-captures.json"),
        serde_json::to_vec_pretty(&json!({
            "renderer":"Production cockpit view → runtime Surfaces → terminal graphics → decoded PNG",
            "synthetic":true,"production_shell":true,"captures":manifest
        }))?,
    )?;
    Ok(())
}

fn capture_motion(data: &Value, output: &Path) -> Result<(), Box<dyn Error>> {
    use magistr_cockpit::theme::MotionMode;
    let mut manifest = Vec::new();
    for (name, theme, cols) in [
        ("dark", ThemeMode::Dark, 144),
        ("light", ThemeMode::Light, 144),
        ("compact", ThemeMode::Dark, 64),
    ] {
        let mut current = model(Size::new(cols, 40), data, theme)?;
        update(
            &mut current,
            Msg::Agent(AgentEvent::UserText("Check the restart behavior".into())),
        );
        for step in 0..32 {
            if step % 8 == 0 {
                capture(
                    &current,
                    output,
                    &format!("motion-{name}-{step}"),
                    &mut manifest,
                )?;
            }
            update(&mut current, Msg::Tick);
        }
        current = current.with_theme(Theme::forced(theme).with_motion(MotionMode::Reduced));
        capture(
            &current,
            output,
            &format!("motion-{name}-reduced"),
            &mut manifest,
        )?;
        update(
            &mut current,
            Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
        );
        capture(
            &current,
            output,
            &format!("motion-{name}-complete"),
            &mut manifest,
        )?;
    }
    std::fs::write(
        output.join("motion-captures.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

fn capture_requests(data: &Value, output: &Path) -> Result<(), Box<dyn Error>> {
    let mut manifest = Vec::new();
    for (theme_name, theme) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        for state in [
            "waiting",
            "queued",
            "working",
            "complete",
            "failed",
            "permission",
        ] {
            let mut current = model_with_roadmap(Size::new(144, 50), data, theme, true)?;
            requests::seed(&mut current, state)?;
            capture(
                &current,
                output,
                &format!("request-{state}-{theme_name}"),
                &mut manifest,
            )?;
        }
    }
    for (name, size, large) in [
        ("small", Size::new(64, 24), false),
        ("large-font", Size::new(110, 34), true),
    ] {
        for state in ["waiting", "permission"] {
            let mut current = model_with_roadmap(size, data, ThemeMode::Dark, true)?;
            if large {
                update(
                    &mut current,
                    Msg::CellSize(Some(magistr_cockpit::pixel::CellSize {
                        width: 12,
                        height: 28,
                    })),
                );
            }
            requests::seed(&mut current, state)?;
            capture(
                &current,
                output,
                &format!("request-{state}-{name}"),
                &mut manifest,
            )?;
        }
    }
    let mut current = model(Size::new(128, 40), data, ThemeMode::Dark)?;
    requests::seed(&mut current, "waiting")?;
    requests::click(&mut current, Widget::TabTurn.id())?;
    capture(&current, output, "conversation-waiting", &mut manifest)?;
    std::fs::write(
        output.join("native-captures.json"),
        serde_json::to_vec_pretty(&json!({
            "synthetic": true, "production_shell": true, "captures": manifest
        }))?,
    )?;
    Ok(())
}

fn model(size: Size, data: &Value, theme: ThemeMode) -> Result<Model, Box<dyn Error>> {
    model_with_roadmap(size, data, theme, false)
}

fn model_with_roadmap(
    size: Size,
    data: &Value,
    theme: ThemeMode,
    complete: bool,
) -> Result<Model, Box<dyn Error>> {
    let records = |key: &str| {
        data[key]
            .as_array()
            .cloned()
            .ok_or("missing fixture records")
    };
    let mut chunks = records("chunks")?;
    let ids = [
        "attempts",
        "leases",
        "retry-budget",
        "retry-after",
        "backoff",
    ];
    if !complete {
        chunks.retain(|c| ids.iter().any(|id| c["id"] == *id));
        for chunk in &mut chunks {
            if let Some(deps) = chunk["deps"].as_array_mut() {
                deps.retain(|d| ids.iter().any(|id| d == *id));
            }
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
        .with_title("Harbor / Reliable retries")
        .with_session_started("14:00")
        .with_snapshot(&snapshot)
        .with_theme(Theme::forced(theme))
        .with_tab(StageTab::Turn)
        .with_workspace_files(vec![
            "src/retry/lease.rs".into(),
            "src/retry/budget.rs".into(),
        ]);
    update(
        &mut model,
        Msg::Mode(Decision::Graphics(Protocol::Kitty(Passthrough::Direct)).mode()),
    );
    for branch in records("branches")? {
        update(
            &mut model,
            Msg::Engine(fixtures::event("think", "branch_status_changed", branch)),
        );
    }
    harbor::conversation(&mut model, &data["turn"])?;
    update(
        &mut model,
        Msg::Paste("Also test a restart immediately after the receipt commits.".into()),
    );
    for _ in 0..8 {
        update(&mut model, Msg::Tick);
    }
    Ok(model)
}

fn command(model: &mut Model, query: &str) -> Result<(), Box<dyn Error>> {
    click(model, Widget::Commands.id())?;
    for c in query.chars() {
        update(model, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    click(model, Widget::palette_row(0))?;
    let opened = match query {
        "Settings" => model.settings().is_some(),
        "Search recorded work" => model.search().is_some(),
        "Keyboard help" => model.is_sheet_open(),
        _ => false,
    };
    if !opened {
        return Err(format!("command did not open {query}").into());
    }
    Ok(())
}

fn clock() -> FrameClock {
    FrameClock::new(8, Duration::from_millis(1600))
}

fn click(model: &mut Model, id: WidgetId) -> Result<(), Box<dyn Error>> {
    let resolved = Preview::layout(model, &clock(), view)?;
    let placed = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(id))
        .ok_or("missing decision label")?;
    let x = placed.px.x + placed.px.width / 2.0;
    let y = placed.px.y + placed.px.height / 2.0;
    let mut hits = HitRegistry::new();
    hits.register_layout_px(&resolved, 0);
    let target = hits.hit_at(f64::from(x), f64::from(y));
    if target != Some(id) {
        return Err(format!("visible label resolves to {target:?}, expected {id:?}").into());
    }
    let cell = model.cell_size();
    for kind in [MouseKind::Down(Button::Left), MouseKind::Up(Button::Left)] {
        update(
            model,
            Msg::Mouse {
                event: MouseEvent::new(
                    kind,
                    (x / f32::from(cell.width)) as u16,
                    (y / f32::from(cell.height)) as u16,
                )
                .at_px(x as u32, y as u32),
                target,
            },
        );
    }
    Ok(())
}

fn capture(
    model: &Model,
    output: &Path,
    name: &str,
    manifest: &mut Vec<Value>,
) -> Result<(), Box<dyn Error>> {
    let pixels = Preview::frame(model, &clock(), view)?;
    pixels.save_png(output.join(format!("{name}.png")))?;
    let resolved = Preview::layout(model, &clock(), view)?;
    let areas:Vec<_> = resolved.placed().iter().filter_map(|placed| {
        placed.id.map(|id|json!({"id":format!("{id:?}"),"x":placed.px.x,"y":placed.px.y,"width":placed.px.width,"height":placed.px.height}))
    }).collect();
    manifest.push(json!({"file":format!("{name}.png"),"width":pixels.width(),"height":pixels.height(),"selected":format!("{:?}",model.selected()),"areas":areas}));
    Ok(())
}
