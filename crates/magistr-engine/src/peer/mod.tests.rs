use super::*;

#[path = "reconciliation.tests.rs"]
mod reconciliation;

#[path = "task_authority.tests.rs"]
mod task_authority;

#[path = "peer_request.tests.rs"]
mod peer_request;

#[path = "shared_context.tests.rs"]
mod shared_context;

#[test]
fn revision_v1_has_a_frozen_digest() {
    assert_eq!(
        agreement().revision().reference().digest,
        "e590113a38b7bcd987f3cb48817d4e383d9ce33063d9ff74483c4f2479a14acc"
    );
}

#[test]
fn revision_overflow_preserves_revision_and_both_votes() {
    let revision = PlanRevision::seal(contract(), body(), u64::MAX).unwrap();
    let mut agreement = PlanAgreement::from_revision(revision);
    approve_both(&mut agreement);
    let old = agreement.revision().reference();
    assert!(matches!(
        agreement.revise(body()),
        Err(AgreementError::RevisionOverflow)
    ));
    assert_eq!(agreement.revision().reference(), old);
    assert!(agreement.has_dual_approval());
}

#[test]
fn snapshot_rejects_nested_unknown_and_duplicate_fields() {
    let bytes = agreement().revision().to_json().unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["contract"]["limits"]["unlimited"] = true.into();
    assert!(PlanRevision::from_json(&serde_json::to_vec(&value).unwrap(), &contract()).is_err());
    let json = String::from_utf8(bytes).unwrap();
    let duplicate = json.replacen("\"revision\":1", "\"revision\":1,\"revision\":1", 1);
    assert!(PlanRevision::from_json(duplicate.as_bytes(), &contract()).is_err());
    let duplicate_nested = json.replacen(
        "\"max_attempts\":10",
        "\"max_attempts\":10,\"max_attempts\":10",
        1,
    );
    assert!(PlanRevision::from_json(duplicate_nested.as_bytes(), &contract()).is_err());
}

#[test]
fn duplicate_workers_and_provider_scoped_voting_sessions_are_invalid() {
    let mut value = contract();
    value
        .allowed_worker_models
        .push(value.allowed_worker_models[0].clone());
    assert!(PlanAgreement::new(value, body()).is_err());
    let mut next = body();
    next.worker_models.push(next.worker_models[0].clone());
    assert!(PlanAgreement::new(contract(), next).is_err());
    let mut value = contract();
    value.peers[1].model.provider = value.peers[0].model.provider.clone();
    value.peers[1].session_id = value.peers[0].session_id.clone();
    assert!(PlanAgreement::new(value, body()).is_err());
}

#[test]
fn default_reconciliation_is_bounded_and_worker_selection_is_exact() {
    let limits = RunLimits::new(100, 1000, 2);
    assert_eq!(limits.max_reconciliation_rounds, 2);
    assert_eq!(limits.max_retries_per_task, 0);
    for invalid in ["", " ", "gpt-*", "gpt-?"] {
        let mut value = contract();
        value.allowed_worker_models[0].model = invalid.into();
        assert!(PlanAgreement::new(value, body()).is_err());
    }
}

fn model(provider: &str, name: &str) -> WorkerModel {
    WorkerModel {
        provider: provider.into(),
        model: name.into(),
        effort: "medium".into(),
    }
}

fn contract() -> RunContract {
    RunContract {
        project_id: "project-a".into(),
        run_id: "run-1".into(),
        peers: [
            PeerIdentity {
                peer: Peer::Astra,
                session_id: "astra-session".into(),
                model: model("openai", "gpt-6-astra"),
            },
            PeerIdentity {
                peer: Peer::Fable,
                session_id: "fable-session".into(),
                model: model("anthropic", "claude-fable-5-1"),
            },
        ],
        allowed_worker_models: vec![
            model("openai", "gpt-5.6-sol"),
            model("anthropic", "claude-opus-5"),
        ],
        limits: RunLimits {
            deadline_unix_ms: 2_000_000_000_000,
            max_total_tokens: 100_000,
            max_attempts: 10,
            max_reconciliation_rounds: 2,
            max_retries_per_task: 1,
        },
        outage_policy: OutagePolicy::CheckpointAndStop,
    }
}

fn body() -> PlanBody {
    PlanBody {
        scope: vec!["engine/src/peer".into()],
        context: PlanContext {
            roadmap_revision: "roadmap:42".into(),
            code_revision: "git:abc".into(),
            constraints: vec!["preserve user changes".into()],
            routing_history_revision: "history:0".into(),
        },
        tasks: vec![
            PlanTask {
                id: "build".into(),
                description: "Implement".into(),
                dependencies: vec![],
                owner: Peer::Astra,
                acceptance: vec!["tests pass".into()],
                risks: vec![],
            },
            PlanTask {
                id: "review".into(),
                description: "Review".into(),
                dependencies: vec!["build".into()],
                owner: Peer::Fable,
                acceptance: vec!["review complete".into()],
                risks: vec!["missed regression".into()],
            },
        ],
        worker_models: vec![model("openai", "gpt-5.6-sol")],
        routing_policy: "choose by verified outcomes".into(),
        retry_allowance: 1,
        acceptance: vec!["required gates pass".into()],
    }
}

fn agreement() -> PlanAgreement {
    PlanAgreement::new(contract(), body()).unwrap()
}

fn approve_both(agreement: &mut PlanAgreement) {
    let reference = agreement.revision().reference();
    for identity in &contract().peers {
        agreement.approve(identity, &reference).unwrap();
    }
}

#[test]
fn one_peer_twice_and_silence_never_make_agreement() {
    let mut agreement = agreement();
    let reference = agreement.revision().reference();
    assert!(!agreement.has_dual_approval());
    agreement.approve(&contract().peers[0], &reference).unwrap();
    agreement.approve(&contract().peers[0], &reference).unwrap();
    assert!(!agreement.has_dual_approval());
    agreement.approve(&contract().peers[1], &reference).unwrap();
    assert!(agreement.has_dual_approval());
}

#[test]
fn wrong_session_model_provider_effort_or_role_never_votes() {
    for field in 0..5 {
        let mut agreement = agreement();
        let mut identity = contract().peers[1].clone();
        match field {
            0 => identity.session_id.push_str("-other"),
            1 => identity.model.model = "substitute".into(),
            2 => identity.model.provider = "other".into(),
            3 => identity.model.effort = "low".into(),
            _ => identity.peer = Peer::Astra,
        }
        let reference = agreement.revision().reference();
        agreement.approve(&contract().peers[0], &reference).unwrap();
        assert!(agreement.approve(&identity, &reference).is_err());
        assert!(!agreement.has_dual_approval());
    }
}

#[test]
fn cross_project_run_revision_and_digest_approvals_are_rejected() {
    for field in 0..4 {
        let mut agreement = agreement();
        let mut reference = agreement.revision().reference();
        match field {
            0 => reference.project_id.push('x'),
            1 => reference.run_id.push('x'),
            2 => reference.revision += 1,
            _ => reference.digest.push('x'),
        }
        assert!(agreement.approve(&contract().peers[0], &reference).is_err());
        assert!(!agreement.has_dual_approval());
    }
}

#[test]
fn changed_scope_ownership_policy_and_repeated_body_clear_both_approvals() {
    let mut agreement = agreement();
    for field in 0..4 {
        approve_both(&mut agreement);
        let old = agreement.revision().reference();
        let mut next = agreement.revision().body().clone();
        match field {
            0 => next.scope.push("docs".into()),
            1 => next.tasks[0].owner = Peer::Fable,
            2 => next.routing_policy = "round robin".into(),
            _ => {}
        }
        agreement.revise(next).unwrap();
        assert_eq!(agreement.revision().reference().revision, old.revision + 1);
        assert_ne!(agreement.revision().reference().digest, old.digest);
        assert_eq!(agreement.revision().contract(), &contract());
        assert!(!agreement.has_dual_approval());
        for peer in &contract().peers {
            assert!(agreement.approve(peer, &old).is_err());
        }
    }
}

#[test]
fn invalid_revision_does_not_destroy_existing_approvals() {
    let mut agreement = agreement();
    approve_both(&mut agreement);
    let old = agreement.revision().reference();
    let mut next = body();
    next.tasks[0].dependencies.push("missing".into());
    assert!(agreement.revise(next).is_err());
    assert_eq!(agreement.revision().reference(), old);
    assert!(agreement.has_dual_approval());
}

#[test]
fn plan_cannot_expand_model_retry_or_attempt_allowances() {
    for field in 0..3 {
        let mut agreement = agreement();
        let mut next = body();
        match field {
            0 => next.worker_models.push(model("other", "unapproved")),
            1 => next.retry_allowance = 2,
            _ => {
                for n in 0..10 {
                    let mut task = next.tasks[0].clone();
                    task.id = format!("extra-{n}");
                    next.tasks.push(task);
                }
            }
        }
        assert!(agreement.revise(next).is_err());
    }
}

#[test]
fn rejects_missing_duplicate_and_cyclic_dependencies() {
    for field in 0..5 {
        let mut next = body();
        match field {
            0 => next.tasks[0].dependencies.push("absent".into()),
            1 => next.tasks[0].dependencies.push("build".into()),
            2 => next.tasks[0].dependencies.push("review".into()),
            3 => next.tasks[1].id = "build".into(),
            _ => next.tasks[1].dependencies.push("build".into()),
        }
        assert!(
            PlanAgreement::new(contract(), next).is_err(),
            "case {field}"
        );
    }
}

#[test]
fn rejects_invalid_contracts_and_empty_required_fields() {
    for field in 0..8 {
        let mut value = contract();
        match field {
            0 => value.project_id.clear(),
            1 => value.run_id = "  ".into(),
            2 => value.peers[1].peer = Peer::Astra,
            3 => value.peers[0].session_id.clear(),
            4 => value.limits.max_attempts = 0,
            5 => value.limits.deadline_unix_ms = 0,
            6 => value.limits.max_total_tokens = 0,
            _ => value.limits.max_reconciliation_rounds = 0,
        }
        assert!(PlanAgreement::new(value, body()).is_err(), "case {field}");
    }
    for field in 0..7 {
        let mut next = body();
        match field {
            0 => next.scope.clear(),
            1 => next.acceptance.clear(),
            2 => next.tasks[0].acceptance.clear(),
            3 => next.context.code_revision.clear(),
            4 => next.worker_models.clear(),
            5 => next.routing_policy.clear(),
            _ => next.tasks[0].id.clear(),
        }
        assert!(
            PlanAgreement::new(contract(), next).is_err(),
            "case {field}"
        );
    }
}

#[test]
fn digest_binds_all_consent_bearing_fields() {
    let base = agreement().revision().reference().digest;
    for field in 0..16 {
        let mut next = body();
        match field {
            0 => next.scope.push("docs".into()),
            1 => next.context.roadmap_revision.push('x'),
            2 => next.context.code_revision.push('x'),
            3 => next.context.constraints.push("extra".into()),
            4 => next.context.routing_history_revision.push('x'),
            5 => next.tasks[0].description.push('x'),
            6 => next.tasks[1].dependencies.clear(),
            7 => next.tasks[0].owner = Peer::Fable,
            8 => next.tasks[0].acceptance.push("extra".into()),
            9 => next.tasks[0].risks.push("extra".into()),
            10 => next
                .worker_models
                .push(contract().allowed_worker_models[1].clone()),
            11 => next.routing_policy.push('x'),
            12 => next.retry_allowance = 0,
            13 => next.acceptance.push("extra".into()),
            14 => next.tasks.reverse(),
            _ => {
                next.tasks[0].id = "new".into();
                next.tasks[1].dependencies = vec!["new".into()];
            }
        }
        assert_ne!(
            PlanAgreement::new(contract(), next)
                .unwrap()
                .revision()
                .reference()
                .digest,
            base,
            "field {field}"
        );
    }
    for field in 0..10 {
        let mut value = contract();
        match field {
            0 => value.project_id.push('x'),
            1 => value.run_id.push('x'),
            2 => value.peers[0].session_id.push('x'),
            3 => value.peers[0].model.effort.push('x'),
            4 => value.limits.deadline_unix_ms += 1,
            5 => value.limits.max_total_tokens += 1,
            6 => value.limits.max_attempts += 1,
            7 => value.limits.max_reconciliation_rounds += 1,
            8 => value.limits.max_retries_per_task += 1,
            _ => value.outage_policy = OutagePolicy::CheckpointAffectedAndContinueIndependent,
        }
        assert_ne!(
            PlanAgreement::new(value, body())
                .unwrap()
                .revision()
                .reference()
                .digest,
            base,
            "contract field {field}"
        );
    }
}

#[test]
fn sealed_revision_roundtrips_with_expected_contract_and_stable_digest() {
    let agreement = agreement();
    let revision = agreement.revision();
    let bytes = revision.to_json().unwrap();
    let restored = PlanRevision::from_json(&bytes, &contract()).unwrap();
    assert_eq!(restored.reference(), revision.reference());
    assert_eq!(restored.body(), revision.body());
    assert_eq!(restored.to_json().unwrap(), bytes);
    assert_eq!(restored.reference().digest.len(), 64);
    // JSON object ordering and whitespace are not part of the typed payload.
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let pretty = serde_json::to_vec_pretty(&value).unwrap();
    assert_eq!(
        PlanRevision::from_json(&pretty, &contract())
            .unwrap()
            .reference(),
        revision.reference()
    );
}

#[test]
fn restored_revision_refuses_tampering_unknown_schema_and_foreign_contract() {
    let bytes = agreement().revision().to_json().unwrap();
    for field in 0..5 {
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        match field {
            0 => value["digest"] = "wrong".into(),
            1 => value["schema_version"] = 99.into(),
            2 => value["body"]["scope"] = serde_json::json!(["changed"]),
            3 => value["revision"] = 0.into(),
            _ => value["unrecognized_authority"] = true.into(),
        }
        assert!(
            PlanRevision::from_json(&serde_json::to_vec(&value).unwrap(), &contract()).is_err()
        );
    }
    let mut foreign = contract();
    foreign.limits.max_total_tokens += 1;
    assert!(PlanRevision::from_json(&bytes, &foreign).is_err());
}

#[test]
fn reopening_a_revision_never_restores_votes_from_untrusted_input() {
    let mut agreement = agreement();
    approve_both(&mut agreement);
    let bytes = agreement.revision().to_json().unwrap();
    let restored = PlanRevision::from_json(&bytes, &contract()).unwrap();
    assert!(!PlanAgreement::from_revision(restored).has_dual_approval());
}
