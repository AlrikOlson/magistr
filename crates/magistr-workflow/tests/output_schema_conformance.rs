//! What a tool returns validates against the `outputSchema` it advertises.
//!
//! A client that validates `structuredContent` (Claude Code does) replaces a
//! result that breaks its schema with a schema error, so a task that started
//! reads as a call that failed. The working path is held to the schema here,
//! over the real wire.
use magistr_workflow::mcp::UnifiedService;
use magistr_workflow::roadmap::{RoadmapService, engine::RoadmapEngine};
use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::ship::mcp::ShipService;
use magistr_workflow::signal::{SignalService, engine::SignalEngine};
use magistr_workflow::think::{ThinkService, config::ThinkConfig, engine::core::ReasoningServer};
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde_json::{Value, json};

#[derive(Clone)]
struct Client;
impl ClientHandler for Client {}

type Session = rmcp::service::RunningService<RoleClient, Client>;

async fn session() -> Session {
    let mut config = ThinkConfig::default();
    config.display.color_output = false;
    let think = ThinkService::new(ReasoningServer::new(config));
    let ship = ShipService::new(ShipEngine::new("schema-conformance".into()));
    let roadmap = RoadmapService::new(
        RoadmapEngine::new("schema-conformance".into()),
        ship.use_cases(),
    );
    let signal = SignalService::new(SignalEngine::new("schema-conformance".into()));
    let server = UnifiedService::new(think, ship, roadmap, signal);
    let (server_io, client_io) = tokio::io::duplex(1 << 16);
    tokio::spawn(async move {
        let running = server.serve(server_io).await.expect("server.serve");
        let _ = running.waiting().await;
    });
    Client.serve(client_io).await.expect("client.serve")
}

async fn catalog(session: &Session) -> Vec<Tool> {
    session
        .peer()
        .list_all_tools()
        .await
        .expect("the tool catalog")
}

async fn call(session: &Session, name: &'static str, arguments: Value) -> CallToolResult {
    let mut request = CallToolRequestParams::new(name);
    request.arguments = arguments.as_object().cloned();
    session
        .peer()
        .call_tool(request)
        .await
        .unwrap_or_else(|error| panic!("{name} answers: {error}"))
}

/// Every way `body` breaks the schema `name` advertises.
fn violations(catalog: &[Tool], name: &str, body: &Value) -> Vec<String> {
    let tool = catalog
        .iter()
        .find(|tool| tool.name == name)
        .unwrap_or_else(|| panic!("{name} is served"));
    let schema = Value::Object(
        tool.output_schema
            .as_deref()
            .unwrap_or_else(|| panic!("{name} advertises an outputSchema"))
            .clone(),
    );
    jsonschema::validator_for(&schema)
        .unwrap_or_else(|error| panic!("{name}'s outputSchema compiles: {error}"))
        .iter_errors(body)
        .map(|error| error.to_string())
        .collect()
}

fn assert_conforms(catalog: &[Tool], name: &str, result: &CallToolResult) {
    let body = result
        .structured_content
        .as_ref()
        .unwrap_or_else(|| panic!("{name} returns structuredContent"));
    let violations = violations(catalog, name, body);
    assert!(
        violations.is_empty(),
        "{name} broke its own outputSchema: {violations:?}\n{body}"
    );
}

#[tokio::test]
async fn a_task_that_starts_blocks_and_completes_is_returned_as_its_schema_says() {
    let session = session().await;
    let catalog = catalog(&session).await;
    call(
        &session,
        "ship_set_objective",
        json!({"description": "conformance"}),
    )
    .await;
    call(
        &session,
        "ship_plan",
        json!({"action": "add", "task_id": "t1", "title": "one"}),
    )
    .await;

    for (name, arguments) in [
        ("ship_start", json!({"task_id": "t1"})),
        ("ship_block", json!({"task_id": "t1", "reason": "waiting"})),
        ("ship_complete", json!({"task_id": "t1"})),
    ] {
        let result = call(&session, name, arguments).await;
        assert_ne!(result.is_error, Some(true), "{name} succeeds: {result:?}");
        assert_conforms(&catalog, name, &result);
    }
}
