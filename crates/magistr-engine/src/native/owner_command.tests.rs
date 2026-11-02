use super::*;
use crate::native::{DecisionCommand, OwnerReply};
use magistr_workflow::run::domain::step::StepType;
use magistr_workflow_contracts::{Observed, ReadSource};

fn round_trip(command: &OwnerCommand) -> OwnerCommand {
    let json = serde_json::to_string(command).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

#[test]
fn every_family_survives_the_wire() {
    let commands = [
        OwnerCommand::Plan(PlanCommand::SetFeature {
            id: "c".into(),
            feature: None,
        }),
        OwnerCommand::Decision(DecisionCommand::Review),
        OwnerCommand::Decision(DecisionCommand::Pin {
            note: 3,
            pinned: true,
        }),
        OwnerCommand::Run(RunCommand::AddStep {
            id: "t".into(),
            title: "Step".into(),
            step_type: StepType::Implement,
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
    assert_eq!(json["family"], "decision");
    assert_eq!(json["verb"], "review");
}

#[test]
fn replies_that_carry_primitives_survive_the_wire() {
    for reply in [
        OwnerReply::Note(Box::new(
            magistr_workflow_contracts::decision::RecordedNote {
                project_id: "project".into(),
                note: serde_json::from_value(serde_json::json!({
                    "note": 1, "record_id": "record-1", "purpose": "investigation"
                }))
                .unwrap(),
                source: ReadSource::Memory,
                report: serde_json::json!({}),
            },
        )),
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
