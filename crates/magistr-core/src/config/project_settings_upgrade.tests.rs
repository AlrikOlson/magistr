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
