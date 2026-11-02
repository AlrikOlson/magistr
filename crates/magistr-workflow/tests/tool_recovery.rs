//! Recovery contracts over the real MCP wire, with an isolated in-memory engine.
use magistr_workflow::decision::{
    DecisionService, config::DecisionConfig, engine::core::ReasoningServer,
};
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
    let mut config = DecisionConfig::default();
    config.display.color_output = false;
    let service = DecisionService::new(ReasoningServer::new_for_project(
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
        .find(|t| t.name == "decision_record")
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
                    CallToolRequestParams::new("decision_record")
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
        assert!(owner.get_note(corrected + 1, false).unwrap().is_none());
        let response = client
            .peer()
            .call_tool(
                CallToolRequestParams::new("decision_record")
                    .with_arguments(valid().as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        assert_ne!(response.is_error, Some(true), "{response:?}");
        corrected += 1;
        assert_eq!(response.structured_content.unwrap()["note"], corrected);
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
async fn omitted_numbers_thread_continuation_and_merge_survive_server_restart() {
    use magistr_workflow::decision::domain::ThreadStatus;

    let directory = tempfile::tempdir().unwrap();
    let mut config = DecisionConfig::default();
    config.display.color_output = false;
    config.features.enable_logs = false;
    config.persistence.enabled = true;
    config.persistence.data_dir = directory.path().to_path_buf();
    let mut thread_id = String::new();
    let mut nested_id = String::new();

    for restarted in [false, true] {
        let service = DecisionService::new(ReasoningServer::new_for_project(
            config.clone(),
            "thread-wire-fixture".into(),
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
                    .any(|t| t.name == "decision_set_thread_status")
            );
            let record = catalog
                .tools
                .iter()
                .find(|t| t.name == "decision_record")
                .unwrap();
            assert!(
                !record.input_schema["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("note"))
            );

            assert_eq!(call!("decision_record", valid())["note"], 1);
            let mut thread = valid();
            thread["thread_from"] = json!(1);
            thread["thread_name"] = json!("Can retry limits be shared?");
            let opened = call!("decision_record", thread);
            assert_eq!(opened["note"], 2);
            thread_id = opened["thread"]["id"].as_str().unwrap().to_owned();

            let mut continuation = valid();
            continuation["thread_id"] = json!(thread_id);
            let continued = call!("decision_record", continuation);
            assert_eq!(continued["note"], 3);
            assert_eq!(continued["thread"]["from"], 1);
            assert_eq!(continued["thread"]["name"], "Can retry limits be shared?");

            let mut nested = valid();
            nested["thread_from"] = json!(3);
            nested["thread_name"] = json!("Do shared limits survive a restart?");
            let opened = call!("decision_record", nested);
            assert_eq!(opened["note"], 4);
            nested_id = opened["thread"]["id"].as_str().unwrap().to_owned();

            let mut revision = valid();
            revision["thread_id"] = json!(thread_id);
            revision["revises_note"] = json!(2);
            revision["revision_reason"] = json!("The restart check rejected shared limits.");
            revision["dependencies"] = json!([{"note":2,"relation":"refutes"}, 4]);
            assert_eq!(call!("decision_record", revision)["note"], 5);

            let mut synthesis = valid();
            synthesis["dependencies"] = json!([3, 5]);
            assert_eq!(call!("decision_record", synthesis)["note"], 6);
            for id in [&thread_id, &nested_id] {
                let merged = call!(
                    "decision_set_thread_status",
                    json!({
                        "thread_id": id, "status":"merged", "merged_into":6
                    })
                );
                assert_eq!(merged["merged_into"], 6);
            }
        }

        for (origin, id) in [(1, &thread_id), (3, &nested_id)] {
            let impact = call!("decision_impact", json!({"note":origin}));
            let threads = impact["threads_from"].as_array().unwrap();
            let thread = threads.iter().find(|thread| thread["id"] == *id).unwrap();
            assert_eq!(thread["status"], "merged");
            assert_eq!(thread["merged_into"], 6);
        }
        let continued = call!("decision_get", json!({"note":3}));
        assert_eq!(continued["thread_id"], thread_id);
        assert_eq!(continued["thread_from"], 1);
        assert_eq!(continued["thread_name"], "Can retry limits be shared?");
        let revised = call!("decision_get", json!({"note":2}));
        assert_eq!(revised["revised_by"], 5);
        let status = call!("decision_status", json!({}));
        assert_eq!(status["next_note_number"], 7);
        {
            let engine = owner.engine();
            let engine = engine.lock().unwrap();
            let thread = &engine.threads()[&thread_id];
            assert_eq!(thread.status, ThreadStatus::Merged);
            assert_eq!(thread.merged_into, Some(6));
            assert_eq!(
                thread.notes.iter().map(|s| s.note).collect::<Vec<_>>(),
                vec![2, 3, 5]
            );
            assert_eq!(thread.from_note, 1);
            assert_eq!(thread.depth, 1);
            let nested = &engine.threads()[&nested_id];
            assert_eq!(nested.status, ThreadStatus::Merged);
            assert_eq!(nested.merged_into, Some(6));
            assert_eq!(nested.from_note, 3);
            assert_eq!(nested.depth, 2);
            assert!(engine.note_by_number(6).unwrap().thread_id.is_none());
        }
        client.cancel().await.unwrap();
        server.await.unwrap();
    }
}

#[test]
fn every_advertised_required_field_has_actionable_admission() {
    use magistr_workflow::infra::tool_input::ToolInput;
    use magistr_workflow::{
        plan::{PlanEngine, PlanService},
        run::{RunService, engine::RunEngine},
        signal::{SignalEngine, SignalService},
    };
    let mut config = DecisionConfig::default();
    config.display.color_output = false;
    let tools = DecisionService::new(ReasoningServer::new_for_project(
        config,
        "schema-contract".into(),
    ))
    .list_tools_view()
    .into_iter()
    .chain(RunService::new(RunEngine::new("schema-contract".into())).list_tools_view())
    .chain(
        PlanService::new(
            PlanEngine::new("schema-contract".into()),
            RunService::new(RunEngine::new("schema-contract".into())).use_cases(),
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
    let legacy: magistr_workflow::decision::domain::Note =
        serde_json::from_value(json!({})).unwrap();
    assert!(legacy.thought.is_empty());
}
