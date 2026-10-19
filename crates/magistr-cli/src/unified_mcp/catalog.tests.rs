use super::*;

fn fixture() -> Catalog {
    let tools = DIRECT
        .iter()
        .map(|(name, _, _)| {
            schema_tool(
                name,
                "native",
                json!({"type":"object","properties":{"native":{"type":"string"}}}),
            )
        })
        .collect();
    Catalog::new(
        tools,
        vec![schema_tool("undo", "journal", json!({"type":"object"}))],
    )
    .unwrap()
}

#[test]
fn direct_discovery_and_dispatch_share_native_schemas() {
    let catalog = fixture();
    for tool in catalog
        .direct_tools()
        .into_iter()
        .filter(|t| t.name != "discover" && t.name != "execute")
    {
        let discovered = catalog.discover(&json!({"operation":tool.name})).unwrap();
        assert_eq!(
            discovered["operation"]["inputSchema"],
            serde_json::to_value(&tool.input_schema).unwrap()
        );
        assert!(
            !catalog
                .operation(&tool.name)
                .unwrap()
                .native_name
                .is_empty()
        );
    }
    assert!(!catalog.operation("undo").unwrap().direct);
    assert!(catalog.operation("ship_plan").is_err());
    assert_eq!(
        catalog.operation("plan_work").unwrap().native_name,
        "ship_plan"
    );
}

#[test]
fn discovery_reports_pagination_and_missing_operations() {
    let catalog = fixture();
    let page = catalog.discover(&json!({"limit":2})).unwrap();
    assert_eq!(page["operations"].as_array().unwrap().len(), 2);
    assert_eq!(page["truncated"], true);
    assert_eq!(page["nextOffset"], 2);
    assert!(catalog.discover(&json!({"operation":"missing"})).is_err());
}

#[test]
fn duplicate_or_missing_native_operations_refuse_startup() {
    assert!(Catalog::new(Vec::new(), Vec::new()).is_err());
    let tools = DIRECT
        .iter()
        .map(|(name, _, _)| schema_tool(name, "native", json!({})))
        .collect();
    assert!(Catalog::new(tools, vec![schema_tool("search", "collision", json!({}))]).is_err());
}

#[test]
fn malformed_discovery_arguments_are_rejected_instead_of_silently_defaulting() {
    let catalog = fixture();
    for args in [
        json!({"limit":0}),
        json!({"limit":51}),
        json!({"offset":-1}),
        json!({"query":false}),
        json!({"extra":true}),
    ] {
        assert!(catalog.discover(&args).is_err(), "{args}");
    }
}

#[test]
fn discovery_supports_batch_schemas_and_any_word_search() {
    let catalog = fixture();
    let batch = catalog
        .discover(&json!({"operations":["read","undo"]}))
        .unwrap();
    assert_eq!(batch["operations"].as_array().unwrap().len(), 2);
    let found = catalog.discover(&json!({"query":"undo read"})).unwrap();
    assert!(found["total"].as_u64().unwrap() >= 2);
    assert!(catalog.discover(&json!({"operations":[]})).is_err());
    assert!(
        catalog
            .discover(&json!({"operation":"read","query":"read"}))
            .is_err()
    );
}
