use super::*;

#[test]
fn shared_state_defaults_to_zero() {
    let state = SharedGuardState::new();
    assert_eq!(state.research_calls.load(Ordering::Relaxed), 0);
    assert_eq!(state.exploration_calls.load(Ordering::Relaxed), 0);
    assert_eq!(state.phase.load(Ordering::Relaxed), 0);
}

#[test]
fn record_research_increments() {
    let state = SharedGuardState::new();
    state.record_research();
    state.record_research();
    assert_eq!(state.research_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn record_exploration_increments() {
    let state = SharedGuardState::new();
    state.record_exploration();
    assert_eq!(state.exploration_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn can_transition_blocked_without_research() {
    let state = SharedGuardState::new();
    // Phase 0 (Researching), no research calls → blocked
    let result = state.can_transition("plan");
    assert!(!result.allowed);
    assert!(result.reason.is_some());
    assert!(result.reason.unwrap().contains("No research tool calls"));
}

#[test]
fn can_transition_allowed_with_research() {
    let state = SharedGuardState::new();
    state.record_research();
    let result = state.can_transition("plan");
    assert!(result.allowed);
    assert!(result.reason.is_none());
}

#[test]
fn can_transition_allowed_for_research_phase() {
    let state = SharedGuardState::new();
    // Setting phase to research is always allowed
    let result = state.can_transition("research");
    assert!(result.allowed);
}

#[test]
fn can_transition_allowed_when_not_in_research_phase() {
    let state = SharedGuardState::new();
    // Phase 1 (Planning) → execute transition doesn't need research check
    state.set_phase(1);
    let result = state.can_transition("execute");
    assert!(result.allowed);
}

#[test]
fn reset_clears_all_state() {
    let state = SharedGuardState::new();
    state.record_research();
    state.record_exploration();
    state.set_phase(3);
    state.reset();
    assert_eq!(state.research_calls.load(Ordering::Relaxed), 0);
    assert_eq!(state.exploration_calls.load(Ordering::Relaxed), 0);
    assert_eq!(state.phase.load(Ordering::Relaxed), 0);
}

#[test]
fn handle_query_can_transition() {
    let state = SharedGuardState::new();
    let result = handle_query(r#"{"query":"can_transition","to":"plan"}"#, &state);
    assert!(!result.allowed);

    state.record_research();
    let result = handle_query(r#"{"query":"can_transition","to":"plan"}"#, &state);
    assert!(result.allowed);
}

#[test]
fn handle_query_unknown_returns_allowed() {
    let state = SharedGuardState::new();
    let result = handle_query(r#"{"query":"unknown"}"#, &state);
    assert!(result.allowed);
}

#[test]
fn handle_query_invalid_json_returns_allowed() {
    let state = SharedGuardState::new();
    let result = handle_query("not json", &state);
    assert!(result.allowed);
}

#[test]
fn guard_socket_path_is_deterministic() {
    let path = guard_socket_path(Path::new("/tmp/project"));
    assert_eq!(
        path,
        PathBuf::from("/tmp/project/.magistr/workflow-guard.sock")
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn socket_listener_answers_query() {
    let dir = tempfile::TempDir::new().unwrap();
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();

    let state = SharedGuardState::new();
    let handle = start_listener(dir.path(), state.clone());
    assert!(handle.is_some());

    // Wait for listener to bind — poll until socket file exists
    let socket = guard_socket_path(dir.path());
    for _ in 0..20 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(
        socket.exists(),
        "socket file should exist after listener starts"
    );

    // Query: should be blocked (no research)
    let result = query_guard(dir.path(), "plan");
    assert!(result.is_some(), "query_guard should connect to socket");
    let result = result.unwrap();
    assert!(!result.allowed);

    // Record research and query again
    state.record_research();
    let result = query_guard(dir.path(), "plan").unwrap();
    assert!(result.allowed);

    // Handle drops, socket file should be cleaned up
    drop(handle);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!socket.exists(), "socket file should be cleaned up on drop");
}

#[test]
fn query_guard_returns_none_when_no_socket() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".magistr")).unwrap();
    // No socket listener running → query_guard should return None (non-workflow mode)
    let result = query_guard(dir.path(), "plan");
    assert!(result.is_none(), "should return None when no socket exists");
}

#[test]
fn can_transition_blocks_execute_without_research() {
    let state = SharedGuardState::new();
    // Phase 0 (Researching), no research → any forward transition blocked
    let result = state.can_transition("execute");
    assert!(!result.allowed);
}

#[test]
fn can_transition_blocks_verify_without_research() {
    let state = SharedGuardState::new();
    let result = state.can_transition("verify");
    assert!(!result.allowed);
}

#[test]
fn can_transition_blocks_done_without_research() {
    let state = SharedGuardState::new();
    let result = state.can_transition("done");
    assert!(!result.allowed);
}

#[test]
fn shared_state_is_clone_and_thread_safe() {
    let state = SharedGuardState::new();
    let clone = state.clone();

    // Mutation on clone is visible on original (shared Arc)
    clone.record_research();
    assert_eq!(state.research_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn guard_result_serializes_without_reason_when_allowed() {
    let result = GuardResult {
        allowed: true,
        reason: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("reason"), "reason should be skipped: {json}");
    assert!(json.contains("true"));
}

#[test]
fn guard_result_round_trips_through_json() {
    let original = GuardResult {
        allowed: false,
        reason: Some("blocked".into()),
    };
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: GuardResult = serde_json::from_str(&json).unwrap();
    assert!(!deserialized.allowed);
    assert_eq!(deserialized.reason.as_deref(), Some("blocked"));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn socket_handles_multiple_queries() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".magistr")).unwrap();

    let state = SharedGuardState::new();
    let _handle = start_listener(dir.path(), state.clone());

    let socket = guard_socket_path(dir.path());
    for _ in 0..20 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    // Multiple sequential queries should all work
    for _ in 0..3 {
        let result = query_guard(dir.path(), "plan");
        assert!(result.is_some());
        assert!(!result.unwrap().allowed);
    }

    state.record_research();
    for _ in 0..3 {
        let result = query_guard(dir.path(), "plan").unwrap();
        assert!(result.allowed);
    }
}
