use super::*;

fn final_slot() -> (TaskAuthority, WorkerPermit, ReconciliationObservation) {
    let mut fixed = contract();
    fixed.limits.max_attempts = 2;
    let mut plan = body();
    plan.tasks[1].dependencies.clear();
    let mut value = TaskAuthority::new(fixed, plan.clone(), observation()).unwrap();
    candidate(&mut value, plan);
    vote(&mut value, 0);
    vote(&mut value, 1);
    worker(&mut value, "build", 0, "first");
    value.apply(observation(), retire("first")).unwrap();
    let permit = worker(&mut value, "build", 0, "last");
    let observed = ReconciliationObservation {
        worker_attempts: 2,
        ..observation()
    };
    (value, permit, observed)
}

#[test]
fn a_single_allowed_start_can_finish_without_any_retry_allowance() {
    let mut fixed = contract();
    fixed.limits.max_attempts = 1;
    let mut plan = body();
    plan.tasks.truncate(1);
    plan.retry_allowance = 0;
    let mut value = TaskAuthority::new(fixed, plan.clone(), observation()).unwrap();
    candidate(&mut value, plan);
    vote(&mut value, 0);
    vote(&mut value, 1);
    let permit = worker(&mut value, "build", 0, "only");
    let observed = ReconciliationObservation {
        worker_attempts: 1,
        ..observation()
    };
    value
        .validate_worker(observed, permit.lease().owner(), &permit)
        .unwrap();
    assert!(matches!(
        value.apply(observed, accept(&permit)).unwrap(),
        AuthorityOutcome::Updated
    ));
    assert_eq!(value.worker_receipt("only"), Some("check:verified-by-host"));
    assert_eq!(
        value
            .reconciliation()
            .revision()
            .contract()
            .limits
            .max_attempts,
        1
    );
}

fn legacy(bytes: &[u8]) -> Vec<u8> {
    // Everything in this prefix occurred below the limit and is valid v1 history.
    let mut journal: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    journal["schema_version"] = 1.into();
    serde_json::to_vec(&journal).unwrap()
}

#[test]
fn exact_attempt_ceiling_does_not_waive_result_fences() {
    for case in 0..5 {
        let (mut value, permit, mut observed) = final_slot();
        let mut acceptance = accept(&permit);
        match case {
            0 => {
                let action = object(&value, "concern", &["build"], 1);
                value.apply(observed, action).unwrap();
            }
            1 => {
                if let AuthorityAction::AcceptWorker { peer, .. } = &mut acceptance {
                    *peer = contract().peers[1].clone();
                }
            }
            2 => observed.now_unix_ms = permit.lease().expires_unix_ms(),
            3 => {
                let action = issue(&value, "build", 0, 200);
                value.apply(observed, action).unwrap();
            }
            _ => {
                if let AuthorityAction::AcceptWorker { receipt, .. } = &mut acceptance {
                    receipt.clear();
                }
            }
        }
        assert!(value.apply(observed, acceptance).is_err(), "case {case}");
        assert_eq!(value.worker_receipt("last"), None);
        value.apply(observed, retire("last")).unwrap();
        assert_eq!(value.unsettled_workers().count(), 0);
    }
}

#[test]
fn final_attempt_still_stops_for_time_tokens_outage_or_excess_starts() {
    for case in 0..4 {
        let (mut value, permit, mut observed) = final_slot();
        let mut action = accept(&permit);
        let expected = match case {
            0 => {
                observed.now_unix_ms = contract().limits.deadline_unix_ms;
                ReconciliationStop::Deadline
            }
            1 => {
                observed.total_tokens = contract().limits.max_total_tokens;
                ReconciliationStop::TokensExhausted
            }
            2 => {
                action = AuthorityAction::Protocol {
                    action: ReconciliationAction::PeerUnavailable {
                        peer: contract().peers[1].clone(),
                    },
                };
                ReconciliationStop::PeerUnavailable(Peer::Fable)
            }
            _ => {
                observed.worker_attempts += 1;
                ReconciliationStop::AttemptsExhausted
            }
        };
        assert!(matches!(
            value.apply(observed, action).unwrap(),
            AuthorityOutcome::Stopped(reason) if reason == expected
        ));
        assert!(
            value
                .validate_worker(observed, permit.lease().owner(), &permit)
                .is_err()
        );
        assert_eq!(value.worker_receipt("last"), None);
        value.apply(observed, retire("last")).unwrap();
        let restored = TaskAuthority::from_json(
            &value.to_json().unwrap(),
            value.reconciliation().revision().contract(),
            observed,
        )
        .unwrap();
        assert_eq!(
            restored.reconciliation().state(),
            ReconciliationState::Stopped(expected)
        );
    }
}

#[test]
fn exact_ceiling_replay_keeps_capacity_used_and_discards_worker_authority() {
    let (mut value, permit, observed) = final_slot();
    value.apply(observed, AuthorityAction::observe()).unwrap();
    let fixed = value.reconciliation().revision().contract().clone();
    let mut restored =
        TaskAuthority::from_json(&value.to_json().unwrap(), &fixed, observed).unwrap();
    assert_eq!(
        restored.reconciliation().state(),
        ReconciliationState::Agreed
    );
    assert!(restored.apply(observed, accept(&permit)).is_err());
    restored.apply(observed, retire("last")).unwrap();
    let action = issue(&restored, "review", 1, 100);
    let lease = match restored.apply(observed, action).unwrap() {
        AuthorityOutcome::Lease(lease) => lease,
        other => panic!("{other:?}"),
    };
    assert!(matches!(
        restored.apply(observed, admit(&lease, "extra")),
        Err(AgreementError::Invalid(message)) if message == "worker attempt reservations exhausted"
    ));
    assert_eq!(
        restored.worker_receipt("last"),
        Some("runtime:termination-confirmed")
    );
}

#[test]
fn legacy_authority_replay_never_revives_exact_ceiling_stops() {
    let (value, _, observed) = final_slot();
    let fixed = value.reconciliation().revision().contract();
    let bytes = legacy(&value.to_json().unwrap());
    let mut restored = TaskAuthority::from_json(&bytes, fixed, observation()).unwrap();
    assert_eq!(
        restored.reconciliation().state(),
        ReconciliationState::Agreed
    );
    assert!(matches!(
        restored
            .apply(observed, AuthorityAction::observe())
            .unwrap(),
        AuthorityOutcome::Stopped(ReconciliationStop::AttemptsExhausted)
    ));
    for _ in 0..2 {
        restored = TaskAuthority::from_json(&restored.to_json().unwrap(), fixed, observed).unwrap();
        assert_eq!(
            restored.reconciliation().state(),
            ReconciliationState::Stopped(ReconciliationStop::AttemptsExhausted)
        );
    }
    restored.apply(observed, retire("last")).unwrap();
}

#[test]
fn reconciliation_journal_version_preserves_exact_ceiling_policy() {
    let (value, _, observed) = final_slot();
    let fixed = value.reconciliation().revision().contract();
    let bytes = value.reconciliation().to_json().unwrap();
    let mut current = Reconciliation::from_json(&bytes, fixed, observed).unwrap();
    assert_eq!(current.state(), ReconciliationState::Agreed);
    // Repeated restoration retains v2 behavior, with no loss of dual approval.
    current = Reconciliation::from_json(&current.to_json().unwrap(), fixed, observed).unwrap();
    assert_eq!(current.state(), ReconciliationState::Agreed);
    let mut old = Reconciliation::from_json(&legacy(&bytes), fixed, observation()).unwrap();
    assert_eq!(old.state(), ReconciliationState::Agreed);
    old.apply(observed, ReconciliationAction::Observe).unwrap();
    for _ in 0..2 {
        old = Reconciliation::from_json(&old.to_json().unwrap(), fixed, observed).unwrap();
        assert_eq!(
            old.state(),
            ReconciliationState::Stopped(ReconciliationStop::AttemptsExhausted)
        );
    }
}
