use super::*;

use rmcp::handler::server::wrapper::Parameters;

use crate::content::StructuredContent;
use crate::plan::PlanService;
use crate::plan::application::{
    ExportFormat, ExportOutcome, ExportRequest, ExportTarget, FocusChange, TaskPatch, WaitingDraft,
    WorkstreamInspection, WorkstreamView,
};
use crate::plan::domain::{FocusMode, FocusState, WaitingReason};
use crate::plan::mcp::handlers::{FocusGetArgs, FocusSetArgs, ProposeFeaturesArgs, UpdateTaskArgs};
use crate::run::engine::RunEngine;

/// A plan over a run family with no evidence capability: nothing here
/// completes a task.
fn application() -> PlanUseCases {
    PlanUseCases::new(
        Arc::new(Mutex::new(PlanEngine::new("project".into()))),
        RunUseCases::new(Arc::new(Mutex::new(RunEngine::new("project".into())))),
    )
}

fn draft() -> TaskDraft {
    TaskDraft {
        id: "feature".into(),
        title: "Original claim".into(),
        name: String::new(),
        status: TaskStatus::ToDo,
        priority: 42,
        description: String::new(),
        acceptance: vec![],
        deps: vec![],
        shared: false,
        feature: Some("Delivery".into()),
        content: None,
    }
}

#[tokio::test]
async fn native_patch_and_mcp_share_proposal_order_and_validation() {
    let app = application();
    app.add_task(draft()).unwrap();
    app.propose_title(
        "feature",
        "Suggested claim".into(),
        "Reason".into(),
        "tracker".into(),
    )
    .unwrap();
    let before = app.engine().lock().unwrap().plan().tasks[0].clone();
    let invalid = TaskPatch {
        resolve_title_proposal: Some(true),
        content: Some(StructuredContent {
            version: 2,
            summary: "Invalid version".into(),
            facts: None,
            sections: None,
        }),
        ..TaskPatch::default()
    };
    assert!(matches!(
        app.update_task("feature", invalid),
        Err(PlanError::InvalidArgs(_))
    ));
    assert_eq!(app.engine().lock().unwrap().plan().tasks[0], before);
    let service = PlanService::from_use_cases(app.clone());
    let args = serde_json::from_value::<UpdateTaskArgs>(serde_json::json!({
        "id": "feature", "resolve_title_proposal": "accept", "title": "Final human claim",
    }))
    .unwrap();
    let response = service.plan_update_task(Parameters(args)).await.unwrap();
    let task = app.engine().lock().unwrap().plan().tasks[0].clone();
    assert_eq!(task.title, "Final human claim");
    assert!(task.title_proposal.is_none());
    assert_eq!(
        response.structured_content.unwrap()["value"],
        serde_json::to_value(&task).unwrap()
    );
    assert!(Arc::ptr_eq(&service.engine(), &app.engine()));
}

#[test]
fn native_blocker_commands_validate_and_clear_through_the_same_patch() {
    let app = application();
    let original = app.add_task(draft()).unwrap();
    let blocker = WaitingDraft {
        kind: WaitingReason::Person,
        reason: "  ".into(),
        evidence: None,
    };
    assert!(matches!(
        app.update_task(
            "feature",
            TaskPatch {
                waiting_on: Some(blocker),
                ..TaskPatch::default()
            }
        ),
        Err(PlanError::InvalidArgs(_))
    ));
    assert_eq!(app.engine().lock().unwrap().plan().tasks[0], original.value);
    let blocked = app
        .update_task(
            "feature",
            TaskPatch {
                waiting_on: Some(WaitingDraft {
                    kind: WaitingReason::Person,
                    reason: "Need owner input".into(),
                    evidence: Some("decision:7".into()),
                }),
                ..TaskPatch::default()
            },
        )
        .unwrap();
    assert_eq!(
        blocked.value.waiting_on.unwrap().evidence.as_deref(),
        Some("decision:7")
    );
    assert!(
        app.update_task(
            "feature",
            TaskPatch {
                clear_waiting_on: true,
                ..TaskPatch::default()
            }
        )
        .unwrap()
        .value
        .waiting_on
        .is_none()
    );
    assert!(matches!(
        app.update_task(
            "feature",
            TaskPatch {
                clear_waiting_on: true,
                ..TaskPatch::default()
            }
        ),
        Err(PlanError::InvalidState(_))
    ));
}

#[test]
fn native_queries_include_shared_requests_and_keep_lane_isolation() {
    let app = application();
    app.add_task(draft()).unwrap();
    let requests = crate::request::application::RequestUseCases::new(Arc::new(Mutex::new(
        RequestEngine::new("project".into()),
    )));
    let request = requests
        .capture(
            crate::request::domain::RequestKind::Suggestion,
            "user".into(),
            "Relevant idea".into(),
            None,
        )
        .unwrap();
    requests
        .research(&request.value.id, "Evidence".into(), 0.9, vec![], None)
        .unwrap();
    let app = app.with_request(requests.engine());
    assert_eq!(app.status().unwrap()["ready_requests"], 1);
    app.focus_set("lane-a", "Delivery", FocusMode::Build)
        .unwrap();
    assert!(
        app.focus_get("lane-b", None, WorkstreamView::Current)
            .unwrap()
            .focus
            .is_none()
    );
    let before = app
        .focus_get("lane-a", None, WorkstreamView::Current)
        .unwrap()
        .focus;
    assert!(
        app.focus_set("lane-a", "unknown", FocusMode::Shape)
            .is_err()
    );
    assert_eq!(
        app.focus_get("lane-a", None, WorkstreamView::Current)
            .unwrap()
            .focus,
        before
    );
    assert!(app.focus_clear(" ").is_err());
    app.focus_clear("lane-a").unwrap();
    assert!(
        app.focus_get("lane-a", None, WorkstreamView::Current)
            .unwrap()
            .focus
            .is_none()
    );
}

#[test]
fn export_capability_requires_a_workspace_only_for_file_delivery() {
    let app = application();
    app.add_task(draft()).unwrap();
    assert!(matches!(
        app.export(ExportRequest {
            format: ExportFormat::Json,
            target: ExportTarget::File
        }),
        Err(PlanError::WorkspaceUnavailable)
    ));
    let result = app
        .export(ExportRequest {
            format: ExportFormat::Markdown,
            target: ExportTarget::Inline,
        })
        .unwrap();
    assert!(
        matches!(result, ExportOutcome::Inline { plan, .. } if plan.contains("Original claim"))
    );
}

fn add_workstream_task(app: &PlanUseCases, id: &str, feature: Option<&str>, status: TaskStatus) {
    app.add_task(TaskDraft {
        id: id.into(),
        title: format!("Work for {id}"),
        feature: feature.map(str::to_owned),
        status,
        ..draft()
    })
    .unwrap();
}

#[test]
fn focus_catalog_separates_closed_history_and_unassigned_work_without_rewriting_records() {
    let app = application();
    add_workstream_task(&app, "finished", Some("Archive"), TaskStatus::Done);
    add_workstream_task(&app, "abandoned", Some("Archive"), TaskStatus::Retired);
    add_workstream_task(&app, "feature", Some("Delivery"), TaskStatus::ToDo);
    add_workstream_task(&app, "loose", None, TaskStatus::Idea);
    add_workstream_task(&app, "loose-finished", None, TaskStatus::Done);
    let before = app.engine().lock().unwrap().plan().clone();

    let read = app
        .focus_get("lane-a", None, WorkstreamView::Current)
        .unwrap();
    assert!(read.focus.is_none());
    assert_eq!(read.focus_state, FocusState::Unset);
    assert_eq!(read.frontier, WorkstreamInspection::Unset);
    assert_eq!(read.workstreams.current_names(), vec!["Delivery"]);
    assert_eq!(read.workstreams.history["Archive"].done, 1);
    assert_eq!(read.workstreams.history["Archive"].retired, 1);
    assert_eq!(read.workstreams.unassigned.counts.total, 1);
    assert_eq!(read.workstreams.unassigned_history.done, 1);
    assert!(!read.persistent);
    assert!(
        !serde_json::to_string(&read)
            .unwrap()
            .contains("\"task_ids\":")
    );

    let proposed = app.propose_features(4).unwrap();
    assert_eq!(proposed.authored_names, vec!["Archive", "Delivery"]);
    assert_eq!(proposed.placed, 0);
    let error = app
        .focus_set("lane-a", "Archive", FocusMode::Build)
        .unwrap_err();
    assert!(error.to_string().contains("no unfinished work"));
    assert!(error.to_string().contains("Delivery"));
    assert_eq!(app.engine().lock().unwrap().plan(), &before);
}

#[test]
fn obsoleting_the_last_open_task_keeps_exact_saved_focus_and_other_lanes() {
    let app = application();
    add_workstream_task(&app, "feature", Some("Delivery"), TaskStatus::ToDo);
    add_workstream_task(&app, "other", Some("Delivery tools"), TaskStatus::ToDo);
    let FocusChange::Set {
        focus: saved,
        frontier,
        ..
    } = app
        .focus_set("lane-a", "Delivery", FocusMode::Shape)
        .unwrap()
        .value
    else {
        panic!("focus set must return the selected frontier");
    };
    assert_eq!(frontier.feature.as_deref(), Some("Delivery"));
    assert_eq!(frontier.activity.next_ready.as_deref(), Some("feature"));
    app.focus_set("lane-b", "Delivery tools", FocusMode::Listen)
        .unwrap();
    let other_lane = app
        .focus_get("lane-b", None, WorkstreamView::Current)
        .unwrap()
        .focus;
    app.retire_task("feature", "Superseded by completed delivery work".into())
        .unwrap();

    let read = app
        .focus_get("lane-a", None, WorkstreamView::Current)
        .unwrap();
    assert_eq!(read.focus, Some(saved.clone()));
    assert_eq!(
        read.focus_state,
        FocusState::Closed {
            feature: "Delivery".into()
        }
    );
    assert!(
        !serde_json::to_string(&read)
            .unwrap()
            .contains("\"task_ids\":")
    );
    assert!(matches!(
        read.frontier,
        WorkstreamInspection::Closed { feature, history }
            if feature == "Delivery" && history.retired == 1
    ));
    assert_eq!(read.workstreams.current_names(), vec!["Delivery tools"]);
    let history = app
        .focus_get("lane-a", None, WorkstreamView::History)
        .unwrap();
    assert_eq!(history.focus, Some(saved.clone()));
    assert_eq!(history.focus_state, read.focus_state);
    assert!(matches!(
        history.frontier,
        WorkstreamInspection::History { feature, history }
            if feature == "Delivery" && history.counts.retired == 1
                && history.task_ids == ["feature"]
    ));

    let peek = app
        .focus_get("lane-a", Some("tools"), WorkstreamView::Current)
        .unwrap();
    assert_eq!(peek.focus, Some(saved.clone()));
    assert_eq!(
        peek.focus_state,
        FocusState::Closed {
            feature: "Delivery".into()
        }
    );
    assert!(matches!(
        peek.frontier,
        WorkstreamInspection::Current { frontier }
            if frontier.feature.as_deref() == Some("Delivery tools")
    ));
    let exact = app
        .focus_get("lane-a", Some("Delivery"), WorkstreamView::Current)
        .unwrap();
    assert!(matches!(
        exact.frontier,
        WorkstreamInspection::Closed { .. }
    ));
    assert!(
        app.focus_set("lane-a", "Delivery", FocusMode::Build)
            .is_err()
    );
    assert_eq!(
        app.focus_get("lane-a", None, WorkstreamView::Current)
            .unwrap()
            .focus,
        Some(saved)
    );
    assert_eq!(
        app.focus_get("lane-b", None, WorkstreamView::Current)
            .unwrap()
            .focus,
        other_lane
    );
}

#[test]
fn moving_the_last_task_reports_missing_focus_without_resolving_similar_current_names() {
    let app = application();
    add_workstream_task(&app, "feature", Some("Delivery"), TaskStatus::ToDo);
    app.focus_set("lane-a", "Delivery", FocusMode::Build)
        .unwrap();
    let saved = app
        .focus_get("lane-a", None, WorkstreamView::Current)
        .unwrap()
        .focus;
    app.set_feature("feature", Some("Dispatch".into())).unwrap();
    add_workstream_task(&app, "tools", Some("Delivery tools"), TaskStatus::ToDo);
    add_workstream_task(&app, "reviews", Some("Delivery reviews"), TaskStatus::Idea);
    let before = app.engine().lock().unwrap().plan().clone();

    let read = app
        .focus_get("lane-a", None, WorkstreamView::Current)
        .unwrap();
    assert_eq!(read.focus, saved);
    assert_eq!(
        read.focus_state,
        FocusState::Missing {
            feature: "Delivery".into()
        }
    );
    assert_eq!(
        read.frontier,
        WorkstreamInspection::Missing {
            query: "Delivery".into()
        }
    );

    let history = app
        .focus_get("lane-a", None, WorkstreamView::History)
        .unwrap();
    assert_eq!(history.focus, saved);
    assert_eq!(history.focus_state, read.focus_state);
    assert_eq!(history.frontier, read.frontier);

    let peek = app
        .focus_get("lane-a", Some("Delivery"), WorkstreamView::Current)
        .unwrap();
    assert_eq!(peek.focus_state, read.focus_state);
    assert_eq!(
        peek.frontier,
        WorkstreamInspection::Ambiguous {
            query: "Delivery".into(),
            matches: vec!["Delivery reviews".into(), "Delivery tools".into()],
        }
    );
    let explicit = app
        .focus_get("lane-a", Some("tools"), WorkstreamView::Current)
        .unwrap();
    assert_eq!(explicit.focus_state, read.focus_state);
    assert!(matches!(
        explicit.frontier,
        WorkstreamInspection::Current { frontier }
            if frontier.feature.as_deref() == Some("Delivery tools")
    ));
    let missing = app
        .focus_get("lane-a", Some("Unknown"), WorkstreamView::Current)
        .unwrap();
    assert_eq!(missing.focus_state, read.focus_state);
    assert_eq!(
        missing.frontier,
        WorkstreamInspection::Missing {
            query: "Unknown".into()
        }
    );
    assert_eq!(app.engine().lock().unwrap().plan(), &before);
}

#[test]
fn focus_read_refreshes_catalog_saved_selection_and_frontier_from_one_store_observation() {
    use crate::infra::{Domain, Persistence, PersistenceConfig};

    let directory = tempfile::tempdir().unwrap();
    let config = PersistenceConfig::from_env()
        .with_data_dir(directory.path().to_owned())
        .enabled(true);
    let application = || {
        PlanUseCases::new(
            Arc::new(Mutex::new(
                PlanEngine::new("project".into())
                    .with_persistence(Persistence::new(&config, Domain::Plan)),
            )),
            RunUseCases::new(Arc::new(Mutex::new(RunEngine::new("project".into())))),
        )
    };
    let writer = application();
    add_workstream_task(&writer, "feature", Some("Delivery"), TaskStatus::ToDo);
    writer
        .focus_set("lane-a", "Delivery", FocusMode::Build)
        .unwrap();
    let reader = application();

    writer
        .retire_task("feature", "Finished elsewhere".into())
        .unwrap();
    add_workstream_task(
        &writer,
        "replacement",
        Some("Delivery tools"),
        TaskStatus::ToDo,
    );
    writer
        .focus_set("lane-b", "Delivery tools", FocusMode::Listen)
        .unwrap();

    let read = reader
        .focus_get("lane-a", None, WorkstreamView::Current)
        .unwrap();
    assert!(read.persistent);
    assert_eq!(
        read.focus_state,
        FocusState::Closed {
            feature: "Delivery".into()
        }
    );
    assert_eq!(read.workstreams.current_names(), vec!["Delivery tools"]);
    assert!(matches!(read.frontier, WorkstreamInspection::Closed { .. }));
    let other = reader
        .focus_get("lane-b", None, WorkstreamView::Current)
        .unwrap();
    assert_eq!(other.focus.unwrap().mode, FocusMode::Listen);
    assert!(matches!(
        other.frontier,
        WorkstreamInspection::Current { frontier }
            if frontier.activity.next_ready.as_deref() == Some("replacement")
    ));

    let store = Persistence::new(&config, Domain::Plan).path_for("project");
    std::fs::write(store, b"unreadable plan").unwrap();
    assert!(matches!(
        reader.focus_get("lane-a", None, WorkstreamView::Current),
        Err(PlanError::Store(_))
    ));
}

#[tokio::test]
async fn mcp_history_peek_preserves_current_focus_and_refuses_closed_selection() {
    let app = application();
    add_workstream_task(&app, "finished", Some("Archive"), TaskStatus::Done);
    add_workstream_task(&app, "feature", Some("Delivery"), TaskStatus::ToDo);
    add_workstream_task(&app, "earlier", Some("Delivery"), TaskStatus::Done);
    app.focus_set("lane-a", "Delivery", FocusMode::Build)
        .unwrap();
    let before = app.engine().lock().unwrap().plan().clone();
    let service = PlanService::from_use_cases(app.clone());

    // Omitting the view returns active work and history summaries without IDs.
    let args: FocusGetArgs = serde_json::from_value(serde_json::json!({
        "lane": "lane-a",
    }))
    .unwrap();
    assert_eq!(args.view, WorkstreamView::Current);
    let current = service
        .plan_focus_get(Parameters(args))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(current["frontier"]["state"], "current");
    assert_eq!(
        current["frontier"]["frontier"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["feature"]
    );
    assert!(!current.to_string().contains("\"task_ids\":"));

    // History is an explicit query for either a mixed or a closed workstream.
    for (feature, id) in [(None, "earlier"), (Some("Archive"), "finished")] {
        let read = service
            .plan_focus_get(Parameters(FocusGetArgs {
                lane: "lane-a".into(),
                feature: feature.map(str::to_owned),
                view: WorkstreamView::History,
            }))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(read["focus"]["feature"], "Delivery");
        assert_eq!(
            read["focus_state"],
            serde_json::json!({"state":"current", "feature":"Delivery"})
        );
        assert_eq!(read["frontier"]["state"], "history");
        assert_eq!(read["frontier"]["feature"], feature.unwrap_or("Delivery"));
        assert_eq!(read["frontier"]["history"]["done"], 1);
        assert_eq!(
            read["frontier"]["history"]["task_ids"],
            serde_json::json!([id])
        );
        assert!(read["workstreams"]["current"].get("Archive").is_none());
        assert_eq!(read["workstreams"]["history"]["Archive"]["done"], 1);
        assert_eq!(read["workstreams"]["history"]["Delivery"]["done"], 1);
        assert!(read.get("features").is_none());
    }

    let closed = service
        .plan_focus_get(Parameters(FocusGetArgs {
            lane: "lane-a".into(),
            feature: Some("Archive".into()),
            view: WorkstreamView::Current,
        }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(closed["frontier"]["state"], "closed");
    assert_eq!(closed["frontier"]["history"]["done"], 1);
    assert!(!closed.to_string().contains("\"task_ids\":"));
    assert!(
        serde_json::from_value::<FocusGetArgs>(serde_json::json!({
            "lane": "lane-a", "view": "all",
        }))
        .is_err()
    );

    let refused = service
        .plan_focus_set(Parameters(FocusSetArgs {
            lane: "lane-a".into(),
            feature: "Archive".into(),
            mode: "build".into(),
            clear: false,
        }))
        .await
        .unwrap();
    assert_eq!(refused.is_error, Some(true));
    let refused = refused.structured_content.unwrap();
    assert_eq!(refused["error_kind"], "invalid_args");
    assert!(refused["message"].as_str().unwrap().contains("Delivery"));

    let proposal = service
        .plan_propose_features(Parameters(ProposeFeaturesArgs { min_shared: 4 }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(
        proposal["authored_names"],
        serde_json::json!(["Archive", "Delivery"])
    );
    assert!(proposal.get("features").is_none());
    assert_eq!(app.engine().lock().unwrap().plan(), &before);
}
