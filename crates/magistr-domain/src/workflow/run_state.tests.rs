use super::*;

#[test]
fn mode_decides_the_same_observation_sequence() {
    for mode in [RunMode::Conversation, RunMode::OneChunk, RunMode::ChunkLoop] {
        let mut run = RunState::new(mode, 10, None);
        run.record_commit(false);
        assert_eq!(
            run.stop_reason(),
            (mode == RunMode::OneChunk).then_some(StopReason::OneChunkComplete)
        );
        run.advance_attempt();
        run.record_commit(false);
        assert_eq!(
            run.stop_reason(),
            match mode {
                RunMode::Conversation => None,
                RunMode::OneChunk => Some(StopReason::OneChunkComplete),
                RunMode::ChunkLoop => Some(StopReason::NoProgress),
            }
        );
    }
}

#[test]
fn duplicate_settlement_and_recovery_cannot_manufacture_progress_or_failure() {
    let mut run = RunState::new(RunMode::ChunkLoop, 10, None);
    run.record_commit(false);
    let mut restored: RunState =
        serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    restored.record_commit(false);
    assert_eq!(restored.consecutive_no_commits, 1);
    restored.advance_attempt();
    restored.record_commit(false);
    assert_eq!(restored.stop_reason(), Some(StopReason::NoProgress));
    restored.record_commit(true);
    assert_eq!(restored.consecutive_no_commits, 0);
    restored.record_commit(false);
    assert_eq!(restored.consecutive_no_commits, 0);
}

#[test]
fn recovery_preserves_budget_limit_and_iteration_ceiling() {
    let mut run = RunState::new(RunMode::ChunkLoop, 2, Some(3.0));
    run.observe_budget(2.5);
    run.advance_attempt();
    let mut restored: RunState =
        serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    restored.observe_budget(0.5);
    assert_eq!(restored.budget_total_usd, 2.5);
    restored.advance_attempt();
    assert_eq!(restored.stop_reason(), Some(StopReason::IterationLimit));
    restored.observe_budget(3.0);
    assert_eq!(restored.stop_reason(), Some(StopReason::BudgetExceeded));
}

#[test]
fn only_pause_can_resume_and_terminal_reasons_survive_later_events() {
    for reason in [
        StopReason::Cancelled,
        StopReason::Failed,
        StopReason::Aborted,
        StopReason::NoReadyChunk,
        StopReason::SpecComplete,
        StopReason::Paused,
    ] {
        let mut run = RunState::new(RunMode::ChunkLoop, 10, None);
        run.stop(reason);
        if reason == StopReason::Paused {
            assert!(run.resume());
            assert_eq!(run.stop_reason(), None);
        } else {
            run.stop(StopReason::Paused);
            run.record_commit(true);
            assert!(!run.resume());
            assert_eq!(run.stop_reason(), Some(reason));
        }
    }
}

#[test]
fn missing_progress_evidence_and_impossible_budget_are_rejected() {
    let run = RunState::new(RunMode::ChunkLoop, 10, None);
    for field in [
        "consecutive_no_commits",
        "terminal_reason",
        "budget_limit_usd",
        "last_settled_attempt",
    ] {
        let mut value = serde_json::to_value(&run).unwrap();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<RunState>(value).is_err(),
            "{field}"
        );
    }
    let mut bad = run;
    bad.budget_total_usd = -1.0;
    assert!(bad.validate().is_err());
    assert_eq!(bad.stop_reason(), Some(StopReason::BudgetExceeded));
    bad.budget_total_usd = 0.0;
    bad.last_settled_attempt = Some(1);
    assert!(bad.validate().is_err());
}

#[test]
fn live_recovery_cannot_erase_observations_or_weaken_limits() {
    let mut known = RunState::new(RunMode::ChunkLoop, 10, Some(3.0));
    known.advance_attempt();
    known.record_commit(false);
    known.observe_budget(2.0);
    for field in 0..7 {
        let mut incoming = known.clone();
        match field {
            0 => incoming.completed_attempts = 0,
            1 => incoming.max_iterations = 11,
            2 => incoming.consecutive_no_commits = 0,
            3 => incoming.budget_total_usd = 1.0,
            4 => incoming.budget_limit_usd = None,
            5 => incoming.last_settled_attempt = None,
            _ => incoming.mode = RunMode::Conversation,
        }
        assert!(incoming.validate_recovery(&known).is_err(), "field {field}");
    }
    let mut tighter = known.clone();
    tighter.budget_limit_usd = Some(2.5);
    assert!(tighter.validate_recovery(&known).is_ok());
    known.stop(StopReason::Cancelled);
    assert!(tighter.validate_recovery(&known).is_err());
}
