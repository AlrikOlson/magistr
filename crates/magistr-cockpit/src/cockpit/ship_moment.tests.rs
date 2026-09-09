use serde_json::json;

use super::*;

#[test]
fn an_action_frame_keeps_task_kind_and_description() {
    let moment = ShipMoment::from_action(&json!({
        "task_id": "wire-port",
        "action": {
            "id": 198,
            "action_type": "code",
            "description": "wired the port",
            "files_touched": ["src/port.rs"],
        },
    }));
    assert_eq!(
        moment,
        ShipMoment::Action {
            task_id: "wire-port".to_owned(),
            kind: "code".to_owned(),
            description: "wired the port".to_owned(),
            id: Some(198),
            files: vec!["src/port.rs".to_owned()],
        }
    );
}

#[test]
fn a_check_frame_keeps_exit_code_and_the_results_duration() {
    let moment = ShipMoment::from_check(&json!({
        "check": {
            "name": "test",
            "passed": false,
            "exit_code": 101,
            "results": {"duration_ms": 5400},
        },
    }));
    assert_eq!(
        moment,
        ShipMoment::Check {
            name: "test".to_owned(),
            passed: false,
            exit_code: Some(101),
            duration_ms: Some(5400),
        }
    );
}

#[test]
fn a_flat_duration_is_read_when_no_results_carry_one() {
    let ShipMoment::Check { duration_ms, .. } = ShipMoment::from_check(&json!({
        "check": {"name": "lint", "passed": true, "duration_ms": 900},
    })) else {
        panic!("a check parses as a check");
    };
    assert_eq!(duration_ms, Some(900));
}

#[test]
fn empty_payloads_default_rather_than_drop() {
    assert_eq!(
        ShipMoment::from_check(&json!({})),
        ShipMoment::Check {
            name: "check".to_owned(),
            passed: false,
            exit_code: None,
            duration_ms: None,
        }
    );
    assert_eq!(
        ShipMoment::from_action(&json!({})),
        ShipMoment::Action {
            task_id: String::new(),
            kind: String::new(),
            description: String::new(),
            id: None,
            files: Vec::new(),
        }
    );
}
