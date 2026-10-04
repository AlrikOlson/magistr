use serde_json::json;

use super::*;
use crate::harness::fixtures;

#[test]
fn a_self_reported_check_is_not_upgraded_to_verified() {
    let check = fixtures::check_from(json!({"name":"review", "passed":true, "verified":false}));
    assert!(matches!(
        ShipMoment::check("", &check),
        ShipMoment::Check {
            verified: false,
            ..
        }
    ));
}

#[test]
fn an_action_keeps_its_task_kind_description_ordinal_and_files() {
    let mut action = fixtures::action(198, "wired the port");
    action.files_touched = vec!["src/port.rs".to_owned()];
    assert_eq!(
        ShipMoment::action("wire-port", &action),
        ShipMoment::Action {
            task_id: "wire-port".to_owned(),
            timestamp: String::new(),
            kind: "code".to_owned(),
            description: "wired the port".to_owned(),
            id: 198,
            files: vec!["src/port.rs".to_owned()],
            result: action.result.clone(),
            tools: Vec::new(),
            think_step: None,
        }
    );
}

#[test]
fn a_check_keeps_its_exit_code_and_the_reports_duration() {
    let check = fixtures::check_from(json!({
        "name": "test",
        "passed": false,
        "exit_code": 101,
        "results": {"duration_ms": 5400},
    }));
    assert_eq!(
        ShipMoment::check("", &check),
        ShipMoment::Check {
            task_id: String::new(),
            timestamp: String::new(),
            name: "test".to_owned(),
            passed: false,
            verified: true,
            exit_code: Some(101),
            duration_ms: Some(5400),
            command: None,
            details: check.details.clone(),
            required: check.required,
        }
    );
}

#[test]
fn a_check_without_a_report_has_no_duration() {
    let ShipMoment::Check {
        exit_code,
        duration_ms,
        ..
    } = ShipMoment::check("", &fixtures::check("lint", true))
    else {
        panic!("a check is a check");
    };
    assert_eq!(exit_code, None);
    assert_eq!(duration_ms, None);
}
