use super::*;

#[path = "worker_admission.tests.rs"]
mod worker_admission;

fn observation() -> ReconciliationObservation {
    ReconciliationObservation {
        now_unix_ms: 1,
        total_tokens: 0,
        worker_attempts: 0,
    }
}

fn graph() -> PlanBody {
    let mut plan = body();
    for (id, dependencies) in [
        ("left", vec!["build"]),
        ("right", vec!["build"]),
        ("join", vec!["left", "right"]),
        ("independent", vec![]),
    ] {
        plan.tasks.push(PlanTask {
            id: id.into(),
            description: id.into(),
            dependencies: dependencies.into_iter().map(str::to_owned).collect(),
            owner: Peer::Fable,
            acceptance: vec!["verified".into()],
            risks: vec![],
        });
    }
    plan
}

fn candidate(value: &mut TaskAuthority, plan: PlanBody) {
    let base = value.reconciliation().revision().reference();
    for peer in &contract().peers {
        value
            .apply(
                observation(),
                AuthorityAction::Protocol {
                    action: ReconciliationAction::Propose {
                        peer: peer.clone(),
                        base: base.clone(),
                        body: Box::new(plan.clone()),
                    },
                },
            )
            .unwrap();
    }
    value
        .apply(
            observation(),
            AuthorityAction::Protocol {
                action: ReconciliationAction::Reconcile {
                    base,
                    body: Box::new(plan),
                },
            },
        )
        .unwrap();
}

fn vote(value: &mut TaskAuthority, index: usize) {
    value
        .apply(
            observation(),
            AuthorityAction::Protocol {
                action: ReconciliationAction::Approve {
                    peer: contract().peers[index].clone(),
                    plan: value.reconciliation().revision().reference(),
                },
            },
        )
        .unwrap();
}

fn agreed() -> TaskAuthority {
    let mut value = TaskAuthority::new(contract(), graph(), observation()).unwrap();
    candidate(&mut value, graph());
    vote(&mut value, 0);
    vote(&mut value, 1);
    value
}

fn issue(value: &TaskAuthority, task: &str, owner: usize, expires: u64) -> AuthorityAction {
    AuthorityAction::IssueLease {
        peer: contract().peers[owner].clone(),
        plan: value.reconciliation().revision().reference(),
        task: task.into(),
        expires_unix_ms: expires,
    }
}

fn lease(value: &mut TaskAuthority, task: &str, owner: usize) -> InstructionLease {
    let action = issue(value, task, owner, 100);
    match value.apply(observation(), action).unwrap() {
        AuthorityOutcome::Lease(lease) => *lease,
        other => panic!("expected lease, got {other:?}"),
    }
}

fn object(value: &TaskAuthority, id: &str, tasks: &[&str], peer: usize) -> AuthorityAction {
    AuthorityAction::Object {
        peer: contract().peers[peer].clone(),
        plan: value.reconciliation().revision().reference(),
        id: id.into(),
        tasks: tasks.iter().map(|s| (*s).into()).collect(),
        reason: "acceptance needs evidence".into(),
    }
}

fn resolve(value: &TaskAuthority, id: &str, peer: usize) -> AuthorityAction {
    AuthorityAction::Resolve {
        peer: contract().peers[peer].clone(),
        plan: value.reconciliation().revision().reference(),
        id: id.into(),
    }
}

#[test]
fn objections_fence_the_diamond_but_preserve_independent_authority() {
    for peer in 0..2 {
        let mut value = agreed();
        let old = lease(&mut value, "build", 0);
        let independent = lease(&mut value, "independent", 1);
        let action = object(&value, "concern", &["build"], peer);
        value.apply(observation(), action).unwrap();
        assert_eq!(
            value.paused_tasks(),
            ["build", "join", "left", "review", "right"]
                .map(String::from)
                .into()
        );
        assert!(
            value
                .validate_lease(observation(), &contract().peers[0], &old)
                .is_err()
        );
        value
            .validate_lease(observation(), &contract().peers[1], &independent)
            .unwrap();
        let action = issue(&value, "join", 1, 100);
        assert!(value.apply(observation(), action).is_err());
        let action = resolve(&value, "concern", peer);
        value.apply(observation(), action).unwrap();
        assert!(value.paused_tasks().is_empty());
        assert!(
            value
                .validate_lease(observation(), &contract().peers[0], &old)
                .is_err()
        );
        let fresh = lease(&mut value, "build", 0);
        assert!(fresh.generation() > old.generation());
    }
}

#[test]
fn overlapping_objections_have_equal_independent_owners() {
    let mut value = agreed();
    for action in [
        object(&value, "a", &["build"], 0),
        object(&value, "b", &["right"], 1),
    ] {
        value.apply(observation(), action).unwrap();
    }
    let before = value.to_json().unwrap();
    let wrong = resolve(&value, "a", 1);
    assert!(value.apply(observation(), wrong).is_err());
    assert_eq!(value.to_json().unwrap(), before);
    let action = resolve(&value, "a", 0);
    value.apply(observation(), action).unwrap();
    assert_eq!(
        value.paused_tasks(),
        ["join", "right"].map(String::from).into()
    );
    let action = resolve(&value, "b", 1);
    value.apply(observation(), action).unwrap();
    assert!(value.paused_tasks().is_empty());
}

#[test]
fn lease_requires_both_votes_exact_owner_and_bounded_expiry() {
    let mut value = TaskAuthority::new(contract(), graph(), observation()).unwrap();
    candidate(&mut value, graph());
    vote(&mut value, 0);
    assert!(
        value
            .apply(observation(), issue(&value, "build", 0, 100))
            .is_err()
    );
    vote(&mut value, 1);
    for action in [
        issue(&value, "build", 1, 100),
        issue(&value, "missing", 0, 100),
        issue(&value, "build", 0, 1),
        issue(&value, "build", 0, contract().limits.deadline_unix_ms + 1),
    ] {
        let before = value.to_json().unwrap();
        assert!(value.apply(observation(), action).is_err());
        assert_eq!(value.to_json().unwrap(), before);
    }
    let first = lease(&mut value, "build", 0);
    let second = lease(&mut value, "build", 0);
    assert!(
        value
            .validate_lease(observation(), &contract().peers[0], &first)
            .is_err()
    );
    assert!(
        value
            .validate_lease(observation(), &contract().peers[1], &second)
            .is_err()
    );
    let mut later = observation();
    later.now_unix_ms = 100;
    assert!(
        value
            .validate_lease(later, &contract().peers[0], &second)
            .is_err()
    );
    assert!(
        value
            .validate_lease(observation(), &contract().peers[0], &second)
            .is_err()
    );
}

#[test]
fn ownership_transfer_needs_new_dual_approval_and_never_revives_old_lease() {
    let mut value = agreed();
    let old = lease(&mut value, "build", 0);
    value
        .apply(observation(), object(&value, "a", &["build"], 0))
        .unwrap();
    value
        .apply(
            observation(),
            AuthorityAction::Protocol {
                action: ReconciliationAction::Disagree {
                    peer: contract().peers[1].clone(),
                    plan: value.reconciliation().revision().reference(),
                    reason: "transfer supervision".into(),
                },
            },
        )
        .unwrap();
    let mut next = graph();
    next.tasks[0].owner = Peer::Fable;
    candidate(&mut value, next);
    vote(&mut value, 0);
    assert!(
        value
            .apply(observation(), issue(&value, "build", 1, 100))
            .is_err()
    );
    assert!(!value.paused_tasks().is_empty());
    vote(&mut value, 1);
    assert!(value.paused_tasks().is_empty());
    assert!(
        value
            .apply(observation(), issue(&value, "build", 0, 100))
            .is_err()
    );
    let fresh = lease(&mut value, "build", 1);
    assert!(fresh.generation() > old.generation());
    assert!(
        value
            .validate_lease(observation(), &contract().peers[0], &old)
            .is_err()
    );
}

#[test]
fn replay_preserves_objections_and_generation_but_never_live_leases() {
    let mut value = agreed();
    let old = lease(&mut value, "independent", 1);
    value
        .apply(observation(), object(&value, "a", &["build"], 0))
        .unwrap();
    let bytes = value.to_json().unwrap();
    let mut restored = TaskAuthority::from_json(&bytes, &contract(), observation()).unwrap();
    assert_eq!(restored.paused_tasks(), value.paused_tasks());
    assert!(
        restored
            .validate_lease(observation(), &contract().peers[1], &old)
            .is_err()
    );
    let fresh = lease(&mut restored, "independent", 1);
    assert!(fresh.generation() > old.generation());
    let mut again =
        TaskAuthority::from_json(&restored.to_json().unwrap(), &contract(), observation()).unwrap();
    assert!(
        again
            .validate_lease(observation(), &contract().peers[1], &fresh)
            .is_err()
    );
    assert!(lease(&mut again, "independent", 1).generation() > fresh.generation());
}

#[test]
fn invalid_objections_are_atomic_and_ids_cannot_resurrect_resolved_pauses() {
    let mut value = agreed();
    for tasks in [vec![], vec!["missing"], vec!["build", "build"]] {
        let before = value.to_json().unwrap();
        let action = object(&value, "a", &tasks, 0);
        assert!(value.apply(observation(), action).is_err());
        assert_eq!(value.to_json().unwrap(), before);
    }
    let original = object(&value, "a", &["build"], 0);
    value.apply(observation(), original.clone()).unwrap();
    let independent = lease(&mut value, "independent", 1);
    value.apply(observation(), original.clone()).unwrap();
    value
        .validate_lease(observation(), &contract().peers[1], &independent)
        .unwrap();
    for action in [
        object(&value, "a", &["right"], 0),
        object(&value, "a", &["build"], 1),
    ] {
        let before = value.to_json().unwrap();
        assert!(value.apply(observation(), action).is_err());
        assert_eq!(value.to_json().unwrap(), before);
    }
    value.apply(observation(), resolve(&value, "a", 0)).unwrap();
    assert!(value.apply(observation(), original).is_err());
    assert!(value.paused_tasks().is_empty());
}

#[test]
fn foreign_peer_and_plan_fields_cannot_obtain_authority_or_object() {
    let mut value = agreed();
    for variant in 0..3 {
        for field in 0..9 {
            let mut action = if variant == 0 {
                object(&value, "a", &["build"], 0)
            } else if variant == 1 {
                issue(&value, "build", 0, 100)
            } else {
                resolve(&value, "missing", 0)
            };
            let (peer, plan) = match &mut action {
                AuthorityAction::Object { peer, plan, .. }
                | AuthorityAction::IssueLease { peer, plan, .. }
                | AuthorityAction::Resolve { peer, plan, .. } => (peer, plan),
                _ => unreachable!(),
            };
            match field {
                0 => peer.session_id.push_str("-other"),
                1 => peer.model.provider.push_str("-other"),
                2 => peer.model.model.push_str("-other"),
                3 => peer.model.effort.push_str("-other"),
                4 => peer.peer = Peer::Fable,
                5 => plan.project_id.push_str("-other"),
                6 => plan.run_id.push_str("-other"),
                7 => plan.revision += 1,
                _ => plan.digest.push_str("-other"),
            }
            let before = value.to_json().unwrap();
            assert!(value.apply(observation(), action).is_err());
            assert_eq!(value.to_json().unwrap(), before);
        }
    }
    for field in 0..2 {
        let mut action = object(&value, "a", &["build"], 0);
        if let AuthorityAction::Object { id, reason, .. } = &mut action {
            if field == 0 {
                *id = " ".into();
            } else {
                *reason = " ".into();
            }
        }
        assert!(value.apply(observation(), action).is_err());
    }
}

#[test]
fn shared_limits_and_outage_permanently_fence_all_leases() {
    for boundary in 0..3 {
        let mut value = agreed();
        let old = lease(&mut value, "build", 0);
        let mut reached = observation();
        let expected = match boundary {
            0 => {
                reached.now_unix_ms = contract().limits.deadline_unix_ms;
                ReconciliationStop::Deadline
            }
            1 => {
                reached.total_tokens = contract().limits.max_total_tokens;
                ReconciliationStop::TokensExhausted
            }
            _ => {
                reached.worker_attempts = contract().limits.max_attempts + 1;
                ReconciliationStop::AttemptsExhausted
            }
        };
        let action = issue(&value, "independent", 1, 100);
        assert!(
            matches!(value.apply(reached, action).unwrap(), AuthorityOutcome::Stopped(reason) if reason == expected)
        );
        assert!(
            value
                .validate_lease(reached, &contract().peers[0], &old)
                .is_err()
        );
        let restored =
            TaskAuthority::from_json(&value.to_json().unwrap(), &contract(), reached).unwrap();
        assert_eq!(
            restored.reconciliation().state(),
            ReconciliationState::Stopped(expected)
        );
    }
    for policy in [
        OutagePolicy::CheckpointAndStop,
        OutagePolicy::CheckpointAffectedAndContinueIndependent,
    ] {
        let mut fixed = contract();
        fixed.outage_policy = policy;
        let mut value = TaskAuthority::new(fixed, graph(), observation()).unwrap();
        candidate(&mut value, graph());
        vote(&mut value, 0);
        vote(&mut value, 1);
        let old = lease(&mut value, "independent", 1);
        let outcome = value
            .apply(
                observation(),
                AuthorityAction::Protocol {
                    action: ReconciliationAction::PeerUnavailable {
                        peer: contract().peers[0].clone(),
                    },
                },
            )
            .unwrap();
        assert!(matches!(
            outcome,
            AuthorityOutcome::Stopped(ReconciliationStop::PeerUnavailable(Peer::Astra))
        ));
        assert!(
            value
                .validate_lease(observation(), &contract().peers[1], &old)
                .is_err()
        );
    }
}

#[test]
fn replay_rejects_malformed_noncanonical_and_reordered_authority_actions() {
    let mut value = agreed();
    lease(&mut value, "independent", 1);
    value
        .apply(observation(), object(&value, "a", &["build"], 0))
        .unwrap();
    let bytes = value.to_json().unwrap();
    let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for mutation in 0..8 {
        let mut snapshot = original.clone();
        match mutation {
            0 => snapshot["schema_version"] = 99.into(),
            1 => snapshot["unrecognized"] = true.into(),
            2 => snapshot["entries"][0]["action"]["unrecognized"] = true.into(),
            3 => {
                snapshot["entries"].as_array_mut().unwrap().remove(1);
            }
            4 => {
                snapshot["entries"].as_array_mut().unwrap().swap(4, 6);
            }
            5 => snapshot["entries"][6]["action"]["peer"]["session_id"] = "foreign".into(),
            6 => {
                snapshot["entries"][6]["observation"]["now_unix_ms"] =
                    contract().limits.deadline_unix_ms.into()
            }
            _ => snapshot["entries"][6]["observation"]["now_unix_ms"] = 0.into(),
        }
        assert!(
            TaskAuthority::from_json(
                &serde_json::to_vec(&snapshot).unwrap(),
                &contract(),
                observation()
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
    let duplicate = String::from_utf8(bytes).unwrap().replacen(
        "\"schema_version\":2",
        "\"schema_version\":2,\"schema_version\":2",
        1,
    );
    assert!(TaskAuthority::from_json(duplicate.as_bytes(), &contract(), observation()).is_err());
    let mut foreign = contract();
    foreign.run_id.push_str("-other");
    assert!(TaskAuthority::from_json(&value.to_json().unwrap(), &foreign, observation()).is_err());
}

#[test]
fn fresh_restore_observation_expires_authority_at_the_exact_deadline() {
    let mut value = agreed();
    let deadline = contract().limits.deadline_unix_ms;
    let action = issue(&value, "build", 0, deadline);
    assert!(matches!(
        value.apply(observation(), action).unwrap(),
        AuthorityOutcome::Lease(_)
    ));
    let mut fresh = observation();
    fresh.now_unix_ms = deadline;
    let restored = TaskAuthority::from_json(&value.to_json().unwrap(), &contract(), fresh).unwrap();
    assert_eq!(
        restored.reconciliation().state(),
        ReconciliationState::Stopped(ReconciliationStop::Deadline)
    );
}
