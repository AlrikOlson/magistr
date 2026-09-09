use super::*;

#[path = "attempt_ceiling.tests.rs"]
mod attempt_ceiling;
#[path = "worker_admission_replay.tests.rs"]
mod replay;

fn admit(lease: &InstructionLease, id: &str) -> AuthorityAction {
    AuthorityAction::AdmitWorker {
        peer: lease.owner().clone(),
        plan: lease.plan().clone(),
        task: lease.task().into(),
        lease_generation: lease.generation(),
        id: id.into(),
        worker: body().worker_models[0].clone(),
    }
}

fn worker(value: &mut TaskAuthority, task: &str, owner: usize, id: &str) -> WorkerPermit {
    let lease = lease(value, task, owner);
    match value.apply(observation(), admit(&lease, id)).unwrap() {
        AuthorityOutcome::Worker(permit) => *permit,
        other => panic!("expected worker permit, got {other:?}"),
    }
}

fn accept(permit: &WorkerPermit) -> AuthorityAction {
    AuthorityAction::AcceptWorker {
        peer: permit.lease().owner().clone(),
        id: permit.id().into(),
        receipt: "check:verified-by-host".into(),
    }
}

fn retire(id: &str) -> AuthorityAction {
    AuthorityAction::RetireWorker {
        id: id.into(),
        receipt: "runtime:termination-confirmed".into(),
    }
}

#[test]
fn worker_dependencies_require_fenced_acceptance_and_join_both_branches() {
    let mut value = agreed();
    let blocked = lease(&mut value, "left", 1);
    assert!(
        value
            .apply(observation(), admit(&blocked, "too-early"))
            .is_err()
    );
    let build = worker(&mut value, "build", 0, "build-1");
    assert_eq!(build.attempt(), 1);
    assert_eq!(build.worker(), &body().worker_models[0]);
    assert!(
        value
            .validate_worker(observation(), &contract().peers[0], &build)
            .is_ok()
    );
    value.apply(observation(), accept(&build)).unwrap();
    let left = worker(&mut value, "left", 1, "left-1");
    let right = worker(&mut value, "right", 1, "right-1");
    value.apply(observation(), accept(&left)).unwrap();
    let join = lease(&mut value, "join", 1);
    assert!(value.apply(observation(), admit(&join, "join-1")).is_err());
    value.apply(observation(), accept(&right)).unwrap();
    assert!(matches!(
        value.apply(observation(), admit(&join, "join-1")).unwrap(),
        AuthorityOutcome::Worker(_)
    ));
    assert!(
        value
            .validate_worker(observation(), &contract().peers[0], &build)
            .is_err()
    );
}

#[test]
fn worker_requests_deduplicate_without_a_second_permit_and_reject_changed_intent() {
    let mut value = agreed();
    let lease = lease(&mut value, "build", 0);
    let request = admit(&lease, "request-1");
    assert!(matches!(
        value.apply(observation(), request.clone()).unwrap(),
        AuthorityOutcome::Worker(_)
    ));
    assert!(matches!(
        value.apply(observation(), request.clone()).unwrap(),
        AuthorityOutcome::AlreadyAdmitted
    ));
    let mut changed = request;
    if let AuthorityAction::AdmitWorker { worker, .. } = &mut changed {
        *worker = contract().allowed_worker_models[1].clone();
    }
    assert!(value.apply(observation(), changed).is_err());
    assert!(
        value
            .apply(observation(), admit(&lease, "request-2"))
            .is_err()
    );
    value.apply(observation(), retire("request-1")).unwrap();
    let retry = value
        .apply(observation(), admit(&lease, "request-2"))
        .unwrap();
    assert!(matches!(retry, AuthorityOutcome::Worker(permit) if permit.attempt() == 2));
    value.apply(observation(), retire("request-2")).unwrap();
    assert!(
        value
            .apply(observation(), admit(&lease, "request-3"))
            .is_err()
    );
}

#[test]
fn worker_admission_checks_exact_model_owner_reference_and_lease() {
    for field in 0..8 {
        let mut value = agreed();
        let lease = lease(&mut value, "build", 0);
        let mut request = admit(&lease, "request");
        if let AuthorityAction::AdmitWorker {
            peer,
            plan,
            task,
            lease_generation,
            worker,
            id,
        } = &mut request
        {
            match field {
                0 => peer.session_id.push_str("-foreign"),
                1 => plan.revision += 1,
                2 => *task = "unplanned-child".into(),
                3 => *lease_generation += 1,
                4 => worker.model.push_str("-other"),
                5 => worker.provider.push_str("-other"),
                6 => worker.effort.push_str("-other"),
                _ => *id = " ".into(),
            }
        }
        assert!(value.apply(observation(), request).is_err(), "case {field}");
    }
}

#[test]
fn worker_objection_fences_results_preserves_independent_work_and_invalidates_dependencies() {
    let mut value = agreed();
    let build = worker(&mut value, "build", 0, "build-1");
    let independent = worker(&mut value, "independent", 1, "independent-1");
    value.apply(observation(), accept(&build)).unwrap();
    let left = worker(&mut value, "left", 1, "left-1");
    let action = object(&value, "stop", &["build"], 1);
    value.apply(observation(), action).unwrap();
    assert!(value.apply(observation(), accept(&left)).is_err());
    value.apply(observation(), accept(&independent)).unwrap();
    let action = resolve(&value, "stop", 1);
    value.apply(observation(), action).unwrap();
    let right = lease(&mut value, "right", 1);
    assert!(
        value
            .apply(observation(), admit(&right, "right-1"))
            .is_err()
    );
    value.apply(observation(), retire("left-1")).unwrap();
    let build_retry = worker(&mut value, "build", 0, "build-2");
    value.apply(observation(), accept(&build_retry)).unwrap();
    assert!(matches!(
        value
            .apply(observation(), admit(&right, "right-1"))
            .unwrap(),
        AuthorityOutcome::Worker(_)
    ));
}

#[test]
fn worker_reservations_allow_the_final_slot_but_no_additional_start() {
    let mut fixed = contract();
    fixed.limits.max_attempts = 2;
    let mut value = TaskAuthority::new(fixed, body(), observation()).unwrap();
    candidate(&mut value, body());
    vote(&mut value, 0);
    vote(&mut value, 1);
    worker(&mut value, "build", 0, "first");
    value.apply(observation(), retire("first")).unwrap();
    let final_permit = worker(&mut value, "build", 0, "last");
    assert!(
        value
            .validate_worker(observation(), &contract().peers[0], &final_permit)
            .is_ok()
    );
    value.apply(observation(), accept(&final_permit)).unwrap();
    let review = lease(&mut value, "review", 1);
    assert!(
        value
            .apply(observation(), admit(&review, "over-limit"))
            .is_err()
    );
}

#[test]
fn worker_restore_keeps_unsettled_attempts_and_retries_but_discards_authority() {
    let mut value = agreed();
    let old = worker(&mut value, "build", 0, "first");
    let bytes = value.to_json().unwrap();
    let mut restored = TaskAuthority::from_json(&bytes, &contract(), observation()).unwrap();
    assert!(
        restored
            .validate_worker(observation(), &contract().peers[0], &old)
            .is_err()
    );
    assert!(restored.apply(observation(), accept(&old)).is_err());
    let fresh = lease(&mut restored, "build", 0);
    assert!(
        restored
            .apply(observation(), admit(&fresh, "second"))
            .is_err()
    );
    restored.apply(observation(), retire("first")).unwrap();
    assert!(
        matches!(restored.apply(observation(), admit(&fresh, "second")).unwrap(), AuthorityOutcome::Worker(p) if p.attempt() == 2)
    );
    let mut again =
        TaskAuthority::from_json(&restored.to_json().unwrap(), &contract(), observation()).unwrap();
    again.apply(observation(), retire("second")).unwrap();
    let fresh = lease(&mut again, "build", 0);
    assert!(again.apply(observation(), admit(&fresh, "third")).is_err());
}
