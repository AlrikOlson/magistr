use super::*;
use crate::plan::PlanEngine;

use std::sync::{Arc, Barrier, Mutex};

use rmcp::handler::server::wrapper::Parameters;

use crate::signal::SignalService;
use crate::signal::engine::SignalEngine;
use crate::signal::mcp::handlers::PromoteArgs;

fn promotable() -> (SignalUseCases, Arc<Mutex<PlanEngine>>, Signal) {
    let plan = Arc::new(Mutex::new(PlanEngine::new("project".into())));
    let app = SignalUseCases::new(Arc::new(Mutex::new(SignalEngine::new("project".into()))))
        .with_plan(Arc::clone(&plan));
    let captured = app
        .capture(
            SignalKind::Idea,
            "user".into(),
            "A useful change".into(),
            None,
        )
        .unwrap();
    let signal = app
        .research(&captured.value.id, "Verified".into(), 0.9, vec![], None)
        .unwrap();
    (app, plan, signal.value)
}

#[test]
fn concurrent_native_promotions_create_one_reciprocally_linked_task() {
    let (app, plan, signal) = promotable();
    let barrier = Arc::new(Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let app = app.clone();
                let barrier = Arc::clone(&barrier);
                let id = signal.id.clone();
                scope.spawn(move || {
                    barrier.wait();
                    app.promote(&id).unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|result| result.created).count(), 1);
    let task_id = task_id_for(&signal.id);
    let plan = plan.lock().unwrap();
    assert_eq!(plan.plan().tasks.len(), 1);
    assert_eq!(plan.plan().tasks[0].refs, [format!("signal:{}", signal.id)]);
    for result in results {
        assert_eq!(result.signal.value.status, SignalStatus::Promoted);
        assert_eq!(result.signal.value.refs, [format!("task:{task_id}")]);
    }
}

#[test]
fn retries_recover_both_partial_promotion_boundaries() {
    for already_promoted in [false, true] {
        let (app, plan, signal) = promotable();
        let task_id = task_id_for(&signal.id);
        {
            let mut plan = plan.lock().unwrap();
            plan.add_task(
                task_id.clone(),
                "Change".into(),
                TaskStatus::Idea,
                0,
                signal.body.clone(),
                vec![],
                vec![],
                false,
            )
            .unwrap();
            plan.link_task(&task_id, &format!("signal:{}", signal.id))
                .unwrap();
        }
        if already_promoted {
            app.engine().lock().unwrap().promote(&signal.id).unwrap();
        }
        let result = app.promote(&signal.id).unwrap();
        assert_eq!(result.signal.value.status, SignalStatus::Promoted);
        assert!(
            result
                .signal
                .value
                .refs
                .contains(&format!("task:{task_id}"))
        );
        assert!(!app.promote(&signal.id).unwrap().created);
        assert_eq!(plan.lock().unwrap().plan().tasks.len(), 1);
    }
}

#[tokio::test]
async fn native_and_mcp_promotion_return_both_committed_owners() {
    let (app, plan, signal) = promotable();
    let service = SignalService::from_use_cases(app.clone());
    let fresh = service
        .signal_promote(Parameters(PromoteArgs {
            id: signal.id.clone(),
        }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(fresh["created"], true);
    assert!(fresh.get("signal").is_some());
    assert!(fresh["task"].get("source").is_some());
    assert!(fresh["signal"].get("source").is_some());
    assert!(!app.promote(&signal.id).unwrap().created);
    let retry = service
        .signal_promote(Parameters(PromoteArgs {
            id: signal.id.clone(),
        }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(retry["created"], false);
    assert!(retry["signal"].get("source").is_some());
    assert_eq!(retry["task"]["value"]["id"], task_id_for(&signal.id));
    assert_eq!(plan.lock().unwrap().plan().tasks.len(), 1);
}

#[test]
fn invalid_state_and_unwired_promotion_leave_native_state_unchanged() {
    let engine = Arc::new(Mutex::new(SignalEngine::new("project".into())));
    let app = SignalUseCases::new(engine);
    let signal = app
        .capture(SignalKind::Bug, "user".into(), "Broken".into(), None)
        .unwrap();
    assert!(matches!(
        app.promote(&signal.value.id),
        Err(SignalError::NotWired)
    ));
    let plan = Arc::new(Mutex::new(PlanEngine::new("project".into())));
    let app = app.with_plan(Arc::clone(&plan));
    assert!(matches!(
        app.promote(&signal.value.id),
        Err(SignalError::InvalidState(_))
    ));
    assert!(plan.lock().unwrap().plan().tasks.is_empty());
    assert_eq!(app.get(&signal.value.id).unwrap(), signal.value);
    assert_eq!(task_id_for("f1c9a4e2-1234"), "signal-f1c9a4e2-1234");
}
