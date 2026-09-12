use super::*;

#[test]
fn default_is_think_and_ship() {
    assert_eq!(RoadmapBackend::default(), RoadmapBackend::ThinkAndShip);
    assert!(RoadmapBackend::ThinkAndShip.is_default());
    assert!(!RoadmapBackend::Yaml.is_default());
    assert!(!RoadmapBackend::Yaml.is_native());
    assert!(RoadmapBackend::ThinkAndShip.is_native());
}

#[test]
fn wire_form_is_kebab_case() {
    assert_eq!(
        serde_json::to_string(&RoadmapBackend::ThinkAndShip).expect("ser"),
        "\"think-and-ship\""
    );
    let parsed: RoadmapBackend = serde_json::from_str("\"yaml\"").expect("de");
    assert_eq!(parsed, RoadmapBackend::Yaml);
    assert_eq!(RoadmapBackend::ThinkAndShip.as_str(), "think-and-ship");
    assert!(serde_json::from_str::<RoadmapBackend>("\"markdown\"").is_err());
}
