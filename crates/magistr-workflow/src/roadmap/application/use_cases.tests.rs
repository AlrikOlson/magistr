use super::*;

use rmcp::handler::server::wrapper::Parameters;

use crate::content::StructuredContent;
use crate::roadmap::RoadmapService;
use crate::roadmap::application::{
    BlockerDraft, ChunkPatch, ExportFormat, ExportOutcome, ExportRequest, ExportTarget,
    FocusChange, WorkstreamInspection, WorkstreamView,
};
use crate::roadmap::domain::{BlockerKind, FocusMode, FocusState};
use crate::roadmap::mcp::handlers::{
    FocusGetArgs, FocusSetArgs, ProposeGroupsArgs, UpdateChunkArgs,
};
use crate::ship::engine::ShipEngine;

/// A roadmap over a ship family with no evidence capability: nothing here
/// completes a chunk.
fn application() -> RoadmapUseCases {
    RoadmapUseCases::new(
        Arc::new(Mutex::new(RoadmapEngine::new("project".into()))),
        ShipUseCases::new(Arc::new(Mutex::new(ShipEngine::new("project".into())))),
    )
}

fn draft() -> ChunkDraft {
    ChunkDraft {
        id: "feature".into(),
        title: "Original claim".into(),
        name: String::new(),
        status: ChunkStatus::Pending,
        priority: 42,
        description: String::new(),
        acceptance: vec![],
        deps: vec![],
        shared: false,
        group: Some("Delivery".into()),
        content: None,
    }
}

#[tokio::test]
async fn native_patch_and_mcp_share_proposal_order_and_validation() {
    let app = application();
    app.add_chunk(draft()).unwrap();
    app.propose_title(
        "feature",
        "Suggested claim".into(),
        "Reason".into(),
        "tracker".into(),
    )
    .unwrap();
    let before = app.engine().lock().unwrap().roadmap().chunks[0].clone();
    let invalid = ChunkPatch {
        resolve_title_proposal: Some(true),
        content: Some(StructuredContent {
            version: 2,
            summary: "Invalid version".into(),
            facts: None,
            sections: None,
        }),
        ..ChunkPatch::default()
    };
    assert!(matches!(
        app.update_chunk("feature", invalid),
        Err(RoadmapError::InvalidArgs(_))
    ));
    assert_eq!(app.engine().lock().unwrap().roadmap().chunks[0], before);
    let service = RoadmapService::from_use_cases(app.clone());
    let args = serde_json::from_value::<UpdateChunkArgs>(serde_json::json!({
        "id": "feature", "resolve_title_proposal": "accept", "title": "Final human claim",
    }))
    .unwrap();
    let response = service
        .roadmap_update_chunk(Parameters(args))
        .await
        .unwrap();
    let chunk = app.engine().lock().unwrap().roadmap().chunks[0].clone();
    assert_eq!(chunk.title, "Final human claim");
    assert!(chunk.title_proposal.is_none());
    assert_eq!(
        response.structured_content.unwrap(),
        serde_json::to_value(&chunk).unwrap()
    );
    assert!(Arc::ptr_eq(&service.engine(), &app.engine()));
}

#[test]
fn native_blocker_commands_validate_and_clear_through_the_same_patch() {
    let app = application();
    let original = app.add_chunk(draft()).unwrap();
    let blocker = BlockerDraft {
        kind: BlockerKind::AwaitingHuman,
        reason: "  ".into(),
        evidence: None,
    };
    assert!(matches!(
        app.update_chunk(
            "feature",
            ChunkPatch {
                blocked_by: Some(blocker),
                ..ChunkPatch::default()
            }
        ),
        Err(RoadmapError::InvalidArgs(_))
    ));
    assert_eq!(app.engine().lock().unwrap().roadmap().chunks[0], original);
    let blocked = app
        .update_chunk(
            "feature",
            ChunkPatch {
                blocked_by: Some(BlockerDraft {
                    kind: BlockerKind::AwaitingHuman,
                    reason: "Need owner input".into(),
                    evidence: Some("think:7".into()),
                }),
                ..ChunkPatch::default()
            },
        )
        .unwrap();
    assert_eq!(
        blocked.blocked_by.unwrap().evidence.as_deref(),
        Some("think:7")
    );
    assert!(
        app.update_chunk(
            "feature",
            ChunkPatch {
                clear_blocked_by: true,
                ..ChunkPatch::default()
            }
        )
        .unwrap()
        .blocked_by
        .is_none()
    );
    assert!(matches!(
        app.update_chunk(
            "feature",
            ChunkPatch {
                clear_blocked_by: true,
                ..ChunkPatch::default()
            }
        ),
        Err(RoadmapError::InvalidState(_))
    ));
}

#[test]
fn native_queries_include_shared_signals_and_keep_lane_isolation() {
    let app = application();
    app.add_chunk(draft()).unwrap();
    let signals = crate::signal::application::SignalUseCases::new(Arc::new(Mutex::new(
        SignalEngine::new("project".into()),
    )));
    let signal = signals
        .capture(
            crate::signal::domain::SignalKind::Idea,
            "user".into(),
            "Relevant idea".into(),
            None,
        )
        .unwrap();
    signals
        .research(&signal.id, "Evidence".into(), 0.9, vec![], None)
        .unwrap();
    let app = app.with_signal(signals.engine());
    assert_eq!(app.status().unwrap()["pending_signals"], 1);
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
    app.add_chunk(draft()).unwrap();
    assert!(matches!(
        app.export(ExportRequest {
            format: ExportFormat::Json,
            target: ExportTarget::File
        }),
        Err(RoadmapError::WorkspaceUnavailable)
    ));
    let result = app
        .export(ExportRequest {
            format: ExportFormat::Markdown,
            target: ExportTarget::Inline,
        })
        .unwrap();
    assert!(
        matches!(result, ExportOutcome::Inline { roadmap, .. } if roadmap.contains("Original claim"))
    );
}

fn add_workstream_chunk(app: &RoadmapUseCases, id: &str, group: Option<&str>, status: ChunkStatus) {
    app.add_chunk(ChunkDraft {
        id: id.into(),
        title: format!("Work for {id}"),
        group: group.map(str::to_owned),
        status,
        ..draft()
    })
    .unwrap();
}

#[test]
fn focus_catalog_separates_closed_history_and_ungrouped_work_without_rewriting_records() {
    let app = application();
    add_workstream_chunk(&app, "finished", Some("Archive"), ChunkStatus::Done);
    add_workstream_chunk(&app, "abandoned", Some("Archive"), ChunkStatus::Obsoleted);
    add_workstream_chunk(&app, "feature", Some("Delivery"), ChunkStatus::Pending);
    add_workstream_chunk(&app, "loose", None, ChunkStatus::Backlog);
    add_workstream_chunk(&app, "loose-finished", None, ChunkStatus::Done);
    let before = app.engine().lock().unwrap().roadmap().clone();

    let read = app
        .focus_get("lane-a", None, WorkstreamView::Current)
        .unwrap();
    assert!(read.focus.is_none());
    assert_eq!(read.focus_state, FocusState::Unset);
    assert_eq!(read.frontier, WorkstreamInspection::Unset);
    assert_eq!(read.workstreams.current_names(), vec!["Delivery"]);
    assert_eq!(read.workstreams.history["Archive"].done, 1);
    assert_eq!(read.workstreams.history["Archive"].obsoleted, 1);
    assert_eq!(read.workstreams.ungrouped.counts.total, 1);
    assert_eq!(read.workstreams.ungrouped_history.done, 1);
    assert!(!read.persistent);
    assert!(
        !serde_json::to_string(&read)
            .unwrap()
            .contains("\"chunk_ids\":")
    );

    let proposed = app.propose_groups(4).unwrap();
    assert_eq!(proposed.authored_names, vec!["Archive", "Delivery"]);
    assert_eq!(proposed.placed, 0);
    let error = app
        .focus_set("lane-a", "Archive", FocusMode::Build)
        .unwrap_err();
    assert!(error.to_string().contains("no unfinished work"));
    assert!(error.to_string().contains("Delivery"));
    assert_eq!(app.engine().lock().unwrap().roadmap(), &before);
}

#[test]
fn obsoleting_the_last_open_chunk_keeps_exact_saved_focus_and_other_lanes() {
    let app = application();
    add_workstream_chunk(&app, "feature", Some("Delivery"), ChunkStatus::Pending);
    add_workstream_chunk(&app, "other", Some("Delivery tools"), ChunkStatus::Pending);
    let FocusChange::Set {
        focus: saved,
        frontier,
        ..
    } = app
        .focus_set("lane-a", "Delivery", FocusMode::Shape)
        .unwrap()
    else {
        panic!("focus set must return the selected frontier");
    };
    assert_eq!(frontier.group.as_deref(), Some("Delivery"));
    assert_eq!(frontier.activity.next_ready.as_deref(), Some("feature"));
    app.focus_set("lane-b", "Delivery tools", FocusMode::Listen)
        .unwrap();
    let other_lane = app
        .focus_get("lane-b", None, WorkstreamView::Current)
        .unwrap()
        .focus;
    app.obsolete_chunk("feature", "Superseded by completed delivery work".into())
        .unwrap();

    let read = app
        .focus_get("lane-a", None, WorkstreamView::Current)
        .unwrap();
    assert_eq!(read.focus, Some(saved.clone()));
    assert_eq!(
        read.focus_state,
        FocusState::Closed {
            group: "Delivery".into()
        }
    );
    assert!(
        !serde_json::to_string(&read)
            .unwrap()
            .contains("\"chunk_ids\":")
    );
    assert!(matches!(
        read.frontier,
        WorkstreamInspection::Closed { group, history }
            if group == "Delivery" && history.obsoleted == 1
    ));
    assert_eq!(read.workstreams.current_names(), vec!["Delivery tools"]);
    let history = app
        .focus_get("lane-a", None, WorkstreamView::History)
        .unwrap();
    assert_eq!(history.focus, Some(saved.clone()));
    assert_eq!(history.focus_state, read.focus_state);
    assert!(matches!(
        history.frontier,
        WorkstreamInspection::History { group, history }
            if group == "Delivery" && history.counts.obsoleted == 1
                && history.chunk_ids == ["feature"]
    ));

    let peek = app
        .focus_get("lane-a", Some("tools"), WorkstreamView::Current)
        .unwrap();
    assert_eq!(peek.focus, Some(saved.clone()));
    assert_eq!(
        peek.focus_state,
        FocusState::Closed {
            group: "Delivery".into()
        }
    );
    assert!(matches!(
        peek.frontier,
        WorkstreamInspection::Current { frontier }
            if frontier.group.as_deref() == Some("Delivery tools")
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
fn moving_the_last_chunk_reports_missing_focus_without_resolving_similar_current_names() {
    let app = application();
    add_workstream_chunk(&app, "feature", Some("Delivery"), ChunkStatus::Pending);
    app.focus_set("lane-a", "Delivery", FocusMode::Build)
        .unwrap();
    let saved = app
        .focus_get("lane-a", None, WorkstreamView::Current)
        .unwrap()
        .focus;
    app.set_group("feature", Some("Dispatch".into())).unwrap();
    add_workstream_chunk(&app, "tools", Some("Delivery tools"), ChunkStatus::Pending);
    add_workstream_chunk(
        &app,
        "reviews",
        Some("Delivery reviews"),
        ChunkStatus::Backlog,
    );
    let before = app.engine().lock().unwrap().roadmap().clone();

    let read = app
        .focus_get("lane-a", None, WorkstreamView::Current)
        .unwrap();
    assert_eq!(read.focus, saved);
    assert_eq!(
        read.focus_state,
        FocusState::Missing {
            group: "Delivery".into()
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
            if frontier.group.as_deref() == Some("Delivery tools")
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
    assert_eq!(app.engine().lock().unwrap().roadmap(), &before);
}

#[test]
fn focus_read_refreshes_catalog_saved_selection_and_frontier_from_one_store_observation() {
    use crate::infra::{Domain, Persistence, PersistenceConfig};

    let directory = tempfile::tempdir().unwrap();
    let config = PersistenceConfig::from_env()
        .with_data_dir(directory.path().to_owned())
        .enabled(true);
    let application = || {
        RoadmapUseCases::new(
            Arc::new(Mutex::new(
                RoadmapEngine::new("project".into())
                    .with_persistence(Persistence::new(&config, Domain::Roadmap)),
            )),
            ShipUseCases::new(Arc::new(Mutex::new(ShipEngine::new("project".into())))),
        )
    };
    let writer = application();
    add_workstream_chunk(&writer, "feature", Some("Delivery"), ChunkStatus::Pending);
    writer
        .focus_set("lane-a", "Delivery", FocusMode::Build)
        .unwrap();
    let reader = application();

    writer
        .obsolete_chunk("feature", "Finished elsewhere".into())
        .unwrap();
    add_workstream_chunk(
        &writer,
        "replacement",
        Some("Delivery tools"),
        ChunkStatus::Pending,
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
            group: "Delivery".into()
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

    let store = Persistence::new(&config, Domain::Roadmap).path_for("project");
    std::fs::write(store, b"unreadable roadmap").unwrap();
    assert!(matches!(
        reader.focus_get("lane-a", None, WorkstreamView::Current),
        Err(RoadmapError::Store(_))
    ));
}

#[tokio::test]
async fn mcp_history_peek_preserves_current_focus_and_refuses_closed_selection() {
    let app = application();
    add_workstream_chunk(&app, "finished", Some("Archive"), ChunkStatus::Done);
    add_workstream_chunk(&app, "feature", Some("Delivery"), ChunkStatus::Pending);
    add_workstream_chunk(&app, "earlier", Some("Delivery"), ChunkStatus::Done);
    app.focus_set("lane-a", "Delivery", FocusMode::Build)
        .unwrap();
    let before = app.engine().lock().unwrap().roadmap().clone();
    let service = RoadmapService::from_use_cases(app.clone());

    // Omitting the view returns active work and history summaries without IDs.
    let args: FocusGetArgs = serde_json::from_value(serde_json::json!({
        "lane": "lane-a",
    }))
    .unwrap();
    assert_eq!(args.view, WorkstreamView::Current);
    let current = service
        .roadmap_focus_get(Parameters(args))
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
    assert!(!current.to_string().contains("\"chunk_ids\":"));

    // History is an explicit query for either a mixed or a closed workstream.
    for (group, id) in [(None, "earlier"), (Some("Archive"), "finished")] {
        let read = service
            .roadmap_focus_get(Parameters(FocusGetArgs {
                lane: "lane-a".into(),
                group: group.map(str::to_owned),
                view: WorkstreamView::History,
            }))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(read["focus"]["group"], "Delivery");
        assert_eq!(
            read["focus_state"],
            serde_json::json!({"state":"current", "group":"Delivery"})
        );
        assert_eq!(read["frontier"]["state"], "history");
        assert_eq!(read["frontier"]["group"], group.unwrap_or("Delivery"));
        assert_eq!(read["frontier"]["history"]["done"], 1);
        assert_eq!(
            read["frontier"]["history"]["chunk_ids"],
            serde_json::json!([id])
        );
        assert!(read["workstreams"]["current"].get("Archive").is_none());
        assert_eq!(read["workstreams"]["history"]["Archive"]["done"], 1);
        assert_eq!(read["workstreams"]["history"]["Delivery"]["done"], 1);
        assert!(read.get("groups").is_none());
    }

    let closed = service
        .roadmap_focus_get(Parameters(FocusGetArgs {
            lane: "lane-a".into(),
            group: Some("Archive".into()),
            view: WorkstreamView::Current,
        }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(closed["frontier"]["state"], "closed");
    assert_eq!(closed["frontier"]["history"]["done"], 1);
    assert!(!closed.to_string().contains("\"chunk_ids\":"));
    assert!(
        serde_json::from_value::<FocusGetArgs>(serde_json::json!({
            "lane": "lane-a", "view": "all",
        }))
        .is_err()
    );

    let refused = service
        .roadmap_focus_set(Parameters(FocusSetArgs {
            lane: "lane-a".into(),
            group: "Archive".into(),
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
        .roadmap_propose_groups(Parameters(ProposeGroupsArgs { min_shared: 4 }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(
        proposal["authored_names"],
        serde_json::json!(["Archive", "Delivery"])
    );
    assert!(proposal.get("groups").is_none());
    assert_eq!(app.engine().lock().unwrap().roadmap(), &before);
}
