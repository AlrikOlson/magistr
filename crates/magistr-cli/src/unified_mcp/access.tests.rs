use super::*;
use serde_json::json;
#[test]
fn planning_checks_each_effect_and_cannot_start_or_execute_source() {
    for name in [
        "write",
        "edit",
        "files",
        "script",
        "run",
        "start_step",
        "check_step",
        "finish_step",
        "run_finish",
        "plan_start_task",
        "plan_export",
    ] {
        assert!(!Access::Planning.permits(name, &json!({})), "{name}");
    }
    assert!(Access::Planning.permits("plan_add_task", &json!({"title":"Plan"})));
    assert!(!Access::Planning.permits("plan_add_task", &json!({"status":"done"})));
    assert!(!Access::Planning.permits("plan_update_task", &json!({"status":"in_progress"})));
    assert!(!Access::ReadOnly.permits("request_capture", &json!({})));
    assert!(Access::ReadOnly.permits("context", &json!({})));
}
