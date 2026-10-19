use super::*;

#[test]
fn default_is_workflow() {
    assert_eq!(RoadmapBackend::default(), RoadmapBackend::Native);
    assert!(RoadmapBackend::Native.is_default());
    assert!(!RoadmapBackend::Yaml.is_default());
    assert!(!RoadmapBackend::Yaml.is_native());
    assert!(RoadmapBackend::Native.is_native());
}

#[test]
fn wire_form_is_kebab_case() {
    assert_eq!(
        serde_json::to_string(&RoadmapBackend::Native).expect("ser"),
        "\"native\""
    );
    let parsed: RoadmapBackend = serde_json::from_str("\"yaml\"").expect("de");
    assert_eq!(parsed, RoadmapBackend::Yaml);
    assert_eq!(RoadmapBackend::Native.as_str(), "native");
    assert!(serde_json::from_str::<RoadmapBackend>("\"markdown\"").is_err());
}
