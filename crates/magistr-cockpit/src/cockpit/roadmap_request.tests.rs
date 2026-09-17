use super::*;
use crate::agent::{AgentEvent, ToolRecord, ToolStatus, ToolVerb, TurnEnd};
use crate::cockpit::{
    Animation, Cmd, LoopEvent, Model, Msg, Size, StageTab, TurnState, Widget, update,
};
use crate::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use crate::theme::{MotionMode, Theme};

fn model() -> Model {
    let mut model = Model::new(Size::new(144, 50));
    model.pixels = true;
    model.tab = StageTab::Roadmap;
    model
}

fn submit(model: &mut Model, text: &str) -> Vec<Cmd> {
    update(model, Msg::Paste(text.to_owned()));
    update(model, Msg::Key(KeyEvent::new(KeyCode::Enter)))
        .into_iter()
        .filter(|cmd| !matches!(cmd, Cmd::Animate(_) | Cmd::Still))
        .collect()
}

fn click(model: &mut Model, widget: Widget) -> Vec<Cmd> {
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(widget.id()),
        },
    )
    .into_iter()
    .filter(|cmd| !matches!(cmd, Cmd::Animate(_) | Cmd::Still))
    .collect()
}

#[test]
fn request_owns_only_its_admitted_response_across_iteration_boundaries() {
    let mut model = model();
    update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 1,
            max: 3,
        }),
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("Existing implementation".into())),
    );
    let old_lines = model.turn.lines().len();
    assert_eq!(
        submit(&mut model, "Add recovery checks"),
        vec![Cmd::RoadmapPrompt("Add recovery checks".into())]
    );
    assert!(model.roadmap_requests_open());
    assert_eq!(model.roadmap_requests()[0].phase, RequestPhase::Waiting);
    assert_eq!(model.turn.lines().len(), old_lines);
    update(&mut model, Msg::RoadmapRequestQueued);
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("\nImplementation is done.".into())),
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
    );
    assert_eq!(model.roadmap_requests()[0].phase, RequestPhase::Queued);
    assert_eq!(model.roadmap_requests()[0].source_start, None);
    update(&mut model, Msg::RoadmapRequestStarted);
    let start = model.roadmap_requests()[0]
        .source_start
        .expect("admitted range");
    assert_eq!(model.agent_activity().task, "Updating roadmap");
    assert_eq!(model.agent_activity().status, "Waiting for response");
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("Added two recovery checks.".into())),
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
    );
    let response = model.roadmap_requests()[0].clone();
    assert_eq!(response.phase, RequestPhase::Complete);
    assert_eq!(response.source_end, Some(start + 1));
    update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 2,
            max: 3,
        }),
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("Next implementation".into())),
    );
    assert_eq!(model.roadmap_requests()[0], response);
    assert_eq!(
        model.turn.lines()[start].text(),
        "Added two recovery checks."
    );
}

#[test]
fn queued_rejection_keeps_the_current_work_and_newer_draft() {
    let mut model = model();
    update(
        &mut model,
        Msg::Agent(AgentEvent::Thought("Checking current work".into())),
    );
    submit(&mut model, "Read @missing.txt");
    update(&mut model, Msg::RoadmapRequestQueued);
    model.prompt.insert("A newer request");
    update(
        &mut model,
        Msg::Agent(AgentEvent::PromptRejected {
            text: "Read @missing.txt".into(),
            error: "The attachment was not found.".into(),
        }),
    );
    assert_eq!(model.turn_state(), TurnState::Streaming);
    assert_eq!(model.agent_activity().status, "Thinking");
    assert_eq!(model.prompt.text(), "A newer request");
    assert_eq!(model.roadmap_requests()[0].phase, RequestPhase::Failed);
    assert_eq!(
        model.roadmap_requests()[0].error.as_deref(),
        Some("The attachment was not found.")
    );
    assert!(!model.roadmap_request_pending());
}

#[test]
fn unrelated_rejected_prompt_does_not_end_a_queued_or_running_roadmap_request() {
    for started in [false, true] {
        let mut model = model();
        submit(&mut model, "Add checks");
        update(
            &mut model,
            if started {
                Msg::RoadmapRequestStarted
            } else {
                Msg::RoadmapRequestQueued
            },
        );
        let request = model.roadmap_requests()[0].clone();
        let activity = model.agent_activity();
        update(
            &mut model,
            Msg::Agent(AgentEvent::PromptRejected {
                text: "Another question".to_owned(),
                error: "A response is already in progress.".to_owned(),
            }),
        );
        assert_eq!(model.roadmap_requests()[0], request);
        assert_eq!(model.agent_activity(), activity);
        assert_eq!(model.prompt.text(), "Another question");
    }
}

#[test]
fn queued_cancel_leaves_implementation_running_and_restores_request_draft() {
    let mut model = model();
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("Current item".into())),
    );
    submit(&mut model, "Split the next item");
    update(&mut model, Msg::RoadmapRequestQueued);
    assert_eq!(
        click(&mut model, Widget::PromptAction),
        vec![Cmd::CancelRoadmapPrompt]
    );
    update(&mut model, Msg::RoadmapRequestCancelled);
    assert_eq!(model.turn_state(), TurnState::Streaming);
    assert_eq!(model.prompt.text(), "Split the next item");
    assert_eq!(model.roadmap_requests()[0].phase, RequestPhase::Stopped);
}

#[test]
fn request_panel_and_draft_survive_navigation() {
    let mut model = model();
    submit(&mut model, "Add more ready work");
    update(&mut model, Msg::RoadmapRequestStarted);
    model.prompt.insert("Keep this draft");
    click(&mut model, Widget::RoadmapDetails);
    assert!(!model.roadmap_requests_open());
    click(&mut model, Widget::RoadmapRequests);
    model.show(StageTab::Turn);
    model.show(StageTab::Roadmap);
    assert!(model.roadmap_requests_open());
    assert_eq!(model.prompt.text(), "Keep this draft");
    assert_eq!(model.roadmap_requests()[0].text, "Add more ready work");
}

#[test]
fn activity_is_observed_and_tool_completion_does_not_complete_request() {
    let mut model = model();
    submit(&mut model, "Add ready work");
    assert_eq!(model.agent_activity().status, "Waiting for response");
    update(&mut model, Msg::RoadmapRequestStarted);
    update(
        &mut model,
        Msg::Agent(AgentEvent::Thought("Looking at the plan".into())),
    );
    assert_eq!(model.agent_activity().status, "Thinking");
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCall {
            id: "read".into(),
            title: "internal_tool_name".into(),
            verb: ToolVerb::Read,
            status: ToolStatus::InProgress,
            diff: None,
            images: vec![],
            record: Box::new(ToolRecord::default()),
        }),
    );
    assert_eq!(model.agent_activity().status, "Reading");
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCallUpdate {
            id: "read".into(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: None,
            images: vec![],
            record: Box::new(ToolRecord::default()),
        }),
    );
    assert_eq!(model.agent_activity().status, "Waiting for response");
    assert_eq!(model.roadmap_requests()[0].phase, RequestPhase::Working);
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("The plan includes checks.".into())),
    );
    assert_eq!(model.agent_activity().status, "Writing a response");
    update(
        &mut model,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
    );
    assert_eq!(model.agent_activity().status, "Response complete");
    assert!(!model.agent_activity().active);
}

#[test]
fn request_scrolling_leaves_following_and_preserves_held_position_on_append() {
    let mut model = model();
    submit(&mut model, "Describe the plan");
    update(&mut model, Msg::RoadmapRequestStarted);
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("A long response line.\n".repeat(100))),
    );
    assert_eq!(model.roadmap_request_scroll(), usize::MAX);
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::ScrollUp, 0, 0),
            target: Some(Widget::RequestBody.id()),
        },
    );
    let held = model.roadmap_request_scroll();
    assert!(held > 0 && held < usize::MAX);
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("More response text.\n".repeat(20))),
    );
    assert_eq!(model.roadmap_request_scroll(), held);
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::End)));
    assert_eq!(model.roadmap_request_scroll(), usize::MAX);
}

#[test]
fn roadmap_request_semantics_and_panel_keys_are_independent_of_graphics() {
    let mut model = model();
    model.pixels = false;
    assert_eq!(
        submit(&mut model, "Add checks"),
        vec![Cmd::RoadmapPrompt("Add checks".to_owned())]
    );
    assert!(
        model
            .turn
            .lines()
            .iter()
            .any(|line| line.text().contains("Roadmap request: Add checks"))
    );
    let toggle = Msg::Key(KeyEvent::new(KeyCode::Char('m')).with(crate::input::Modifiers::ALT));
    let commands = update(&mut model, toggle.clone());
    assert!(!model.roadmap_requests_open());
    assert!(commands.contains(&Cmd::Announce("Roadmap item details".to_owned())));
    update(&mut model, toggle);
    assert!(model.roadmap_requests_open());
    model.roadmap_request_scroll = 2;
    click(&mut model, Widget::RequestLatest);
    assert_eq!(model.roadmap_request_scroll(), usize::MAX);
}

#[test]
fn incomplete_responses_keep_their_reason_and_recover_the_request_without_overwriting_drafts() {
    for (end, phase, reason) in [
        (
            TurnEnd::Refusal,
            RequestPhase::Failed,
            Some("The agent declined this request."),
        ),
        (
            TurnEnd::MaxTokens,
            RequestPhase::Failed,
            Some("The response reached its length limit before it finished."),
        ),
        (
            TurnEnd::MaxTurnRequests,
            RequestPhase::Failed,
            Some("The agent reached its action limit before it finished."),
        ),
        (
            TurnEnd::Other,
            RequestPhase::Stopped,
            Some("The response stopped before it finished."),
        ),
        (TurnEnd::Cancelled, RequestPhase::Stopped, None),
    ] {
        for newer_draft in [false, true] {
            let mut model = model();
            submit(&mut model, "Add recovery checks");
            update(&mut model, Msg::RoadmapRequestStarted);
            update(
                &mut model,
                Msg::Agent(AgentEvent::Text("Reviewing current coverage".into())),
            );
            if newer_draft {
                model.prompt.insert("A newer draft");
            }
            update(&mut model, Msg::Agent(AgentEvent::TurnEnded(end)));
            let request = model.roadmap_requests()[0].clone();
            assert_eq!(request.phase, phase);
            assert_eq!(request.error.as_deref(), reason);
            assert_eq!(
                model.prompt.text(),
                if newer_draft {
                    "A newer draft"
                } else {
                    "Add recovery checks"
                }
            );
            update(
                &mut model,
                Msg::Loop(LoopEvent::Started {
                    iteration: 2,
                    max: 3,
                }),
            );
            assert_eq!(
                model.roadmap_requests()[0],
                request,
                "the saved explanation survives later work"
            );
        }
    }
}

#[test]
fn pending_actions_wait_and_running_actions_take_precedence() {
    let mut model = model();
    submit(&mut model, "Review the plan");
    update(&mut model, Msg::RoadmapRequestStarted);
    for id in ["a", "b"] {
        update(
            &mut model,
            Msg::Agent(AgentEvent::ToolCall {
                id: id.into(),
                title: "Read project notes".into(),
                verb: ToolVerb::Read,
                status: ToolStatus::Pending,
                diff: None,
                images: vec![],
                record: Box::new(ToolRecord::default()),
            }),
        );
    }
    assert_eq!(model.agent_activity().status, "Waiting for an action");
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCallUpdate {
            id: "b".into(),
            title: None,
            status: Some(ToolStatus::InProgress),
            diff: None,
            duration_ms: None,
            images: vec![],
            record: Box::new(ToolRecord::default()),
        }),
    );
    assert_eq!(
        model.agent_activity().status,
        "Reading",
        "a pending action does not obscure an action that started"
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCallUpdate {
            id: "b".into(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: None,
            images: vec![],
            record: Box::new(ToolRecord::default()),
        }),
    );
    assert_eq!(model.agent_activity().status, "Waiting for an action");
    assert_eq!(model.roadmap_requests()[0].phase, RequestPhase::Working);
}

#[test]
fn request_motion_uses_model_ticks_and_respects_reduced_motion() {
    let mut model = model();
    submit(&mut model, "Add checks");
    assert!(model.animates(Animation::Motion));
    let tick = model.ticks();
    update(&mut model, Msg::Tick);
    assert!(model.ticks() > tick);
    model.set_theme(Theme::default().with_motion(MotionMode::Reduced));
    assert!(!model.animates(Animation::Motion));
}
