//! Send a request, cancel it, send again, resize the panels, then return to Conversation.
//! Close the window after typing "Keep this draft" to verify the native journey.
use magistr_application::sessions::{AgentEvent, TurnEnd};
use magistr_cockpit::cockpit::{
    Cmd, Msg, RequestPhase, Size, StageTab, flight_divider::FlightDivider as D,
};
use magistr_gui2::{Native, fixture::Fixture};

fn main() {
    let model = Fixture::roadmap(Size::new(120, 40));
    let mut native = Native::new("Magistr roadmap");
    let mut commands = native.commands().expect("command owner");
    let inbox = native.inbox();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("fixture runtime");
        runtime.block_on(async move {
            let mut requests = 0;
            while let Some(command) = commands.next().await {
                match command {
                    Cmd::RoadmapPrompt(_) => {
                        requests += 1;
                        if requests == 1 {
                            inbox.send(Msg::RoadmapRequestQueued);
                            inbox.send(Msg::Agent(AgentEvent::Text(
                                "Implementation continues while planning is queued.\n\n".into(),
                            )));
                        } else {
                            inbox.send(Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
                            inbox.send(Msg::RoadmapRequestStarted);
                            inbox.send(Msg::Agent(AgentEvent::Text(
                                "Recovery checks have been added to the plan.\n\n".repeat(24),
                            )));
                            inbox.send(Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
                        }
                    }
                    Cmd::CancelRoadmapPrompt => {
                        inbox.send(Msg::RoadmapRequestCancelled);
                    }
                    _ => {}
                }
            }
        });
    });
    let model = native.run(model).expect("native window");
    assert_eq!(model.roadmap_requests().len(), 2);
    assert_eq!(model.roadmap_requests()[0].phase, RequestPhase::Stopped);
    assert_eq!(model.roadmap_requests()[1].phase, RequestPhase::Complete);
    assert!(
        model.roadmap_requests()[0]
            .response(model.turn())
            .is_empty()
    );
    assert_ne!(model.panel_fraction(D::Columns, 0.57), 0.57);
    assert_eq!(model.tab(), StageTab::Turn);
    assert_eq!(model.prompt().text(), "Keep this draft");
    assert!(model.active_panel().is_none());
    println!("Native request routing, cancellation, resize, return and retained draft verified.");
}
