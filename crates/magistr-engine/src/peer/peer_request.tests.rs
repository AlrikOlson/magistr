use super::*;
use serde_json::{Value, json};

fn observation() -> ReconciliationObservation {
    ReconciliationObservation {
        now_unix_ms: 1,
        total_tokens: 0,
        worker_attempts: 0,
    }
}

fn wire(request: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"schema_version": 1, "request": request})).unwrap()
}

fn send(
    value: &mut TaskAuthority,
    peer: usize,
    request: Value,
) -> Result<AuthorityOutcome, AgreementError> {
    value.receive_peer(
        value.reconciliation().observation(),
        &contract().peers[peer],
        &wire(request),
    )
}

fn reference(value: &TaskAuthority) -> PlanReference {
    value.reconciliation().revision().reference()
}

fn proposals(value: &mut TaskAuthority) {
    let base = reference(value);
    for peer in 0..2 {
        send(
            value,
            peer,
            json!({"kind":"propose", "base":base, "body":body()}),
        )
        .unwrap();
    }
}

fn reviewing() -> TaskAuthority {
    let mut value = TaskAuthority::new(contract(), body(), observation()).unwrap();
    proposals(&mut value);
    let base = reference(&value);
    send(
        &mut value,
        1,
        json!({"kind":"reconcile", "base":base, "body":body()}),
    )
    .unwrap();
    value
}

fn agreed() -> TaskAuthority {
    let mut value = reviewing();
    let plan = reference(&value);
    for peer in 0..2 {
        send(&mut value, peer, json!({"kind":"approve", "plan":plan})).unwrap();
    }
    value
}

fn issue(value: &TaskAuthority, task: &str) -> Value {
    json!({"kind":"issue_lease", "plan":reference(value), "task":task, "expires_unix_ms":100})
}

fn lease(value: &mut TaskAuthority, task: &str, peer: usize) -> InstructionLease {
    let request = issue(value, task);
    match send(value, peer, request).unwrap() {
        AuthorityOutcome::Lease(lease) => *lease,
        other => panic!("expected lease: {other:?}"),
    }
}

fn admit(lease: &InstructionLease, id: &str) -> Value {
    json!({"kind":"admit_worker", "plan":lease.plan, "task":lease.task,
        "lease_generation":lease.generation, "id":id, "worker":body().worker_models[0]})
}

#[test]
fn wire_proposals_are_independent_and_votes_bind_to_the_current_revision() {
    let mut value = TaskAuthority::new(contract(), body(), observation()).unwrap();
    let base = reference(&value);
    let proposal = json!({"kind":"propose", "base":base, "body":body()});
    send(&mut value, 0, proposal.clone()).unwrap();
    send(&mut value, 0, proposal.clone()).unwrap();
    assert!(value.reconciliation().proposals().is_none());
    assert!(
        send(
            &mut value,
            1,
            json!({"kind":"reconcile", "base":base, "body":body()})
        )
        .is_err()
    );
    let mut alternative = body();
    alternative.tasks[0].risks.push("second opinion".into());
    assert!(
        send(
            &mut value,
            0,
            json!({"kind":"propose", "base":base, "body":alternative})
        )
        .is_err()
    );
    send(
        &mut value,
        1,
        json!({"kind":"propose", "base":base, "body":alternative}),
    )
    .unwrap();
    assert_ne!(
        value.reconciliation().proposals().unwrap()[0],
        value.reconciliation().proposals().unwrap()[1]
    );
    send(
        &mut value,
        1,
        json!({"kind":"reconcile", "base":base, "body":body()}),
    )
    .unwrap();
    let plan = reference(&value);
    assert_ne!(base, plan);
    assert!(send(&mut value, 0, json!({"kind":"approve", "plan":base})).is_err());
    for _ in 0..2 {
        send(&mut value, 0, json!({"kind":"approve", "plan":plan})).unwrap();
        assert_eq!(
            value.reconciliation().state(),
            ReconciliationState::Reviewing
        );
    }
    let mut foreign = plan.clone();
    foreign.run_id = "other".into();
    assert!(send(&mut value, 1, json!({"kind":"approve", "plan":foreign})).is_err());
    send(&mut value, 1, json!({"kind":"approve", "plan":plan})).unwrap();
    assert_eq!(value.reconciliation().state(), ReconciliationState::Agreed);
    let restored =
        TaskAuthority::from_json(&value.to_json().unwrap(), &contract(), observation()).unwrap();
    assert_eq!(
        restored.reconciliation().state(),
        ReconciliationState::Agreed
    );
    assert_eq!(reference(&restored), plan);
}

#[test]
fn wire_cannot_claim_identity_or_host_only_actions() {
    let mut value = reviewing();
    let plan = reference(&value);
    let mut requests = vec![
        json!({"kind":"approve", "plan":plan, "peer":contract().peers[1]}),
        json!({"kind":"approve", "plan":plan, "session_id":"fable-session"}),
        json!({"kind":"approve", "plan":plan, "observation":observation()}),
        json!({"kind":"observe"}),
        json!({"kind":"peer_unavailable", "peer":contract().peers[1]}),
        json!({"kind":"protocol", "action":{"kind":"approve", "peer":contract().peers[1], "plan":plan}}),
        json!({"kind":"accept_worker", "id":"job", "receipt":"claimed success"}),
        json!({"kind":"retire_worker", "id":"job", "receipt":"claimed termination"}),
    ];
    let mut nested = json!({"kind":"approve", "plan":plan});
    nested["plan"]["extra"] = true.into();
    requests.push(nested);
    for request in requests {
        assert!(send(&mut value, 0, request).is_err());
        assert_eq!(
            value.reconciliation().state(),
            ReconciliationState::Reviewing
        );
        assert!(value.unsettled_workers().next().is_none());
        assert!(value.worker_receipt("job").is_none());
    }
    send(&mut value, 0, json!({"kind":"approve", "plan":plan})).unwrap();
    assert_eq!(
        value.reconciliation().state(),
        ReconciliationState::Reviewing
    );
}

#[test]
fn every_fixed_identity_field_is_checked_even_for_candidate_reconciliation() {
    let mut value = TaskAuthority::new(contract(), body(), observation()).unwrap();
    proposals(&mut value);
    let bytes = wire(json!({"kind":"reconcile", "base":reference(&value), "body":body()}));
    for index in 0..5 {
        let mut stranger = contract().peers[0].clone();
        match index {
            0 => stranger.peer = Peer::Fable,
            1 => stranger.session_id = "another-session".into(),
            2 => stranger.model.provider = "other-provider".into(),
            3 => stranger.model.model = "other-model".into(),
            _ => stranger.model.effort = "other-effort".into(),
        }
        assert!(matches!(
            value.receive_peer(observation(), &stranger, &bytes),
            Err(AgreementError::UnknownPeer)
        ));
        assert_eq!(
            value.reconciliation().state(),
            ReconciliationState::Collecting
        );
    }
    value
        .receive_peer(observation(), &contract().peers[0], &bytes)
        .unwrap();
    assert_eq!(
        value.reconciliation().state(),
        ReconciliationState::Reviewing
    );
}

#[test]
fn wire_requires_one_strict_versioned_bounded_json_message() {
    let mut value = reviewing();
    let valid =
        String::from_utf8(wire(json!({"kind":"approve", "plan":reference(&value)}))).unwrap();
    let invalid = vec![
        valid.replace("\"schema_version\":1", "\"schema_version\":2"),
        valid.replace(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
        ),
        valid.replace(
            "\"kind\":\"approve\"",
            "\"kind\":\"approve\",\"kind\":\"approve\"",
        ),
        valid.replace(
            "\"schema_version\":1",
            "\"schema_version\":1,\"peer\":\"fable\"",
        ),
        serde_json::to_string(&json!({"request":{"kind":"approve","plan":reference(&value)}}))
            .unwrap(),
        format!("{valid}{valid}"),
        format!("```json\n{valid}\n```"),
        format!("{valid}{}", " ".repeat(65_537 - valid.len())),
        valid.replace("\"revision\":2", "\"revision\":2,\"revision\":2"),
    ];
    for bytes in invalid {
        assert!(
            value
                .receive_peer(observation(), &contract().peers[0], bytes.as_bytes())
                .is_err()
        );
    }
    let padded = format!("{valid}{}", " ".repeat(65_536 - valid.len()));
    value
        .receive_peer(observation(), &contract().peers[0], padded.as_bytes())
        .unwrap();
    assert_eq!(
        value.reconciliation().state(),
        ReconciliationState::Reviewing
    );
}

#[test]
fn rejected_messages_still_charge_usage_expire_leases_and_replay_observations() {
    let mut value = agreed();
    let old = lease(&mut value, "build", 0);
    let mut stale = reference(&value);
    stale.revision -= 1;
    let cases = [
        (contract().peers[0].clone(), b"not JSON".to_vec()),
        (contract().peers[0].clone(), vec![b' '; 65_537]),
        (
            contract().peers[0].clone(),
            wire(json!({"kind":"approve", "plan":stale})),
        ),
        (
            {
                let mut p = contract().peers[0].clone();
                p.session_id = "unknown".into();
                p
            },
            wire(json!({"kind":"approve", "plan":reference(&value)})),
        ),
    ];
    for (i, (peer, bytes)) in cases.into_iter().enumerate() {
        let observed = ReconciliationObservation {
            now_unix_ms: 100 + i as u64,
            total_tokens: 10 + i as u64,
            worker_attempts: 0,
        };
        assert!(value.receive_peer(observed, &peer, &bytes).is_err());
        assert_eq!(value.reconciliation().observation(), observed);
        assert!(
            value
                .validate_lease(observed, &contract().peers[0], &old)
                .is_err()
        );
        let restored =
            TaskAuthority::from_json(&value.to_json().unwrap(), &contract(), observed).unwrap();
        assert_eq!(restored.reconciliation().observation(), observed);
        assert_eq!(
            restored.reconciliation().state(),
            ReconciliationState::Agreed
        );
    }
    let bytes = value.to_json().unwrap();
    assert!(
        value
            .receive_peer(observation(), &contract().peers[0], b"invalid")
            .is_err()
    );
    assert_eq!(
        value.to_json().unwrap(),
        bytes,
        "decreasing host readings change nothing"
    );
}

#[test]
fn wire_admission_reuses_owner_dependency_model_and_duplicate_guards() {
    let mut value = reviewing();
    let request = issue(&value, "build");
    assert!(send(&mut value, 0, request).is_err());
    let mut value = agreed();
    let request = issue(&value, "build");
    assert!(send(&mut value, 1, request).is_err());
    let dependent = lease(&mut value, "review", 1);
    assert!(send(&mut value, 1, admit(&dependent, "dependent")).is_err());
    let owner = lease(&mut value, "build", 0);
    assert!(send(&mut value, 1, admit(&owner, "wrong-owner")).is_err());
    let mut bad_model = admit(&owner, "bad-model");
    bad_model["worker"]["model"] = "unapproved".into();
    assert!(send(&mut value, 0, bad_model).is_err());
    let request = admit(&owner, "job");
    assert!(matches!(
        send(&mut value, 0, request.clone()).unwrap(),
        AuthorityOutcome::Worker(_)
    ));
    assert!(matches!(
        send(&mut value, 0, request).unwrap(),
        AuthorityOutcome::AlreadyAdmitted
    ));
    assert_eq!(value.unsettled_workers().count(), 1);
    assert!(
        send(
            &mut value,
            0,
            json!({"kind":"accept_worker", "id":"job", "receipt":"model claim"})
        )
        .is_err()
    );
    assert!(value.worker_receipt("job").is_none());
    assert!(send(&mut value, 1, admit(&dependent, "dependent")).is_err());
}

#[test]
fn wire_objections_revoke_fences_and_only_the_author_can_resolve() {
    let mut value = agreed();
    let owner = lease(&mut value, "build", 0);
    let plan = reference(&value);
    send(&mut value, 1, json!({"kind":"object", "plan":plan, "id":"objection", "tasks":["build"], "reason":"missing evidence"})).unwrap();
    assert!(value.paused_tasks().contains("review"));
    assert!(send(&mut value, 0, admit(&owner, "stale")).is_err());
    assert!(
        send(
            &mut value,
            0,
            json!({"kind":"resolve", "plan":plan, "id":"objection"})
        )
        .is_err()
    );
    send(
        &mut value,
        1,
        json!({"kind":"resolve", "plan":plan, "id":"objection"}),
    )
    .unwrap();
    assert!(value.paused_tasks().is_empty());
    assert!(send(&mut value, 0, admit(&owner, "still-stale")).is_err());
    let fresh = lease(&mut value, "build", 0);
    assert!(matches!(
        send(&mut value, 0, admit(&fresh, "fresh")).unwrap(),
        AuthorityOutcome::Worker(_)
    ));
}

#[test]
fn wire_disagreement_invalidates_votes_and_stops_at_the_round_allowance() {
    let mut value = agreed();
    let old = lease(&mut value, "build", 0);
    let plan = reference(&value);
    send(
        &mut value,
        1,
        json!({"kind":"disagree", "plan":plan, "reason":"revise scope"}),
    )
    .unwrap();
    assert_eq!(value.reconciliation().round(), 2);
    assert!(send(&mut value, 0, admit(&old, "stale")).is_err());
    assert!(send(&mut value, 1, json!({"kind":"approve", "plan":plan})).is_err());
    proposals(&mut value);
    let base = reference(&value);
    send(
        &mut value,
        0,
        json!({"kind":"reconcile", "base":base, "body":body()}),
    )
    .unwrap();
    let plan = reference(&value);
    assert!(matches!(
        send(
            &mut value,
            0,
            json!({"kind":"disagree", "plan":plan, "reason":"still disagree"})
        )
        .unwrap(),
        AuthorityOutcome::Stopped(ReconciliationStop::RoundsExhausted)
    ));
}

#[test]
fn typed_messages_roundtrip_through_the_receiver_and_bound_output_size() {
    let mut value = reviewing();
    let request = PeerRequest::Approve {
        plan: reference(&value),
    };
    let bytes = request.to_json().unwrap();
    for peer in &contract().peers {
        value.receive_peer(observation(), peer, &bytes).unwrap();
    }
    assert_eq!(value.reconciliation().state(), ReconciliationState::Agreed);
    let oversized = PeerRequest::Disagree {
        plan: reference(&value),
        reason: "x".repeat(PeerRequest::MAX_JSON_BYTES),
    };
    assert!(oversized.to_json().is_err());
}

#[test]
fn wire_cannot_settle_workers_and_restoration_never_reissues_a_permit() {
    let mut value = agreed();
    let owner = lease(&mut value, "build", 0);
    let request = admit(&owner, "job");
    send(&mut value, 0, request.clone()).unwrap();
    for kind in ["accept_worker", "retire_worker"] {
        assert!(
            send(
                &mut value,
                0,
                json!({"kind":kind, "id":"job", "receipt":"model claim"})
            )
            .is_err()
        );
        assert_eq!(value.unsettled_workers().count(), 1);
        assert!(value.worker_receipt("job").is_none());
    }
    let mut restored =
        TaskAuthority::from_json(&value.to_json().unwrap(), &contract(), observation()).unwrap();
    assert!(
        send(&mut restored, 0, request).is_err(),
        "replay discards old leases"
    );
    assert_eq!(restored.unsettled_workers().count(), 1);
    value
        .apply(
            observation(),
            AuthorityAction::AcceptWorker {
                peer: contract().peers[0].clone(),
                id: "job".into(),
                receipt: "host verified artifacts, gates and termination".into(),
            },
        )
        .unwrap();
    assert_eq!(value.unsettled_workers().count(), 0);
    let dependent = lease(&mut value, "review", 1);
    assert!(matches!(
        send(&mut value, 1, admit(&dependent, "dependent")).unwrap(),
        AuthorityOutcome::Worker(_)
    ));
}

#[test]
fn reached_limits_and_host_outage_stop_even_malformed_peer_messages() {
    for (observed, expected) in [
        (
            ReconciliationObservation {
                now_unix_ms: contract().limits.deadline_unix_ms,
                ..observation()
            },
            ReconciliationStop::Deadline,
        ),
        (
            ReconciliationObservation {
                total_tokens: contract().limits.max_total_tokens,
                ..observation()
            },
            ReconciliationStop::TokensExhausted,
        ),
        (
            ReconciliationObservation {
                worker_attempts: contract().limits.max_attempts + 1,
                ..observation()
            },
            ReconciliationStop::AttemptsExhausted,
        ),
    ] {
        let mut value = agreed();
        assert!(
            matches!(value.receive_peer(observed, &contract().peers[0], b"malformed").unwrap(),
            AuthorityOutcome::Stopped(reason) if reason == expected)
        );
        let restored =
            TaskAuthority::from_json(&value.to_json().unwrap(), &contract(), observed).unwrap();
        assert_eq!(
            restored.reconciliation().state(),
            ReconciliationState::Stopped(expected)
        );
    }
    let mut value = agreed();
    value
        .apply(
            observation(),
            AuthorityAction::Protocol {
                action: ReconciliationAction::PeerUnavailable {
                    peer: contract().peers[1].clone(),
                },
            },
        )
        .unwrap();
    assert!(matches!(
        value
            .receive_peer(observation(), &contract().peers[0], b"malformed")
            .unwrap(),
        AuthorityOutcome::Stopped(ReconciliationStop::PeerUnavailable(Peer::Fable))
    ));
}
