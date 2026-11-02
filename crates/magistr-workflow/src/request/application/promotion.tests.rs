use super::*;
use crate::plan::PlanEngine;

use std::sync::{Arc, Barrier, Mutex};

use rmcp::handler::server::wrapper::Parameters;

use crate::request::RequestService;
use crate::request::engine::RequestEngine;
use crate::request::mcp::handlers::PromoteArgs;

fn promotable() -> (RequestUseCases, Arc<Mutex<PlanEngine>>, Request) {
    let plan = Arc::new(Mutex::new(PlanEngine::new("project".into())));
    let app = RequestUseCases::new(Arc::new(Mutex::new(RequestEngine::new("project".into()))))
        .with_plan(Arc::clone(&plan));
    let captured = app
        .capture(
            RequestKind::Suggestion,
            "user".into(),
            "A useful change".into(),
            None,
        )
        .unwrap();
    let request = app
        .research(&captured.value.id, "Verified".into(), 0.9, vec![], None)
        .unwrap();
    (app, plan, request.value)
}

#[test]
fn concurrent_native_promotions_create_one_reciprocally_linked_task() {
    let (app, plan, request) = promotable();
    let barrier = Arc::new(Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let app = app.clone();
                let barrier = Arc::clone(&barrier);
                let id = request.id.clone();
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
    let task_id = task_id_for(&request.id);
    let plan = plan.lock().unwrap();
    assert_eq!(plan.plan().tasks.len(), 1);
    assert_eq!(
        plan.plan().tasks[0].refs,
        [format!("request:{}", request.id)]
    );
    for result in results {
        assert_eq!(result.request.value.status, RequestStatus::Promoted);
        assert_eq!(result.request.value.refs, [format!("task:{task_id}")]);
    }
}

#[test]
fn retries_recover_both_partial_promotion_boundaries() {
    for already_promoted in [false, true] {
        let (app, plan, request) = promotable();
        let task_id = task_id_for(&request.id);
        {
            let mut plan = plan.lock().unwrap();
            plan.add_task(
                task_id.clone(),
                "Change".into(),
                TaskStatus::Idea,
                0,
                request.body.clone(),
                vec![],
                vec![],
                false,
            )
            .unwrap();
            plan.link_task(&task_id, &format!("request:{}", request.id))
                .unwrap();
        }
        if already_promoted {
            app.engine().lock().unwrap().promote(&request.id).unwrap();
        }
        let result = app.promote(&request.id).unwrap();
        assert_eq!(result.request.value.status, RequestStatus::Promoted);
        assert!(
            result
                .request
                .value
                .refs
                .contains(&format!("task:{task_id}"))
        );
        assert!(!app.promote(&request.id).unwrap().created);
        assert_eq!(plan.lock().unwrap().plan().tasks.len(), 1);
    }
}

#[tokio::test]
async fn native_and_mcp_promotion_return_both_committed_owners() {
    let (app, plan, request) = promotable();
    let service = RequestService::from_use_cases(app.clone());
    let fresh = service
        .request_promote(Parameters(PromoteArgs {
            id: request.id.clone(),
        }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(fresh["created"], true);
    assert!(fresh.get("request").is_some());
    assert!(fresh["task"].get("source").is_some());
    assert!(fresh["request"].get("source").is_some());
    assert!(!app.promote(&request.id).unwrap().created);
    let retry = service
        .request_promote(Parameters(PromoteArgs {
            id: request.id.clone(),
        }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(retry["created"], false);
    assert!(retry["request"].get("source").is_some());
    assert_eq!(retry["task"]["value"]["id"], task_id_for(&request.id));
    assert_eq!(plan.lock().unwrap().plan().tasks.len(), 1);
}

#[test]
fn invalid_state_and_unwired_promotion_leave_native_state_unchanged() {
    let engine = Arc::new(Mutex::new(RequestEngine::new("project".into())));
    let app = RequestUseCases::new(engine);
    let request = app
        .capture(RequestKind::Bug, "user".into(), "Broken".into(), None)
        .unwrap();
    assert!(matches!(
        app.promote(&request.value.id),
        Err(RequestError::NotWired)
    ));
    let plan = Arc::new(Mutex::new(PlanEngine::new("project".into())));
    let app = app.with_plan(Arc::clone(&plan));
    assert!(matches!(
        app.promote(&request.value.id),
        Err(RequestError::InvalidState(_))
    ));
    assert!(plan.lock().unwrap().plan().tasks.is_empty());
    assert_eq!(app.get(&request.value.id).unwrap(), request.value);
    assert_eq!(task_id_for("f1c9a4e2-1234"), "request-f1c9a4e2-1234");
}
