//! Export must do the file write itself; document-sized tool results are opt-in.
use magistr_workflow::roadmap::{RoadmapEngine, RoadmapService};
use magistr_workflow::ship::{ShipService, ShipUseCases, engine::ShipEngine};
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, CallToolResult},
};
use serde_json::{Value, json};

async fn call_export(service: RoadmapService, arguments: Value) -> CallToolResult {
    let (server_tx, client_tx) = tokio::io::duplex(4096);
    let server = tokio::spawn(async move {
        let running = service.serve(server_tx).await.unwrap();
        let _ = running.waiting().await;
    });
    let client = ().serve(client_tx).await.unwrap();
    let mut request = CallToolRequestParams::new("roadmap_export");
    request.arguments = Some(arguments.as_object().unwrap().clone());
    let result = client.peer().call_tool(request).await.unwrap();
    let _ = client.cancel().await;
    server.abort();
    result
}

#[tokio::test]
async fn default_export_cannot_report_success_without_a_workspace() {
    let result = call_export(
        RoadmapService::new(RoadmapEngine::new("test".into()), completion()),
        json!({}),
    )
    .await;
    let receipt = result.structured_content.unwrap();
    assert_eq!(
        receipt["ok"], false,
        "file export needs an explicit workspace"
    );
    assert_eq!(receipt["error_kind"], "workspace_unavailable");
    assert!(receipt.get("roadmap").is_none());
}

/// The ship family every roadmap under test completes against; nothing
/// here completes a chunk, so it carries no evidence capability.
fn completion() -> ShipUseCases {
    ShipService::new(ShipEngine::new("export-test".into())).use_cases()
}

fn seeded_service(root: &std::path::Path) -> RoadmapService {
    let mut engine = RoadmapEngine::new("waterworks-export".into());
    engine
        .add_chunk(
            "sediment-survey".into(),
            "Sediment survey".into(),
            magistr_workflow::roadmap::domain::ChunkStatus::Pending,
            10,
            "Sample the northern reservoir — retain every measurement.\n".repeat(30_000),
            vec!["Review all readings".into()],
            vec![],
            false,
        )
        .unwrap();
    RoadmapService::new(engine, completion())
        .with_workspace_root(root)
        .unwrap()
}

fn assert_schema(service: &RoadmapService, receipt: &Value) {
    let tool = service
        .list_tools_view()
        .into_iter()
        .find(|tool| tool.name == "roadmap_export")
        .unwrap();
    let annotations = tool.annotations.unwrap();
    assert_eq!(annotations.read_only_hint, Some(false));
    assert_eq!(annotations.destructive_hint, Some(true));
    let schema = Value::Object((*tool.output_schema.unwrap()).clone());
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(receipt)
        .unwrap();
}

#[tokio::test]
async fn default_export_replaces_the_complete_view_and_returns_only_a_small_receipt() {
    let root = tempfile::tempdir().unwrap();
    let service = seeded_service(root.path());
    let destination = root.path().join("ROADMAP.md");
    std::fs::write(&destination, "STALE projection").unwrap();
    let expected = service.engine().lock().unwrap().export("markdown");
    assert!(
        expected.len() > 1_500_000,
        "fixture must exceed a normal tool response limit"
    );

    let response = call_export(service.clone(), json!({})).await;
    assert!(
        serde_json::to_vec(&response).unwrap().len() < 1024,
        "file export must stay bounded in both content and structuredContent"
    );
    let receipt = response.structured_content.unwrap();
    assert_eq!(receipt["written"], true);
    assert_eq!(receipt["format"], "markdown");
    assert_eq!(receipt["path"], destination.to_str().unwrap());
    assert_eq!(receipt["bytes"], expected.len());
    assert!(receipt.get("roadmap").is_none());
    assert_eq!(std::fs::read_to_string(&destination).unwrap(), expected);
    assert_schema(&service, &receipt);

    service
        .engine()
        .lock()
        .unwrap()
        .add_chunk(
            "flow-survey".into(),
            "Flow survey".into(),
            magistr_workflow::roadmap::domain::ChunkStatus::Pending,
            20,
            "The eastern main needs a flow reading".into(),
            vec![],
            vec![],
            false,
        )
        .unwrap();
    let expected = service.engine().lock().unwrap().export("markdown");
    let response = call_export(service.clone(), json!({})).await;
    assert_eq!(
        response.structured_content.unwrap()["bytes"],
        expected.len()
    );
    assert_eq!(std::fs::read_to_string(&destination).unwrap(), expected);
    assert_eq!(
        std::fs::read_dir(root.path()).unwrap().count(),
        1,
        "no temporary files remain"
    );
}

#[tokio::test]
async fn json_writes_its_own_file_and_inline_remains_an_explicit_read() {
    let root = tempfile::tempdir().unwrap();
    let service = RoadmapService::new(RoadmapEngine::new("export-test".into()), completion())
        .with_workspace_root(root.path())
        .unwrap();
    let expected = service.engine().lock().unwrap().export("json");
    let response = call_export(service.clone(), json!({"format":"json"})).await;
    let receipt = response.structured_content.unwrap();
    assert_eq!(receipt["written"], true);
    assert_eq!(receipt["bytes"], expected.len());
    assert_eq!(
        std::fs::read_to_string(root.path().join("ROADMAP.json")).unwrap(),
        expected
    );
    assert!(!root.path().join("ROADMAP.md").exists());
    assert_schema(&service, &receipt);

    for format in ["markdown", "json"] {
        let response =
            call_export(service.clone(), json!({"output":"inline", "format":format})).await;
        let receipt = response.structured_content.unwrap();
        assert_eq!(
            receipt["roadmap"],
            service.engine().lock().unwrap().export(format)
        );
        assert!(receipt.get("written").is_none());
        assert_schema(&service, &receipt);
    }
    assert!(!root.path().join("ROADMAP.md").exists());
    assert_eq!(
        std::fs::read_to_string(root.path().join("ROADMAP.json")).unwrap(),
        expected
    );
}

#[tokio::test]
async fn invalid_modes_and_formats_do_not_touch_the_existing_export() {
    let root = tempfile::tempdir().unwrap();
    let service = RoadmapService::new(RoadmapEngine::new("export-test".into()), completion())
        .with_workspace_root(root.path())
        .unwrap();
    let destination = root.path().join("ROADMAP.md");
    std::fs::write(&destination, "keep me").unwrap();
    for args in [
        json!({"format":"yaml"}),
        json!({"output":"../escape"}),
        json!({"format":null}),
        json!({"output":null}),
        json!({"format":[]}),
        json!({"output":false}),
        json!({"path":"../escape"}),
    ] {
        let response = call_export(service.clone(), args).await;
        let receipt = response.structured_content.unwrap();
        assert_eq!(receipt["ok"], false);
        assert_eq!(receipt["error_kind"], "invalid_args");
        assert_schema(&service, &receipt);
        assert_eq!(std::fs::read_to_string(&destination).unwrap(), "keep me");
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn write_failures_are_explicit_and_leave_no_temporary_files() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("ROADMAP.md");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("untouched"), "keep me").unwrap();
    let service = RoadmapService::new(RoadmapEngine::new("export-test".into()), completion())
        .with_workspace_root(root.path())
        .unwrap();
    let response = call_export(service.clone(), json!({})).await;
    assert_eq!(response.is_error, Some(true));
    let receipt = response.structured_content.unwrap();
    assert_eq!(receipt["ok"], false);
    assert_eq!(receipt["error_kind"], "export_failed");
    assert!(receipt.get("written").is_none());
    assert_schema(&service, &receipt);
    assert_eq!(
        std::fs::read_to_string(destination.join("untouched")).unwrap(),
        "keep me"
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_and_dangling_symlink_destinations_are_never_followed_or_replaced() {
    for exists in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("untouched");
        if exists {
            std::fs::write(&target, "outside workspace").unwrap();
        }
        let destination = root.path().join("ROADMAP.md");
        std::os::unix::fs::symlink(&target, &destination).unwrap();
        let service = RoadmapService::new(RoadmapEngine::new("export-test".into()), completion())
            .with_workspace_root(root.path())
            .unwrap();
        let response = call_export(service, json!({})).await;
        assert_eq!(
            response.structured_content.unwrap()["error_kind"],
            "export_failed"
        );
        assert_eq!(std::fs::read_link(&destination).unwrap(), target);
        if exists {
            assert_eq!(
                std::fs::read_to_string(&target).unwrap(),
                "outside workspace"
            );
        } else {
            assert!(!target.exists());
        }
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn an_empty_workspace_path_is_rejected_during_configuration() {
    let error = RoadmapService::new(RoadmapEngine::new("export-test".into()), completion())
        .with_workspace_root("")
        .err()
        .expect("an invalid root must fail before the service is used");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}

#[tokio::test]
async fn a_relative_workspace_path_is_bound_to_an_absolute_export_destination() {
    let cwd = std::env::current_dir().unwrap();
    let root = tempfile::tempdir_in(&cwd).unwrap();
    let relative_root = root.path().strip_prefix(&cwd).unwrap();
    assert!(relative_root.is_relative());
    let service = RoadmapService::new(RoadmapEngine::new("export-test".into()), completion())
        .with_workspace_root(relative_root)
        .unwrap();
    let receipt = call_export(service, json!({}))
        .await
        .structured_content
        .unwrap();
    let destination = root.path().join("ROADMAP.md");
    assert!(destination.is_absolute());
    assert_eq!(receipt["written"], true);
    assert_eq!(receipt["path"], destination.to_str().unwrap());
    assert!(destination.is_file());
}

#[test]
fn workspace_resolution_uses_project_marker_then_git_then_start_directory() {
    use magistr_workflow::roadmap::mcp::service::resolve_workspace_root;
    let root = tempfile::tempdir().unwrap();
    let deep = root.path().join("nested/src");
    std::fs::create_dir_all(&deep).unwrap();
    assert_eq!(
        resolve_workspace_root(&deep).unwrap(),
        deep.canonicalize().unwrap()
    );
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root.path())
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        resolve_workspace_root(&deep).unwrap(),
        root.path().canonicalize().unwrap()
    );
    let nested = root.path().join("nested");
    std::fs::create_dir(nested.join(".think-and-ship")).unwrap();
    std::fs::write(
        nested.join(".think-and-ship/project.json"),
        r#"{"id":"nested-project"}"#,
    )
    .unwrap();
    assert_eq!(
        resolve_workspace_root(&deep).unwrap(),
        nested.canonicalize().unwrap()
    );
}
