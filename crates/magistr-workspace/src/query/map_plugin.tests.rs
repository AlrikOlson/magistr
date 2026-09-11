use super::*;

fn state_with(inventory: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("state");
    std::fs::write(dir.path().join("plugins.json"), inventory).expect("inventory");
    dir
}

#[test]
fn a_missing_inventory_is_empty_and_silent() {
    let dir = tempfile::tempdir().expect("state");
    assert_eq!(MapPlugin::inventory(dir.path()), (Vec::new(), Vec::new()));
}

#[test]
fn trust_follows_the_retained_fail_closed_rule() {
    let dir = state_with(
        r#"{"plugins": [
            {"name": "acme.blessed", "version": "1.0.0", "source": "registry", "tier": "blessed"},
            {"name": "acme.signed", "source": "registry"},
            {"name": "acme.odd", "source": "registry", "tier": "bogus"},
            {"name": "local.hack", "source": "sideload", "tier": "blessed"},
            {"name": "local.other", "source": "mirror"}
        ]}"#,
    );
    let (plugins, notes) = MapPlugin::inventory(dir.path());
    let trust: Vec<(&str, &str, &str)> = plugins
        .iter()
        .map(|p| (p.name.as_str(), p.source.as_str(), p.trust.as_str()))
        .collect();
    assert_eq!(
        trust,
        vec![
            ("acme.blessed", "registry", "blessed"),
            ("acme.signed", "registry", "verified"),
            ("acme.odd", "registry", "verified"),
            ("local.hack", "sideload", "untrusted"),
            ("local.other", "sideload", "untrusted"),
        ]
    );
    assert_eq!(plugins[0].version.as_deref(), Some("1.0.0"));
    assert_eq!(
        notes,
        vec![
            "2 sideloaded plugin(s) are untrusted: local.hack, local.other (SDD §8.4 — tightest capability defaults)"
        ]
    );
    let wire = serde_json::to_value(&plugins[1]).expect("json");
    assert_eq!(
        wire,
        serde_json::json!({"name": "acme.signed", "source": "registry", "trust": "verified"})
    );
}

#[test]
fn a_malformed_inventory_is_a_note_not_a_failure() {
    for inventory in [
        "not json",
        r#"{"plugins": {}}"#,
        r#"{"plugins": [{"source": "registry"}]}"#,
    ] {
        let dir = state_with(inventory);
        let (plugins, notes) = MapPlugin::inventory(dir.path());
        assert!(plugins.is_empty());
        assert_eq!(
            notes,
            vec!["plugin inventory unreadable; no plugins are shown"]
        );
    }
}
