use super::*;

fn observation() -> ReconciliationObservation {
    ReconciliationObservation {
        now_unix_ms: 1,
        total_tokens: 0,
        worker_attempts: 0,
    }
}

fn reconcile() -> Reconciliation {
    Reconciliation::new(contract(), body(), observation()).unwrap()
}

fn proposal(value: &Reconciliation, index: usize) -> ReconciliationAction {
    ReconciliationAction::Propose {
        peer: contract().peers[index].clone(),
        base: value.revision().reference(),
        body: Box::new(body()),
    }
}

fn paired(value: &mut Reconciliation) {
    for index in 0..2 {
        value.apply(observation(), proposal(value, index)).unwrap();
    }
}

fn candidate(value: &mut Reconciliation) {
    paired(value);
    let action = ReconciliationAction::Reconcile {
        base: value.revision().reference(),
        body: Box::new(body()),
    };
    value.apply(observation(), action).unwrap();
}

fn vote(value: &Reconciliation, index: usize) -> ReconciliationAction {
    ReconciliationAction::Approve {
        peer: contract().peers[index].clone(),
        plan: value.revision().reference(),
    }
}

fn disagree(value: &Reconciliation, index: usize) -> ReconciliationAction {
    ReconciliationAction::Disagree {
        peer: contract().peers[index].clone(),
        plan: value.revision().reference(),
        reason: "acceptance needs more evidence".into(),
    }
}

#[test]
fn proposals_are_independent_and_never_implicit_votes() {
    let mut value = reconcile();
    let first = proposal(&value, 0);
    value.apply(observation(), first.clone()).unwrap();
    value.apply(observation(), first).unwrap();
    assert!(value.proposals().is_none());
    assert!(
        value
            .apply(
                observation(),
                ReconciliationAction::Reconcile {
                    base: value.revision().reference(),
                    body: Box::new(body()),
                }
            )
            .is_err()
    );
    value.apply(observation(), proposal(&value, 1)).unwrap();
    assert!(value.proposals().is_some());
    assert_eq!(value.state(), ReconciliationState::Collecting);
    value
        .apply(
            observation(),
            ReconciliationAction::Reconcile {
                base: value.revision().reference(),
                body: Box::new(body()),
            },
        )
        .unwrap();
    assert_eq!(value.state(), ReconciliationState::Reviewing);
    value.apply(observation(), vote(&value, 0)).unwrap();
    value.apply(observation(), vote(&value, 0)).unwrap();
    assert_eq!(value.state(), ReconciliationState::Reviewing);
    value.apply(observation(), vote(&value, 1)).unwrap();
    assert_eq!(value.state(), ReconciliationState::Agreed);
}

#[test]
fn either_peer_disagrees_and_old_votes_cannot_cross_rounds() {
    for index in 0..2 {
        let mut value = reconcile();
        candidate(&mut value);
        value.apply(observation(), vote(&value, 1 - index)).unwrap();
        let stale = vote(&value, index);
        value.apply(observation(), disagree(&value, index)).unwrap();
        assert_eq!(value.round(), 2);
        assert_eq!(value.state(), ReconciliationState::Collecting);
        assert!(value.proposals().is_none());
        candidate(&mut value);
        assert!(value.apply(observation(), stale).is_err());
        value.apply(observation(), vote(&value, index)).unwrap();
        assert_eq!(value.state(), ReconciliationState::Reviewing);
        value
            .apply(observation(), disagree(&value, 1 - index))
            .unwrap();
        assert_eq!(
            value.state(),
            ReconciliationState::Stopped(ReconciliationStop::RoundsExhausted)
        );
        assert!(value.apply(observation(), vote(&value, 0)).is_err());
    }
}

#[test]
fn invalid_proposals_leave_the_journal_unchanged() {
    for case in 0..8 {
        let mut value = reconcile();
        let before = value.to_json().unwrap();
        let mut action = proposal(&value, 0);
        if let ReconciliationAction::Propose { peer, base, body } = &mut action {
            match case {
                0 => peer.session_id = "replacement".into(),
                1 => base.run_id = "another-run".into(),
                2 => base.digest = "different".into(),
                3 => body.context.code_revision = "changed".into(),
                4 => body.context.roadmap_revision = "changed".into(),
                5 => body.context.routing_history_revision = "changed".into(),
                6 => body.context.constraints.push("changed".into()),
                _ => body.tasks[0].dependencies.push("missing".into()),
            }
        }
        assert!(value.apply(observation(), action).is_err(), "case {case}");
        assert_eq!(value.to_json().unwrap(), before);
    }
}

#[test]
fn contradictory_proposal_is_rejected_and_candidate_needs_fresh_votes() {
    let mut value = reconcile();
    value.apply(observation(), proposal(&value, 0)).unwrap();
    let mut changed = proposal(&value, 0);
    if let ReconciliationAction::Propose { body, .. } = &mut changed {
        body.tasks[0].owner = Peer::Fable;
    }
    assert!(value.apply(observation(), changed).is_err());
    value.apply(observation(), proposal(&value, 1)).unwrap();
    let premature = vote(&value, 0);
    assert!(value.apply(observation(), premature.clone()).is_err());
    let mut changed = body();
    changed.tasks[0].owner = Peer::Fable;
    value
        .apply(
            observation(),
            ReconciliationAction::Reconcile {
                base: value.revision().reference(),
                body: Box::new(changed),
            },
        )
        .unwrap();
    assert!(value.apply(observation(), premature).is_err());
    value.apply(observation(), vote(&value, 0)).unwrap();
    assert_eq!(value.state(), ReconciliationState::Reviewing);
}

#[test]
fn deadline_and_shared_evidence_limits_stop_before_second_vote() {
    for case in 0..3 {
        let mut value = reconcile();
        candidate(&mut value);
        value.apply(observation(), vote(&value, 0)).unwrap();
        let mut observed = observation();
        let stop = match case {
            0 => {
                observed.now_unix_ms = contract().limits.deadline_unix_ms;
                ReconciliationStop::Deadline
            }
            1 => {
                observed.total_tokens = contract().limits.max_total_tokens;
                ReconciliationStop::TokensExhausted
            }
            _ => {
                observed.worker_attempts = contract().limits.max_attempts + 1;
                ReconciliationStop::AttemptsExhausted
            }
        };
        value.apply(observed, vote(&value, 1)).unwrap();
        assert_eq!(value.state(), ReconciliationState::Stopped(stop));
        assert!(value.apply(observed, vote(&value, 1)).is_err());
    }
}

#[test]
fn observations_are_cumulative_and_survive_disagreement_and_replay() {
    let mut value = reconcile();
    candidate(&mut value);
    let observed = ReconciliationObservation {
        now_unix_ms: 10,
        total_tokens: 500,
        worker_attempts: 1,
    };
    value
        .apply(observed, ReconciliationAction::Observe)
        .unwrap();
    value.apply(observed, disagree(&value, 0)).unwrap();
    for earlier in [
        ReconciliationObservation {
            now_unix_ms: 9,
            ..observed
        },
        ReconciliationObservation {
            total_tokens: 499,
            ..observed
        },
        ReconciliationObservation {
            worker_attempts: 0,
            ..observed
        },
    ] {
        assert!(value.apply(earlier, ReconciliationAction::Observe).is_err());
    }
    let restored =
        Reconciliation::from_json(&value.to_json().unwrap(), &contract(), observed).unwrap();
    assert_eq!(restored.observation(), observed);
    assert_eq!(restored.round(), 2);
    assert_eq!(restored.state(), ReconciliationState::Collecting);
}

#[test]
fn outage_never_supplies_a_vote_under_either_policy() {
    for policy in [
        OutagePolicy::CheckpointAndStop,
        OutagePolicy::CheckpointAffectedAndContinueIndependent,
    ] {
        let mut fixed = contract();
        fixed.outage_policy = policy;
        let mut value = Reconciliation::new(fixed.clone(), body(), observation()).unwrap();
        candidate(&mut value);
        value.apply(observation(), vote(&value, 0)).unwrap();
        value
            .apply(
                observation(),
                ReconciliationAction::PeerUnavailable {
                    peer: fixed.peers[1].clone(),
                },
            )
            .unwrap();
        assert_eq!(
            value.state(),
            ReconciliationState::Stopped(ReconciliationStop::PeerUnavailable(Peer::Fable))
        );
        let restored =
            Reconciliation::from_json(&value.to_json().unwrap(), &fixed, observation()).unwrap();
        assert_eq!(restored.state(), value.state());
    }
}

#[test]
fn replay_restores_votes_but_rechecks_current_time() {
    let mut value = reconcile();
    candidate(&mut value);
    value.apply(observation(), vote(&value, 0)).unwrap();
    let bytes = value.to_json().unwrap();
    let mut restored = Reconciliation::from_json(&bytes, &contract(), observation()).unwrap();
    restored.apply(observation(), vote(&restored, 1)).unwrap();
    assert_eq!(restored.state(), ReconciliationState::Agreed);
    let expired = ReconciliationObservation {
        now_unix_ms: contract().limits.deadline_unix_ms,
        ..observation()
    };
    let restored =
        Reconciliation::from_json(&restored.to_json().unwrap(), &contract(), expired).unwrap();
    assert_eq!(
        restored.state(),
        ReconciliationState::Stopped(ReconciliationStop::Deadline)
    );
}

#[test]
fn one_round_and_changed_mind_after_agreement_preserve_equal_authority() {
    for index in 0..2 {
        let mut fixed = contract();
        fixed.limits.max_reconciliation_rounds = 1;
        let mut value = Reconciliation::new(fixed, body(), observation()).unwrap();
        candidate(&mut value);
        value.apply(observation(), vote(&value, 0)).unwrap();
        value.apply(observation(), vote(&value, 1)).unwrap();
        value.apply(observation(), disagree(&value, index)).unwrap();
        assert_eq!(value.round(), 1);
        assert_eq!(
            value.state(),
            ReconciliationState::Stopped(ReconciliationStop::RoundsExhausted)
        );
    }
    let mut value = reconcile();
    candidate(&mut value);
    value.apply(observation(), vote(&value, 0)).unwrap();
    value.apply(observation(), vote(&value, 1)).unwrap();
    value.apply(observation(), disagree(&value, 0)).unwrap();
    candidate(&mut value);
    value.apply(observation(), vote(&value, 1)).unwrap();
    assert_eq!(value.state(), ReconciliationState::Reviewing);
}

#[test]
fn stopped_checkpoints_keep_reason_and_usage_even_above_integer_limits() {
    for case in 0..3 {
        let mut value = reconcile();
        let mut observed = observation();
        let expected = match case {
            0 => {
                observed.now_unix_ms = u64::MAX;
                ReconciliationStop::Deadline
            }
            1 => {
                observed.total_tokens = u64::MAX;
                ReconciliationStop::TokensExhausted
            }
            _ => {
                observed.worker_attempts = u32::MAX;
                ReconciliationStop::AttemptsExhausted
            }
        };
        value.apply(observed, proposal(&value, 0)).unwrap();
        let mut value =
            Reconciliation::from_json(&value.to_json().unwrap(), &contract(), observed).unwrap();
        let exhausted = ReconciliationObservation {
            now_unix_ms: u64::MAX,
            total_tokens: u64::MAX,
            worker_attempts: u32::MAX,
        };
        value
            .apply(exhausted, ReconciliationAction::Observe)
            .unwrap();
        assert_eq!(value.observation(), exhausted);
        assert_eq!(value.state(), ReconciliationState::Stopped(expected));
    }
}

#[test]
fn malformed_nested_journal_fields_and_candidate_context_are_rejected() {
    let mut value = reconcile();
    paired(&mut value);
    let before = value.to_json().unwrap();
    let mut changed = body();
    changed.context.code_revision = "other".into();
    assert!(
        value
            .apply(
                observation(),
                ReconciliationAction::Reconcile {
                    base: value.revision().reference(),
                    body: Box::new(changed),
                }
            )
            .is_err()
    );
    assert_eq!(value.to_json().unwrap(), before);
    for case in 0..3 {
        let mut encoded: serde_json::Value = serde_json::from_slice(&before).unwrap();
        match case {
            0 => encoded["entries"][1]["action"]["unexpected"] = true.into(),
            1 => encoded["entries"][0]["observation"]["unexpected"] = true.into(),
            _ => encoded["entries"][1]["action"]["peer"]["model"]["effort"] = "substituted".into(),
        }
        assert!(
            Reconciliation::from_json(
                &serde_json::to_vec(&encoded).unwrap(),
                &contract(),
                observation()
            )
            .is_err()
        );
    }
    let text = String::from_utf8(before).unwrap();
    let duplicate = text.replacen(
        "\"schema_version\":2",
        "\"schema_version\":2,\"schema_version\":2",
        1,
    );
    assert!(Reconciliation::from_json(duplicate.as_bytes(), &contract(), observation()).is_err());
}

#[test]
fn replay_rejects_actions_that_would_be_silently_normalized() {
    let mut value = reconcile();
    let impossible_vote = vote(&value, 0);
    let expired = ReconciliationObservation {
        now_unix_ms: contract().limits.deadline_unix_ms,
        ..observation()
    };
    value.apply(expired, impossible_vote.clone()).unwrap();
    let bytes = value.to_json().unwrap();
    assert!(Reconciliation::from_json(&bytes, &contract(), expired).is_ok());
    let mut encoded: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    encoded["entries"][1]["action"] = serde_json::to_value(impossible_vote).unwrap();
    assert!(
        Reconciliation::from_json(&serde_json::to_vec(&encoded).unwrap(), &contract(), expired)
            .is_err()
    );
}

#[test]
fn replay_rejects_wrong_contract_unknown_fields_and_impossible_events() {
    let mut value = reconcile();
    candidate(&mut value);
    value.apply(observation(), vote(&value, 0)).unwrap();
    let bytes = value.to_json().unwrap();
    let mut wrong = contract();
    wrong.limits.max_total_tokens += 1;
    assert!(Reconciliation::from_json(&bytes, &wrong, observation()).is_err());
    for case in 0..4 {
        let mut encoded: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        match case {
            0 => encoded["schema_version"] = 99.into(),
            1 => encoded["votes"] = true.into(),
            2 => {
                encoded["entries"].as_array_mut().unwrap().remove(1);
            }
            _ => encoded["entries"][0]["observation"]["total_tokens"] = 1.into(),
        }
        assert!(
            Reconciliation::from_json(
                &serde_json::to_vec(&encoded).unwrap(),
                &contract(),
                observation()
            )
            .is_err(),
            "case {case}"
        );
    }
}
