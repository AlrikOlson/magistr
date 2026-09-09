use super::*;

#[test]
fn worker_admission_never_treats_one_vote_as_consent() {
    let mut value = TaskAuthority::new(contract(), body(), observation()).unwrap();
    candidate(&mut value, body());
    vote(&mut value, 0);
    let request = AuthorityAction::AdmitWorker {
        peer: contract().peers[0].clone(),
        plan: value.reconciliation().revision().reference(),
        task: "build".into(),
        lease_generation: 1,
        id: "unapproved".into(),
        worker: body().worker_models[0].clone(),
    };
    assert!(value.apply(observation(), request).is_err());
    assert_eq!(value.unsettled_workers().count(), 0);
}

#[test]
fn worker_ownership_transfer_keeps_attempt_history_and_requires_settlement() {
    let mut value = agreed();
    let old = worker(&mut value, "build", 0, "old");
    let plan = value.reconciliation().revision().reference();
    value
        .apply(
            observation(),
            AuthorityAction::Protocol {
                action: ReconciliationAction::Disagree {
                    peer: contract().peers[1].clone(),
                    plan,
                    reason: "transfer".into(),
                },
            },
        )
        .unwrap();
    let mut changed = graph();
    changed.tasks[0].owner = Peer::Fable;
    candidate(&mut value, changed);
    vote(&mut value, 0);
    vote(&mut value, 1);
    assert!(value.apply(observation(), accept(&old)).is_err());
    let fresh = lease(&mut value, "build", 1);
    assert!(value.apply(observation(), admit(&fresh, "new")).is_err());
    value.apply(observation(), retire("old")).unwrap();
    assert!(value.apply(observation(), admit(&fresh, "old")).is_err());
    let new = match value.apply(observation(), admit(&fresh, "new")).unwrap() {
        AuthorityOutcome::Worker(permit) => permit,
        other => panic!("{other:?}"),
    };
    assert_eq!(new.attempt(), 2);
    assert_eq!(new.lease().owner().peer, Peer::Fable);
    value.apply(observation(), retire("new")).unwrap();
    assert!(value.apply(observation(), admit(&fresh, "third")).is_err());
}

#[test]
fn worker_revision_changes_invalidate_prior_dependency_acceptance() {
    let mut value = agreed();
    let build = worker(&mut value, "build", 0, "build");
    value.apply(observation(), accept(&build)).unwrap();
    let plan = value.reconciliation().revision().reference();
    value
        .apply(
            observation(),
            AuthorityAction::Protocol {
                action: ReconciliationAction::Disagree {
                    peer: contract().peers[0].clone(),
                    plan,
                    reason: "new acceptance".into(),
                },
            },
        )
        .unwrap();
    let mut changed = graph();
    changed.tasks[0]
        .acceptance
        .push("additional verified check".into());
    candidate(&mut value, changed);
    vote(&mut value, 0);
    vote(&mut value, 1);
    let left = lease(&mut value, "left", 1);
    assert!(value.apply(observation(), admit(&left, "left")).is_err());
    assert_eq!(
        value.worker_receipt("build"),
        Some("check:verified-by-host")
    );
}

#[test]
fn worker_expiry_and_foreign_receipts_cannot_accept_but_stops_allow_termination() {
    let mut value = agreed();
    let permit = worker(&mut value, "build", 0, "work");
    for bad in [
        AuthorityAction::AcceptWorker {
            peer: contract().peers[1].clone(),
            id: "work".into(),
            receipt: "check:ok".into(),
        },
        AuthorityAction::AcceptWorker {
            peer: contract().peers[0].clone(),
            id: "work".into(),
            receipt: " ".into(),
        },
        AuthorityAction::RetireWorker {
            id: "unknown".into(),
            receipt: "runtime:stopped".into(),
        },
        AuthorityAction::RetireWorker {
            id: "work".into(),
            receipt: "".into(),
        },
    ] {
        assert!(value.apply(observation(), bad).is_err());
    }
    let mut expired = observation();
    expired.now_unix_ms = permit.lease().expires_unix_ms();
    assert!(value.apply(expired, accept(&permit)).is_err());
    assert!(
        value
            .validate_worker(expired, &contract().peers[0], &permit)
            .is_err()
    );
    let mut stopped = expired;
    stopped.now_unix_ms = contract().limits.deadline_unix_ms;
    assert!(matches!(
        value
            .apply(
                stopped,
                AuthorityAction::Protocol {
                    action: ReconciliationAction::Observe
                }
            )
            .unwrap(),
        AuthorityOutcome::Stopped(_)
    ));
    value.apply(stopped, retire("work")).unwrap();
    assert_eq!(value.unsettled_workers().count(), 0);
    assert_eq!(
        value.worker_receipt("work"),
        Some("runtime:termination-confirmed")
    );
    assert!(value.apply(stopped, retire("work")).is_err());
    let restored =
        TaskAuthority::from_json(&value.to_json().unwrap(), &contract(), stopped).unwrap();
    assert_eq!(
        restored.worker_receipt("work"),
        value.worker_receipt("work")
    );
}

#[test]
fn worker_replay_rejects_tampering_and_accepts_old_authority_journals() {
    let mut value = agreed();
    let old_journal = value.to_json().unwrap();
    assert!(TaskAuthority::from_json(&old_journal, &contract(), observation()).is_ok());
    let permit = worker(&mut value, "build", 0, "work");
    value.apply(observation(), accept(&permit)).unwrap();
    let original: serde_json::Value = serde_json::from_slice(&value.to_json().unwrap()).unwrap();
    for field in 0..6 {
        let mut corrupt = original.clone();
        let entries = corrupt["entries"].as_array_mut().unwrap();
        let admission = entries
            .iter()
            .position(|e| e["action"]["kind"] == "admit_worker")
            .unwrap();
        let acceptance = entries
            .iter()
            .position(|e| e["action"]["kind"] == "accept_worker")
            .unwrap();
        match field {
            0 => entries[admission]["action"]["lease_generation"] = 999.into(),
            1 => entries[admission]["action"]["worker"]["effort"] = "unknown".into(),
            2 => entries[acceptance]["action"]["id"] = "unknown".into(),
            3 => entries[acceptance]["action"]["receipt"] = "".into(),
            4 => {
                entries.swap(admission, acceptance);
            }
            _ => entries[admission]["action"]["extra"] = true.into(),
        }
        assert!(
            TaskAuthority::from_json(
                &serde_json::to_vec(&corrupt).unwrap(),
                &contract(),
                observation()
            )
            .is_err(),
            "case {field}"
        );
    }
    let restored = TaskAuthority::from_json(
        &serde_json::to_vec(&original).unwrap(),
        &contract(),
        observation(),
    )
    .unwrap();
    assert_eq!(
        restored.worker_receipt("work"),
        Some("check:verified-by-host")
    );
}

#[test]
fn worker_reservations_use_observed_floor_and_never_overflow() {
    let mut fixed = contract();
    fixed.limits.max_attempts = u32::MAX;
    let mut observed = observation();
    observed.worker_attempts = u32::MAX - 1;
    let mut value = TaskAuthority::new(fixed, body(), observation()).unwrap();
    candidate(&mut value, body());
    vote(&mut value, 0);
    vote(&mut value, 1);
    let lease = lease(&mut value, "build", 0);
    let permit = match value.apply(observed, admit(&lease, "last")).unwrap() {
        AuthorityOutcome::Worker(permit) => permit,
        other => panic!("{other:?}"),
    };
    observed.worker_attempts = u32::MAX;
    value
        .validate_worker(observed, permit.lease().owner(), &permit)
        .unwrap();
    value.apply(observed, accept(&permit)).unwrap();
    assert!(value.apply(observed, admit(&lease, "overflow")).is_err());
    let restored = TaskAuthority::from_json(
        &value.to_json().unwrap(),
        value.reconciliation().revision().contract(),
        observed,
    )
    .unwrap();
    assert_eq!(
        restored.worker_receipt("last"),
        Some("check:verified-by-host")
    );
}

#[test]
fn worker_zero_retry_allowance_still_allows_exactly_one_start() {
    let mut plan = body();
    plan.retry_allowance = 0;
    let mut value = TaskAuthority::new(contract(), plan.clone(), observation()).unwrap();
    candidate(&mut value, plan);
    vote(&mut value, 0);
    vote(&mut value, 1);
    let first = worker(&mut value, "build", 0, "first");
    value.apply(observation(), retire(first.id())).unwrap();
    let lease = lease(&mut value, "build", 0);
    assert!(value.apply(observation(), admit(&lease, "retry")).is_err());
}

#[test]
fn worker_observed_attempt_ceiling_accepts_final_result_without_another_start() {
    let mut fixed = contract();
    fixed.limits.max_attempts = 2;
    let mut value = TaskAuthority::new(fixed.clone(), body(), observation()).unwrap();
    candidate(&mut value, body());
    vote(&mut value, 0);
    vote(&mut value, 1);
    worker(&mut value, "build", 0, "first");
    let mut observed = observation();
    observed.worker_attempts = 1;
    value.apply(observed, retire("first")).unwrap();
    let action = issue(&value, "build", 0, 100);
    let fresh = match value.apply(observed, action).unwrap() {
        AuthorityOutcome::Lease(lease) => lease,
        other => panic!("{other:?}"),
    };
    let final_permit = match value.apply(observed, admit(&fresh, "last")).unwrap() {
        AuthorityOutcome::Worker(permit) => permit,
        other => panic!("{other:?}"),
    };
    observed.worker_attempts = 2;
    value
        .validate_worker(observed, &fixed.peers[0], &final_permit)
        .unwrap();
    assert!(matches!(
        value.apply(observed, admit(&fresh, "last")).unwrap(),
        AuthorityOutcome::AlreadyAdmitted
    ));
    assert!(matches!(
        value.apply(observed, accept(&final_permit)).unwrap(),
        AuthorityOutcome::Updated
    ));
    assert_eq!(value.worker_receipt("last"), Some("check:verified-by-host"));
    // Acceptance unblocks review, but cannot provide another start.
    let action = issue(&value, "review", 1, 100);
    let review = match value.apply(observed, action).unwrap() {
        AuthorityOutcome::Lease(lease) => lease,
        other => panic!("{other:?}"),
    };
    assert!(value.apply(observed, admit(&review, "extra")).is_err());
    assert_eq!(value.unsettled_workers().count(), 0);
    let restored = TaskAuthority::from_json(&value.to_json().unwrap(), &fixed, observed).unwrap();
    assert_eq!(
        restored.reconciliation().state(),
        ReconciliationState::Agreed
    );
    assert_eq!(
        restored.worker_receipt("last"),
        Some("check:verified-by-host")
    );
}

#[test]
fn worker_restore_preserves_accepted_dependencies_and_renewal_fences_old_results() {
    let mut value = agreed();
    let build = worker(&mut value, "build", 0, "build");
    value.apply(observation(), accept(&build)).unwrap();
    let mut restored =
        TaskAuthority::from_json(&value.to_json().unwrap(), &contract(), observation()).unwrap();
    let left = worker(&mut restored, "left", 1, "left");
    lease(&mut restored, "left", 1);
    assert!(restored.apply(observation(), accept(&left)).is_err());
    assert!(
        restored
            .validate_worker(observation(), &contract().peers[1], &left)
            .is_err()
    );
    assert_eq!(restored.unsettled_workers().count(), 1);
}

#[test]
fn worker_replay_preserves_settled_reservations_without_a_fresh_usage_count() {
    let mut fixed = contract();
    fixed.limits.max_attempts = 2;
    let mut value = TaskAuthority::new(fixed.clone(), body(), observation()).unwrap();
    candidate(&mut value, body());
    vote(&mut value, 0);
    vote(&mut value, 1);
    worker(&mut value, "build", 0, "retired");
    value.apply(observation(), retire("retired")).unwrap();
    let accepted = worker(&mut value, "build", 0, "accepted");
    assert_eq!(accepted.attempt(), 2);
    value.apply(observation(), accept(&accepted)).unwrap();

    // The restarted host has observed no starts. Admission must reconstruct
    // consumed capacity from journaled reservations, including settled ones.
    assert_eq!(observation().worker_attempts, 0);
    for _ in 0..2 {
        value = TaskAuthority::from_json(&value.to_json().unwrap(), &fixed, observation()).unwrap();
        assert_eq!(value.reconciliation().state(), ReconciliationState::Agreed);
        assert_eq!(value.unsettled_workers().count(), 0);
        assert_eq!(
            value.worker_receipt("retired"),
            Some("runtime:termination-confirmed")
        );
        assert_eq!(
            value.worker_receipt("accepted"),
            Some("check:verified-by-host")
        );

        // The accepted build unblocks review and its fresh lease is valid.
        // Only the persisted attempt ceiling must prevent another start.
        let review = lease(&mut value, "review", 1);
        assert!(matches!(
            value.apply(observation(), admit(&review, "extra")),
            Err(AgreementError::Invalid(message))
                if message == "worker attempt reservations exhausted"
        ));
        assert!(value.apply(observation(), retire("accepted")).is_err());
    }
}
