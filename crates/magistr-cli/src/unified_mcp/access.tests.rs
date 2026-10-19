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
        "start_work",
        "check_work",
        "finish_work",
        "ship_finalize",
        "roadmap_start_chunk",
        "roadmap_export",
    ] {
        assert!(!Access::Planning.permits(name, &json!({})), "{name}");
    }
    assert!(Access::Planning.permits("roadmap_add_chunk", &json!({"title":"Plan"})));
    assert!(!Access::Planning.permits("roadmap_add_chunk", &json!({"status":"done"})));
    assert!(!Access::Planning.permits("roadmap_update_chunk", &json!({"status":"in_progress"})));
    assert!(!Access::ReadOnly.permits("signal_capture", &json!({})));
    assert!(Access::ReadOnly.permits("context", &json!({})));
}
