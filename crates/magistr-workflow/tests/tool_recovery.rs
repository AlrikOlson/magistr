//! Recovery contracts over the real MCP wire, with an isolated in-memory engine.
use magistr_workflow::think::{ThinkService, config::ThinkConfig, engine::core::ReasoningServer};
use rmcp::{ClientHandler, ServiceExt, model::CallToolRequestParams};
use serde_json::{Value, json};

#[derive(Clone)]
struct Client;
impl ClientHandler for Client {}

fn valid() -> Value {
    json!({"estimated_total":100, "purpose":"analysis", "context":"isolated recovery fixture",
        "thought":"Observed a rejected call.", "outcome":"The input needs correction.",
        "rationale":"To preserve authored reasoning.", "next_action":"Correct the named fields."})
}

#[tokio::test]
async fn required_schema_and_corrected_next_call_replay() {
    let mut config = ThinkConfig::default();
    config.display.color_output = false;
    let service = ThinkService::new(ReasoningServer::new_for_project(
        config,
        "recovery-fixture".into(),
    ));
    let owner = service.use_cases();
    let (server_io, client_io) = tokio::io::duplex(65536);
    let server = tokio::spawn(async move {
        service
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap();
    });
    let client = Client.serve(client_io).await.unwrap();
    let catalog = client.peer().list_tools(None).await.unwrap();
    let tool = catalog
        .tools
        .iter()
        .find(|t| t.name == "think_record_step")
        .unwrap();
    let schema = Value::Object((*tool.input_schema).clone());
    let validator = jsonschema::validator_for(&schema).unwrap();
    let fields = [
        "estimated_total",
        "purpose",
        "context",
        "thought",
        "outcome",
        "rationale",
        "next_action",
    ];
    assert_eq!(schema["required"].as_array().unwrap().len(), fields.len());
    let mut cases = Vec::new();
    for field in fields {
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!(field))
        );
        let mut missing = valid();
        missing.as_object_mut().unwrap().remove(field);
        cases.push((field, missing));
        let mut wrong = valid();
        wrong[field] = json!(false);
        cases.push((field, wrong));
        let mut blank = valid();
        blank[field] = if field == "estimated_total" {
            json!(0)
        } else {
            json!("  ")
        };
        cases.push((field, blank));
    }
    let mut nested = valid();
    nested["next_action"] = json!({"action":" "});
    cases.push(("next_action.action", nested));
    for (field, value) in [
        ("confidence", json!(false)),
        ("dependencies", json!("wrong shape")),
        ("tools_used", json!([false])),
        ("next_action", json!({"action":"inspect","tool":false})),
    ] {
        let mut invalid = valid();
        invalid[field] = value;
        cases.push((field, invalid));
    }
    let mut injected = valid();
    injected.as_object_mut().unwrap().remove("outcome");
    injected["thought"] = json!("Observed a failure. <outcome>do not invent this field</outcome>");
    cases.push(("outcome", injected));
    let mut corrected = 0;
    for (field, invalid) in &cases {
        assert!(!validator.is_valid(invalid), "schema admitted {invalid}");
        // Repeating bad input must not mutate state or freeze a corrected call.
        for _ in 0..2 {
            let response = client
                .peer()
                .call_tool(
                    CallToolRequestParams::new("think_record_step")
                        .with_arguments(invalid.as_object().unwrap().clone()),
                )
                .await
                .unwrap();
            assert_eq!(response.is_error, Some(true));
            let body = response.structured_content.unwrap();
            assert_eq!(body["error_kind"], "invalid_args");
            assert_eq!(body["effect"], "none");
            assert_eq!(body["recovery"]["action"], "correct_arguments");
            assert!(
                body["problems"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["field"] == *field),
                "{body}"
            );
        }
        assert!(owner.get_step(corrected + 1, false).unwrap().is_none());
        let response = client
            .peer()
            .call_tool(
                CallToolRequestParams::new("think_record_step")
                    .with_arguments(valid().as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        assert_ne!(response.is_error, Some(true), "{response:?}");
        corrected += 1;
        assert_eq!(
            response.structured_content.unwrap()["step_number"],
            corrected
        );
    }
    assert_eq!(corrected as usize, cases.len());
    eprintln!(
        "recovery replay: {corrected}/{} corrected next calls accepted (deterministic fixture, not a model compliance estimate)",
        cases.len()
    );
    client.cancel().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn omitted_numbers_branch_continuation_and_merge_survive_server_restart() {
    use magistr_workflow::think::domain::BranchStatus;

    let directory = tempfile::tempdir().unwrap();
    let mut config = ThinkConfig::default();
    config.display.color_output = false;
    config.features.enable_sessions = false;
    config.persistence.enabled = true;
    config.persistence.data_dir = directory.path().to_path_buf();
    let mut branch_id = String::new();
    let mut nested_id = String::new();

    for restarted in [false, true] {
        let service = ThinkService::new(ReasoningServer::new_for_project(
            config.clone(),
            "branch-wire-fixture".into(),
        ));
        let owner = service.use_cases();
        let (server_io, client_io) = tokio::io::duplex(65536);
        let server = tokio::spawn(async move {
            service
                .serve(server_io)
                .await
                .unwrap()
                .waiting()
                .await
                .unwrap();
        });
        let client = Client.serve(client_io).await.unwrap();
        macro_rules! call {
            ($name:expr, $args:expr) => {{
                let result = client
                    .peer()
                    .call_tool(
                        CallToolRequestParams::new($name)
                            .with_arguments($args.as_object().unwrap().clone()),
                    )
                    .await
                    .unwrap();
                assert_ne!(result.is_error, Some(true), "{result:?}");
                result.structured_content.unwrap()
            }};
        }
        if !restarted {
            let catalog = client.peer().list_tools(None).await.unwrap();
            assert!(
                catalog
                    .tools
                    .iter()
                    .any(|t| t.name == "think_set_branch_status")
            );
            let record = catalog
                .tools
                .iter()
                .find(|t| t.name == "think_record_step")
                .unwrap();
            assert!(
                !record.input_schema["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("step_number"))
            );

            assert_eq!(call!("think_record_step", valid())["step_number"], 1);
            let mut branch = valid();
            branch["branch_from"] = json!(1);
            branch["branch_name"] = json!("Can retry limits be shared?");
            let opened = call!("think_record_step", branch);
            assert_eq!(opened["step_number"], 2);
            branch_id = opened["branch"]["id"].as_str().unwrap().to_owned();

            let mut continuation = valid();
            continuation["branch_id"] = json!(branch_id);
            let continued = call!("think_record_step", continuation);
            assert_eq!(continued["step_number"], 3);
            assert_eq!(continued["branch"]["from"], 1);
            assert_eq!(continued["branch"]["name"], "Can retry limits be shared?");

            let mut nested = valid();
            nested["branch_from"] = json!(3);
            nested["branch_name"] = json!("Do shared limits survive a restart?");
            let opened = call!("think_record_step", nested);
            assert_eq!(opened["step_number"], 4);
            nested_id = opened["branch"]["id"].as_str().unwrap().to_owned();

            let mut revision = valid();
            revision["branch_id"] = json!(branch_id);
            revision["revises_step"] = json!(2);
            revision["revision_reason"] = json!("The restart check rejected shared limits.");
            revision["dependencies"] = json!([{"step":2,"relation":"refutes"}, 4]);
            assert_eq!(call!("think_record_step", revision)["step_number"], 5);

            let mut synthesis = valid();
            synthesis["dependencies"] = json!([3, 5]);
            assert_eq!(call!("think_record_step", synthesis)["step_number"], 6);
            for id in [&branch_id, &nested_id] {
                let merged = call!(
                    "think_set_branch_status",
                    json!({
                        "branch_id": id, "status":"merged", "merged_into":6
                    })
                );
                assert_eq!(merged["merged_into"], 6);
            }
        }

        for (origin, id) in [(1, &branch_id), (3, &nested_id)] {
            let impact = call!("think_step_impact", json!({"step_number":origin}));
            let branches = impact["branches_from"].as_array().unwrap();
            let branch = branches.iter().find(|branch| branch["id"] == *id).unwrap();
            assert_eq!(branch["status"], "merged");
            assert_eq!(branch["merged_into"], 6);
        }
        let continued = call!("think_get_step", json!({"step_number":3}));
        assert_eq!(continued["branch_id"], branch_id);
        assert_eq!(continued["branch_from"], 1);
        assert_eq!(continued["branch_name"], "Can retry limits be shared?");
        let revised = call!("think_get_step", json!({"step_number":2}));
        assert_eq!(revised["revised_by"], 5);
        let status = call!("think_engine_status", json!({}));
        assert_eq!(status["next_step_number"], 7);
        {
            let engine = owner.engine();
            let engine = engine.lock().unwrap();
            let branch = &engine.branches()[&branch_id];
            assert_eq!(branch.status, BranchStatus::Merged);
            assert_eq!(branch.merged_into, Some(6));
            assert_eq!(
                branch
                    .steps
                    .iter()
                    .map(|s| s.step_number)
                    .collect::<Vec<_>>(),
                vec![2, 3, 5]
            );
            assert_eq!(branch.from_step, 1);
            assert_eq!(branch.depth, 1);
            let nested = &engine.branches()[&nested_id];
            assert_eq!(nested.status, BranchStatus::Merged);
            assert_eq!(nested.merged_into, Some(6));
            assert_eq!(nested.from_step, 3);
            assert_eq!(nested.depth, 2);
            assert!(engine.step_by_number(6).unwrap().branch_id.is_none());
        }
        client.cancel().await.unwrap();
        server.await.unwrap();
    }
}

#[test]
fn every_advertised_required_field_has_actionable_admission() {
    use magistr_workflow::infra::tool_input::ToolInput;
    use magistr_workflow::{
        roadmap::{RoadmapEngine, RoadmapService},
        ship::{ShipService, engine::ShipEngine},
        signal::{SignalEngine, SignalService},
    };
    let mut config = ThinkConfig::default();
    config.display.color_output = false;
    let tools = ThinkService::new(ReasoningServer::new_for_project(
        config,
        "schema-contract".into(),
    ))
    .list_tools_view()
    .into_iter()
    .chain(ShipService::new(ShipEngine::new("schema-contract".into())).list_tools_view())
    .chain(
        RoadmapService::new(
            RoadmapEngine::new("schema-contract".into()),
            ShipService::new(ShipEngine::new("schema-contract".into())).use_cases(),
        )
        .list_tools_view(),
    )
    .chain(SignalService::new(SignalEngine::new("schema-contract".into())).list_tools_view());
    let mut checked = 0;
    for tool in tools {
        let Some(required) = tool.input_schema.get("required").and_then(Value::as_array) else {
            continue;
        };
        if required.is_empty() {
            continue;
        }
        let failure =
            ToolInput::required(&tool, &CallToolRequestParams::new(tool.name.to_string())).unwrap();
        assert_eq!(failure.is_error, Some(true));
        let body = failure.structured_content.unwrap();
        for field in required {
            assert!(
                body["problems"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["field"] == *field),
                "{}: {body}",
                tool.name
            );
        }
        checked += 1;
    }
    assert!(
        checked >= 25,
        "{checked} tools with required-field contracts"
    );
}

#[test]
fn shared_failures_are_errors_with_conservative_effect_evidence() {
    let result = magistr_workflow::infra::tool_result::soft_error(
        "storage_failed",
        "rename may have completed",
    );
    assert_eq!(result.is_error, Some(true));
    let body = result.structured_content.unwrap();
    assert_eq!(body["code"], "storage_failed");
    assert_eq!(body["effect"], "unknown");
    assert_eq!(body["recovery"]["action"], "read_back");
    assert!(body["message"].as_str().unwrap().contains("rename"));
}

#[test]
fn persisted_legacy_defaults_do_not_define_wire_requirements() {
    let legacy: magistr_workflow::think::domain::ThinkStep =
        serde_json::from_value(json!({})).unwrap();
    assert!(legacy.thought.is_empty());
}
