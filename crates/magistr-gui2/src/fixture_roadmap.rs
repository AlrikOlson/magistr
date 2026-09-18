//! Deterministic roadmap records and request outcomes for native interface checks.
use crate::fixture::Fixture;
use magistr_application::sessions::{AgentEvent, TurnEnd};
use magistr_cockpit::cockpit::{Model, Msg, RequestPhase, Size, StageTab, Surface, Widget, update};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::harness::{Scenes, fixtures};
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use magistr_workflow_contracts::NativeSnapshot;
use serde_json::json;

impl Fixture {
    pub fn roadmap(size: Size) -> Model {
        let mut model = Model::new(size)
            .with_surface(Surface::Native)
            .with_tab(StageTab::Roadmap);
        let chunks = fixtures::chunks_from(vec![
            json!({"id":"delivery-base","name":"Saved deliveries","title":"Keep delivery attempts after restart","status":"done","priority":50,
                "description":"Delivery attempts are saved before a worker stops."}),
            json!({"id":"delivery-running","name":"Retry delivery","title":"Retry deliveries without repeating completed work","status":"in_progress","priority":100,
                "description":"Keep a separate attempt count for each customer.","deps":["delivery-base"],
                "acceptance":["Resume interrupted deliveries after restart.","Keep completed deliveries from being sent twice."],"cross_refs":["think:701"]}),
            json!({"id":"delivery-ready","name":"Recovery checks","title":"Check recovery after restart","status":"pending","priority":120,
                "description":"Verify that saved deliveries resume safely and report what happened.","deps":["delivery-base"],
                "acceptance":["An interrupted delivery resumes once.","Each customer keeps their own remaining attempts."]}),
            json!({"id":"delivery-blocked","name":"Delivery credentials","title":"Connect the delivery service","status":"pending","priority":140,
                "blocked_by":{"kind":"awaiting_human","reason":"The delivery service needs an access key."},
                "description":"Connect the service after an access key is supplied."}),
            json!({"id":"delivery-later","name":"Delivery summary","title":"Show a summary of delivered messages","status":"pending","priority":180,
                "deps":["delivery-running"],"description":"Show which deliveries finished and which need attention.",
                "reprioritize":{"suggested_priority":160,"reason":"A clear summary helps users check the recovered deliveries."}}),
        ]);
        update(
            &mut model,
            Msg::Engine(EngineEvent::Seed(NativeSnapshot {
                progress: fixtures::progress(1, 5),
                chunks,
                ..NativeSnapshot::default()
            })),
        );
        update(
            &mut model,
            Msg::Engine(EngineEvent::local(fixtures::frame(
                "think",
                "step_appended",
                json!({"step":{"step_number":701,"purpose":"Keep recovery separate for each customer",
                "thought":"A busy customer should not delay another customer's delivery.",
                "outcome":"Save each customer's remaining attempts.","execution_ref":"chunk:delivery-running"}}),
            ))),
        );
        press(&mut model, Widget::roadmap_row(0));
        Scenes::settle(&mut model);
        model
    }

    pub fn roadmap_request(size: Size, phase: RequestPhase) -> Model {
        let mut model = Self::roadmap(size);
        update(
            &mut model,
            Msg::Paste("Add checks for interrupted deliveries.".into()),
        );
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
        match phase {
            RequestPhase::Waiting => {}
            RequestPhase::Queued => {
                update(&mut model, Msg::RoadmapRequestQueued);
            }
            _ => {
                update(&mut model, Msg::RoadmapRequestStarted);
                update(&mut model,Msg::Agent(AgentEvent::Text(
                    "The plan now includes recovery checks.\n\nRestart during a delivery, confirm that it resumes, and verify that completed deliveries are kept.\n\nThese changes leave the current work running.".into())));
                match phase {
                    RequestPhase::Complete => {
                        update(
                            &mut model,
                            Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
                        );
                    }
                    RequestPhase::Stopped => {
                        update(&mut model, Msg::RoadmapRequestCancelled);
                    }
                    RequestPhase::Failed => {
                        update(
                            &mut model,
                            Msg::Agent(AgentEvent::TurnEnded(TurnEnd::MaxTurnRequests)),
                        );
                    }
                    _ => {}
                }
            }
        }
        Scenes::settle(&mut model);
        model
    }
}
fn press(model: &mut Model, target: magistr_cockpit::widget_id::WidgetId) {
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(target),
        },
    );
}
