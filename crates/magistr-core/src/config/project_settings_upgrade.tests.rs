use super::*;

/// This project's own settings, written the way a project saved them before
/// the plan words: `extra` replaces the plan keys.
fn settings(extra: &[(&str, serde_json::Value)]) -> String {
    let mut document: serde_json::Value =
        serde_json::from_str(include_str!("../../../../.magistr/project.json")).unwrap();
    let object = document.as_object_mut().unwrap();
    object.remove("plan_backend");
    object.remove("plan_exports");
    for (key, value) in extra {
        object.insert((*key).to_owned(), value.clone());
    }
    document.to_string()
}

#[test]
fn plan_settings_saved_before_the_plan_words_take_the_plan_keys() {
    let before = settings(&[
        ("roadmap_backend", "think-and-ship".into()),
        ("roadmap_exports", false.into()),
    ]);
    let after: serde_json::Value =
        serde_json::from_str(&ProjectSettingsUpgrade::prepare(&before).unwrap().unwrap()).unwrap();
    assert_eq!(after["plan_backend"], "native");
    assert_eq!(after["plan_exports"], false);
    assert!(after.get("roadmap_backend").is_none() && after.get("roadmap_exports").is_none());
    let again = serde_json::to_string(&after).unwrap();
    assert_eq!(ProjectSettingsUpgrade::prepare(&again).unwrap(), None);
}

#[test]
fn settings_holding_both_spellings_are_refused() {
    let both = settings(&[
        ("roadmap_exports", false.into()),
        ("plan_exports", true.into()),
    ]);
    assert!(ProjectSettingsUpgrade::prepare(&both).is_err());
}

#[test]
fn current_settings_are_unchanged() {
    let current = settings(&[("plan_backend", "native".into())]);
    assert_eq!(ProjectSettingsUpgrade::prepare(&current).unwrap(), None);
}

/// This project's own settings as they were saved before the check words.
const BEFORE_CHECK_WORDS: &str = include_str!("fixtures/before-check-words-project.json");

#[test]
fn settings_saved_before_the_check_words_take_the_check_keys() {
    let before: serde_json::Value = serde_json::from_str(BEFORE_CHECK_WORDS).unwrap();
    let upgraded = ProjectSettingsUpgrade::prepare(BEFORE_CHECK_WORDS)
        .unwrap()
        .expect("a conversion");
    let after: serde_json::Value = serde_json::from_str(&upgraded).unwrap();
    assert_eq!(after["checks"], before["quality_gate"]);
    assert_eq!(after["custom_checks"], before["custom_gates"]);
    assert_eq!(
        after["verification"]["checks"],
        before["verification"]["gates"]
    );
    for old in ["quality_gate", "custom_gates"] {
        assert!(after.get(old).is_none(), "{old} remains");
    }
    assert!(after["verification"].get("gates").is_none());
    assert_eq!(ProjectSettingsUpgrade::prepare(&upgraded).unwrap(), None);
}

#[test]
fn archetypes_guards_and_hooks_take_the_check_words() {
    let mut before: serde_json::Value = serde_json::from_str(BEFORE_CHECK_WORDS).unwrap();
    let standard = before["quality_gate"].clone();
    before["guards"] = serde_json::json!({"quality_gate": false});
    before["archetypes"] = serde_json::json!([{
        "id": "api",
        "name": "API",
        "language": "rust",
        "root": "api",
        "role": "service",
        "quality_gate": standard,
        "format": before["format"].clone(),
        "testing": before["testing"].clone(),
    }]);
    before["lifecycle_hooks"] = serde_json::json!([
        {"event": {"type": "on_gate_fail"}, "tool": "mcp__magistr__magistr_gate", "arguments": {"gate": "test"}},
        {"event": {"type": "on_gate_pass"}, "command": "echo green"},
        {"event": {"type": "on_iteration_end"}, "tool": "not_magistr_gate"},
    ]);
    let after: serde_json::Value = serde_json::from_str(
        &ProjectSettingsUpgrade::prepare(&before.to_string())
            .unwrap()
            .expect("a conversion"),
    )
    .unwrap();
    assert_eq!(after["guards"], serde_json::json!({"checks": false}));
    assert_eq!(after["archetypes"][0]["checks"], before["quality_gate"]);
    assert!(after["archetypes"][0].get("quality_gate").is_none());
    let hooks = &after["lifecycle_hooks"];
    assert_eq!(hooks[0]["event"]["type"], "on_check_fail");
    assert_eq!(hooks[0]["tool"], "mcp__magistr__magistr_check");
    assert_eq!(hooks[0]["arguments"], serde_json::json!({"check": "test"}));
    assert_eq!(hooks[1]["event"]["type"], "on_check_pass");
    assert_eq!(
        hooks[2]["tool"], "not_magistr_gate",
        "another tool keeps its name"
    );
}

#[test]
fn settings_holding_both_check_spellings_are_refused() {
    let mut both: serde_json::Value = serde_json::from_str(BEFORE_CHECK_WORDS).unwrap();
    both["checks"] = both["quality_gate"].clone();
    assert!(ProjectSettingsUpgrade::prepare(&both.to_string()).is_err());
}
