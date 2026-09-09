use super::*;
use serde_json::{Value, json};

fn checkpoint(value: &TaskAuthority) -> Value {
    serde_json::from_slice(&value.to_json().unwrap()).unwrap()
}

fn restore(value: &Value) -> Result<TaskAuthority, AgreementError> {
    TaskAuthority::from_json(
        &serde_json::to_vec(value).unwrap(),
        &contract(),
        observation(),
    )
}

fn assert_stopped(outcome: AuthorityOutcome, expected: ReconciliationStop) {
    assert!(matches!(outcome, AuthorityOutcome::Stopped(reason) if reason == expected));
}

#[test]
fn replay_preserves_partial_delivery_and_requires_the_missing_acknowledgement() {
    let mut value = authority();
    deliver(&mut value, 0);
    acknowledge(&mut value, 0).unwrap();
    deliver(&mut value, 1);
    let mut restored = restore(&checkpoint(&value)).unwrap();
    assert_eq!(
        restored.briefing_for(&contract().peers[1]).unwrap(),
        value.briefing_for(&contract().peers[0]).unwrap()
    );
    assert!(propose(&mut restored, 0).is_err());
    assert!(propose(&mut restored, 1).is_err());
    acknowledge(&mut restored, 1).unwrap();
    propose(&mut restored, 0).unwrap();
    propose(&mut restored, 1).unwrap();
    assert!(restored.reconciliation().proposals().is_some());
    assert_eq!(
        restored.reconciliation().state(),
        ReconciliationState::Collecting
    );
}

#[test]
fn replay_rejects_acknowledgement_before_delivery_and_proposal_before_receipts() {
    let mut value = authority();
    ready(&mut value);
    propose(&mut value, 0).unwrap();
    let valid = checkpoint(&value);

    let mut reordered = valid.clone();
    let entries = reordered["entries"].as_array_mut().unwrap();
    let delivery = entries
        .iter()
        .position(|entry| entry["action"]["kind"] == "deliver_briefing")
        .unwrap();
    let acknowledgement = entries
        .iter()
        .position(|entry| entry["action"]["kind"] == "acknowledge_briefing")
        .unwrap();
    entries.swap(delivery, acknowledgement);
    assert!(restore(&reordered).is_err());

    let mut missing = valid;
    missing["entries"].as_array_mut().unwrap().retain(|entry| {
        entry["action"]["kind"] != "acknowledge_briefing"
            || entry["action"]["peer"]["peer"] != "fable"
    });
    assert!(restore(&missing).is_err());
}

#[test]
fn version_three_requires_the_exact_seed_briefing_and_rejects_legacy_injection() {
    let value = authority();
    let valid = checkpoint(&value);
    assert_eq!(valid["schema_version"], 3);
    for replacement in [None, Some(Value::Null), Some(json!([]))] {
        let mut changed = valid.clone();
        if let Some(replacement) = replacement {
            changed["briefing"] = replacement;
        } else {
            changed.as_object_mut().unwrap().remove("briefing");
        }
        assert!(restore(&changed).is_err());
    }

    let different_evidence = evidence();
    let mut different_plan = body();
    different_plan.context = different_evidence.context(different_plan.context.constraints);
    different_plan
        .scope
        .push("another jointly approved scope".into());
    let different = PeerBriefing::new(contract(), different_plan, different_evidence).unwrap();
    let mut changed = valid.clone();
    changed["briefing"] = json!(different.to_json().unwrap());
    assert!(matches!(
        restore(&changed),
        Err(AgreementError::DigestMismatch)
    ));

    let mut packet: Value = serde_json::from_slice(
        &serde_json::from_value::<Vec<u8>>(valid["briefing"].clone()).unwrap(),
    )
    .unwrap();
    packet["evidence"]["code"] = json!("substituted code evidence");
    changed["briefing"] = json!(serde_json::to_vec(&packet).unwrap());
    assert!(restore(&changed).is_err());

    for version in [1, 2] {
        let mut legacy = valid.clone();
        legacy["schema_version"] = json!(version);
        assert!(restore(&legacy).is_err());
    }
}

#[test]
fn legacy_journals_remain_ungated_and_keep_their_original_attempt_stop_policy() {
    let legacy = TaskAuthority::new(contract(), body(), observation()).unwrap();
    let original = checkpoint(&legacy);
    assert!(original.get("briefing").is_none());
    for version in [1, 2] {
        let mut journal = original.clone();
        journal["schema_version"] = json!(version);
        let mut restored = restore(&journal).unwrap();
        assert!(restored.briefing_for(&contract().peers[0]).is_err());
        propose(&mut restored, 0).unwrap();
        assert_eq!(checkpoint(&restored)["schema_version"], version);
        let observed = ReconciliationObservation {
            worker_attempts: contract().limits.max_attempts,
            ..observation()
        };
        let outcome = restored
            .apply(
                observed,
                AuthorityAction::Protocol {
                    action: ReconciliationAction::Observe,
                },
            )
            .unwrap();
        if version == 1 {
            assert_stopped(outcome, ReconciliationStop::AttemptsExhausted);
        } else {
            assert!(matches!(outcome, AuthorityOutcome::Updated));
        }
        let replayed =
            TaskAuthority::from_json(&restored.to_json().unwrap(), &contract(), observed).unwrap();
        assert_eq!(
            replayed.reconciliation().state(),
            restored.reconciliation().state()
        );
        assert_eq!(checkpoint(&replayed)["schema_version"], version);
    }
}

#[test]
fn rejected_acknowledgements_charge_usage_without_opening_the_proposal_gate() {
    for rejection in 0..3 {
        let mut value = authority();
        deliver(&mut value, 0);
        acknowledge(&mut value, 0).unwrap();
        deliver(&mut value, 1);
        let mut peer = contract().peers[1].clone();
        let mut reference = briefing().revision().reference();
        if rejection == 0 {
            peer.session_id.push_str("-unknown");
        } else if rejection == 1 {
            reference.revision += 1;
        }
        let bytes = if rejection == 2 {
            b"{malformed acknowledgement}".to_vec()
        } else {
            PeerRequest::AcknowledgeBriefing {
                briefing: reference,
            }
            .to_json()
            .unwrap()
        };
        let observed = ReconciliationObservation {
            now_unix_ms: 2,
            total_tokens: 31,
            ..observation()
        };
        assert!(value.receive_peer(observed, &peer, &bytes).is_err());
        assert_eq!(value.reconciliation().observation(), observed);
        let mut restored =
            TaskAuthority::from_json(&value.to_json().unwrap(), &contract(), observed).unwrap();
        let proposal = PeerRequest::Propose {
            base: restored.reconciliation().revision().reference(),
            body: Box::new(restored.reconciliation().revision().body().clone()),
        }
        .to_json()
        .unwrap();
        assert!(
            restored
                .receive_peer(observed, &contract().peers[0], &proposal)
                .is_err()
        );
        let acknowledgement = PeerRequest::AcknowledgeBriefing {
            briefing: briefing().revision().reference(),
        }
        .to_json()
        .unwrap();
        restored
            .receive_peer(observed, &contract().peers[1], &acknowledgement)
            .unwrap();
        restored
            .receive_peer(observed, &contract().peers[0], &proposal)
            .unwrap();
        assert!(restored.reconciliation().proposals().is_none());
    }
}

#[test]
fn peer_silence_stops_at_deadline_or_token_limit_without_an_acknowledgement() {
    for reason in [
        ReconciliationStop::Deadline,
        ReconciliationStop::TokensExhausted,
    ] {
        let mut value = authority();
        deliver(&mut value, 0);
        acknowledge(&mut value, 0).unwrap();
        deliver(&mut value, 1);
        let observed = match reason {
            ReconciliationStop::Deadline => ReconciliationObservation {
                now_unix_ms: contract().limits.deadline_unix_ms,
                ..observation()
            },
            _ => ReconciliationObservation {
                total_tokens: contract().limits.max_total_tokens,
                ..observation()
            },
        };
        assert_stopped(
            value
                .apply(
                    observed,
                    AuthorityAction::Protocol {
                        action: ReconciliationAction::Observe,
                    },
                )
                .unwrap(),
            reason,
        );
        assert_stopped(
            value
                .receive_peer(observed, &contract().peers[1], b"malformed")
                .unwrap(),
            reason,
        );
        let restored =
            TaskAuthority::from_json(&value.to_json().unwrap(), &contract(), observed).unwrap();
        assert_eq!(
            restored.reconciliation().state(),
            ReconciliationState::Stopped(reason)
        );
        assert!(restored.reconciliation().proposals().is_none());
    }
}

#[test]
fn outage_during_shared_delivery_never_supplies_a_missing_peer_acknowledgement() {
    for policy in [
        OutagePolicy::CheckpointAndStop,
        OutagePolicy::CheckpointAffectedAndContinueIndependent,
    ] {
        let mut fixed = contract();
        fixed.outage_policy = policy;
        let evidence = evidence();
        let mut plan = body();
        plan.context = evidence.context(plan.context.constraints);
        let packet = PeerBriefing::new(fixed.clone(), plan, evidence).unwrap();
        let reference = packet.revision().reference();
        let mut value = TaskAuthority::with_briefing(packet, observation()).unwrap();
        value
            .apply(
                observation(),
                AuthorityAction::DeliverBriefing {
                    peer: fixed.peers[0].clone(),
                    briefing: reference.clone(),
                },
            )
            .unwrap();
        let acknowledgement = PeerRequest::AcknowledgeBriefing {
            briefing: reference,
        }
        .to_json()
        .unwrap();
        value
            .receive_peer(observation(), &fixed.peers[0], &acknowledgement)
            .unwrap();
        let reason = ReconciliationStop::PeerUnavailable(Peer::Fable);
        assert_stopped(
            value
                .apply(
                    observation(),
                    AuthorityAction::Protocol {
                        action: ReconciliationAction::PeerUnavailable {
                            peer: fixed.peers[1].clone(),
                        },
                    },
                )
                .unwrap(),
            reason,
        );
        assert_stopped(
            value
                .receive_peer(observation(), &fixed.peers[1], &acknowledgement)
                .unwrap(),
            reason,
        );
        let restored =
            TaskAuthority::from_json(&value.to_json().unwrap(), &fixed, observation()).unwrap();
        assert_eq!(
            restored.reconciliation().state(),
            ReconciliationState::Stopped(reason)
        );
        assert!(restored.reconciliation().proposals().is_none());
    }
}

#[test]
fn fixed_briefing_receipts_survive_new_revisions_but_never_replace_fresh_dual_votes() {
    let mut value = authority();
    ready(&mut value);
    let bytes = value.briefing_for(&contract().peers[0]).unwrap().to_vec();
    let mut previous_candidate = None;
    for round in 1..=2 {
        propose(&mut value, 0).unwrap();
        propose(&mut value, 1).unwrap();
        let base = value.reconciliation().revision().reference();
        let body = value.reconciliation().revision().body().clone();
        send(
            &mut value,
            0,
            json!({"kind":"reconcile", "base":base, "body":body}),
        )
        .unwrap();
        let candidate = value.reconciliation().revision().reference();
        if let Some(stale) = previous_candidate.take() {
            assert!(send(&mut value, 0, json!({"kind":"approve", "plan":stale})).is_err());
        }
        send(&mut value, 0, json!({"kind":"approve", "plan":candidate})).unwrap();
        assert_eq!(
            value.reconciliation().state(),
            ReconciliationState::Reviewing
        );
        send(&mut value, 1, json!({"kind":"approve", "plan":candidate})).unwrap();
        assert_eq!(value.reconciliation().state(), ReconciliationState::Agreed);
        assert_eq!(value.briefing_for(&contract().peers[1]).unwrap(), bytes);
        value = restore(&checkpoint(&value)).unwrap();
        assert_eq!(value.reconciliation().state(), ReconciliationState::Agreed);
        if round == 1 {
            send(
                &mut value,
                1,
                json!({"kind":"disagree", "plan":candidate, "reason":"revise ownership"}),
            )
            .unwrap();
            previous_candidate = Some(candidate);
        }
    }
}
