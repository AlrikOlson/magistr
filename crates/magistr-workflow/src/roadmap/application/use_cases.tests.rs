use super::*;

use rmcp::handler::server::wrapper::Parameters;

use crate::content::StructuredContent;
use crate::roadmap::RoadmapService;
use crate::roadmap::application::{
    BlockerDraft, ChunkPatch, ExportFormat, ExportOutcome, ExportRequest, ExportTarget,
};
use crate::roadmap::domain::{BlockerKind, FocusMode};
use crate::roadmap::mcp::handlers::UpdateChunkArgs;
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
    assert!(app.focus_get("lane-b", None).unwrap().focus.is_none());
    let before = app.focus_get("lane-a", None).unwrap().focus;
    assert!(
        app.focus_set("lane-a", "unknown", FocusMode::Shape)
            .is_err()
    );
    assert_eq!(app.focus_get("lane-a", None).unwrap().focus, before);
    assert!(app.focus_clear(" ").is_err());
    app.focus_clear("lane-a").unwrap();
    assert!(app.focus_get("lane-a", None).unwrap().focus.is_none());
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
