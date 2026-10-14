use super::*;
use crate::native::{OwnerReply, ThinkCommand};
use magistr_workflow::ship::domain::task::TaskType;
use magistr_workflow_contracts::{Observed, ReadSource};

fn round_trip(command: &OwnerCommand) -> OwnerCommand {
    let json = serde_json::to_string(command).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

#[test]
fn every_family_survives_the_wire() {
    let commands = [
        OwnerCommand::Roadmap(RoadmapCommand::SetGroup {
            id: "c".into(),
            group: None,
        }),
        OwnerCommand::Think(ThinkCommand::Checkpoint),
        OwnerCommand::Think(ThinkCommand::PinStep {
            step_number: 3,
            pinned: true,
        }),
        OwnerCommand::Ship(ShipCommand::AddTask {
            id: "t".into(),
            title: "Task".into(),
            task_type: TaskType::Implement,
            estimate: None,
        }),
        OwnerCommand::Signal(SignalCommand::Snooze {
            id: "s".into(),
            minutes: 5,
        }),
    ];
    for command in &commands {
        let back = round_trip(command);
        assert_eq!(
            serde_json::to_value(&back).expect("value"),
            serde_json::to_value(command).expect("value"),
            "{command:?}"
        );
    }
    let json = serde_json::to_value(&commands[1]).expect("value");
    assert_eq!(json["family"], "think");
    assert_eq!(json["verb"], "checkpoint");
}

#[test]
fn replies_that_carry_primitives_survive_the_wire() {
    for reply in [
        OwnerReply::Step(Box::new(magistr_workflow_contracts::think::RecordedStep {
            project_id: "project".into(),
            step: serde_json::from_value(serde_json::json!({
                "step_number": 1, "record_id": "record-1", "purpose": "investigation"
            }))
            .unwrap(),
            source: ReadSource::Memory,
            report: serde_json::json!({}),
        })),
        OwnerReply::Pinned(Observed {
            value: true,
            source: ReadSource::Memory,
        }),
        OwnerReply::Cleared(Observed {
            value: 2,
            source: ReadSource::Memory,
        }),
        OwnerReply::Report(serde_json::json!({"open": 1})),
        OwnerReply::Done(Observed {
            value: (),
            source: ReadSource::Memory,
        }),
    ] {
        let json = serde_json::to_string(&reply).expect("serialize");
        let back: OwnerReply = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            serde_json::to_string(&back).expect("again"),
            json,
            "{reply:?}"
        );
    }
}
