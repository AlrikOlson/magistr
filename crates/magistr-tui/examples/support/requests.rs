//! Fictional roadmap request states exercised through the production input and event path.
use magistr_application::sessions::{
    AgentEvent, PermissionChoice, PermissionKind, ToolRecord, ToolStatus, ToolVerb, TurnEnd,
};
use magistr_cockpit::cockpit::{Cmd, FrameClock, Model, Msg, Widget, update, view};
use magistr_cockpit::hit::HitRegistry;
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseKind};
use magistr_cockpit::widget_id::WidgetId;
use magistr_tui::screenshot::Preview;
use std::{error::Error, time::Duration};

pub const REQUEST: &str = "Add more ready work for retry recovery. Separate restart checks from customer limits, and keep the dependencies clear.";

pub fn seed(model: &mut Model, state: &str) -> Result<(), Box<dyn Error>> {
    update(model, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    update(
        model,
        Msg::Loop(magistr_cockpit::cockpit::LoopEvent::Ended(
            "The previous work item finished.".into(),
        )),
    );
    if model.tab() != magistr_cockpit::cockpit::StageTab::Roadmap {
        click(model, Widget::TabRoadmap.id())?;
    }
    click(model, Widget::RoadmapRequests.id())?;
    click(model, Widget::Prompt.id())?;
    update(
        model,
        Msg::Key(KeyEvent::new(KeyCode::Char('a')).with(Modifiers::CTRL)),
    );
    // Home followed by repeated Delete is portable across editor bindings.
    update(model, Msg::Key(KeyEvent::new(KeyCode::Home)));
    for _ in 0..model.prompt().text().chars().count() {
        update(model, Msg::Key(KeyEvent::new(KeyCode::Delete)));
    }
    update(model, Msg::Paste(REQUEST.into()));
    let commands = click(model, Widget::PromptAction.id())?;
    if !commands
        .iter()
        .any(|c| matches!(c, Cmd::RoadmapPrompt(text) if text == REQUEST))
    {
        return Err(format!("roadmap request was not delivered: {commands:?}").into());
    }
    if state == "waiting" {
        return Ok(());
    }
    if state == "queued" {
        update(
            model,
            Msg::Loop(magistr_cockpit::cockpit::LoopEvent::Started {
                iteration: 2,
                max: 10,
            }),
        );
        update(
            model,
            Msg::Loop(magistr_cockpit::cockpit::LoopEvent::Stage(
                magistr_cockpit::cockpit::LoopStage::Gates,
            )),
        );
        update(model, Msg::RoadmapRequestQueued);
        return Ok(());
    }
    update(model, Msg::RoadmapRequestStarted);
    if state == "failed" {
        update(
            model,
            Msg::Agent(AgentEvent::PromptRejected {
                text: REQUEST.into(),
                error: "The connection closed before the roadmap could be updated.".into(),
            }),
        );
        return Ok(());
    }
    if state == "permission" {
        update(
            model,
            Msg::Agent(AgentEvent::PermissionRequest {
                id: 73,
                title: "Add restart recovery checks to the roadmap".into(),
                choices: vec![
                    PermissionChoice::new("allow", PermissionKind::AllowOnce),
                    PermissionChoice::new("deny", PermissionKind::RejectOnce),
                ],
                edit: false,
                risk: magistr_engine::policy::RiskTier::Low,
                plan: None,
            }),
        );
        return Ok(());
    }
    update(model, Msg::Agent(AgentEvent::Text("I’m checking the existing work and its dependencies before adding the recovery checks.\n".into())));
    update(
        model,
        Msg::Agent(AgentEvent::ToolCall {
            id: "harbor-roadmap-check".into(),
            title: "Review retry recovery work".into(),
            verb: ToolVerb::Read,
            status: if state == "working" {
                ToolStatus::InProgress
            } else {
                ToolStatus::Completed
            },
            diff: None,
            images: Vec::new(),
            record: Box::new(ToolRecord::default()),
        }),
    );
    if state == "complete" {
        update(model, Msg::Agent(AgentEvent::Text("\nThe roadmap now separates restart recovery from customer limits.\n\n• Restart after a saved receipt is ready to begin.\n• Customer limit checks follow the separate-limit work.\n• Recovery checks cover both interrupted writes.\n\nNo work item has been started.".into())));
        update(model, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    }
    Ok(())
}

pub fn click(model: &mut Model, id: WidgetId) -> Result<Vec<Cmd>, Box<dyn Error>> {
    let resolved = Preview::layout(
        model,
        &FrameClock::new(8, Duration::from_millis(1600)),
        view,
    )?;
    let rect = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(id))
        .ok_or_else(|| format!("missing request control {id:?}"))?
        .px;
    let x = rect.x + rect.width / 2.0;
    let y = rect.y + rect.height / 2.0;
    let mut hits = HitRegistry::new();
    hits.register_layout_px(&resolved, 0);
    let target = hits.hit_at(f64::from(x), f64::from(y));
    if target != Some(id) {
        return Err(format!("request control is covered: {id:?} -> {target:?}").into());
    }
    let mut commands = Vec::new();
    for kind in [MouseKind::Down(Button::Left), MouseKind::Up(Button::Left)] {
        commands.extend(update(
            model,
            Msg::Mouse {
                event: MouseEvent::new(kind, 0, 0).at_px(x as u32, y as u32),
                target,
            },
        ));
    }
    Ok(commands)
}
