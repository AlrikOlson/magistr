use super::*;
use crate::utils::check::{CheckRun, CheckVerdict};
use crate::validate::{ProjectValidation, StandardCheck};
use std::time::Duration;

fn make_result(checks: Vec<(StandardCheck, bool, f64)>) -> ProjectValidation {
    ProjectValidation {
        checks: vec![],
        results: checks
            .into_iter()
            .map(|(check, passed, secs)| CheckRun {
                name: check.label().into(),
                verdict: if passed {
                    CheckVerdict::Passed
                } else {
                    CheckVerdict::Failed
                },
                required: true,
                exit_code: if passed { 0 } else { 1 },
                rows: vec![],
                queue_duration: std::time::Duration::ZERO,
                duration: Duration::from_secs_f64(secs),
                output: if passed {
                    String::new()
                } else {
                    "error: something failed\nline 2".to_string()
                },
                command: format!("cargo {}", check.label()),
            })
            .collect(),
    }
}

#[test]
fn render_checks_json_all_passed() {
    let result = make_result(vec![
        (StandardCheck::Check, true, 2.3),
        (StandardCheck::Test, true, 15.1),
        (StandardCheck::Lint, true, 1.8),
    ]);
    let duration = Duration::from_secs_f64(19.2);

    let mut buf = Vec::new();
    render_checks_json(&mut buf, &result, duration).unwrap();
    let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();

    assert_eq!(output["type"], "check_report");
    assert_eq!(output["all_passed"], true);
    assert_eq!(output["passed_count"], 3);
    assert_eq!(output["failed_count"], 0);
    assert!(output["checks"]["check"]["passed"].as_bool().unwrap());
    assert!(output["checks"]["test"]["passed"].as_bool().unwrap());
    assert!(output["checks"]["lint"]["passed"].as_bool().unwrap());
}

#[test]
fn render_checks_json_with_failure() {
    let result = make_result(vec![
        (StandardCheck::Check, true, 2.0),
        (StandardCheck::Test, false, 10.0),
        (StandardCheck::Lint, true, 1.0),
    ]);
    let duration = Duration::from_secs_f64(13.0);

    let mut buf = Vec::new();
    render_checks_json(&mut buf, &result, duration).unwrap();
    let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();

    assert_eq!(output["all_passed"], false);
    assert_eq!(output["passed_count"], 2);
    assert_eq!(output["failed_count"], 1);
    assert!(!output["checks"]["test"]["passed"].as_bool().unwrap());
}

#[test]
fn render_checks_plain_all_passed() {
    let result = make_result(vec![
        (StandardCheck::Check, true, 2.3),
        (StandardCheck::Test, true, 15.1),
        (StandardCheck::Lint, true, 1.8),
    ]);
    let duration = Duration::from_secs_f64(19.2);

    let mut buf = Vec::new();
    render_checks_plain(&mut buf, &result, duration).unwrap();
    let output = String::from_utf8(buf).unwrap();

    assert!(output.contains("PASS"));
    assert!(output.contains("check"));
    assert!(output.contains("test"));
    assert!(output.contains("lint"));
    assert!(!output.contains("FAIL"));
}

#[test]
fn render_checks_plain_with_failure() {
    let result = make_result(vec![
        (StandardCheck::Check, true, 2.0),
        (StandardCheck::Test, false, 10.0),
    ]);
    let duration = Duration::from_secs_f64(12.0);

    let mut buf = Vec::new();
    render_checks_plain(&mut buf, &result, duration).unwrap();
    let output = String::from_utf8(buf).unwrap();

    assert!(output.contains("FAIL"));
    assert!(output.contains("error: something failed"));
    assert!(output.contains("1/2 checks failed"));
}
