use super::*;

use magistr_engine::policy::{PolicyCondition, PolicyEffect, PolicyRule};

#[test]
fn an_absent_file_is_the_empty_charter_and_a_save_round_trips() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = CharterStore::at(dir.path());
    assert_eq!(
        store.path(),
        dir.path().join(".magistr").join("charter.json")
    );
    assert_eq!(store.load().expect("absent loads"), Charter::default());

    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        "delegate:edit:src",
        PolicyCondition::tool("Edit"),
        PolicyEffect::Allow,
    ));
    charter.record(
        "edit:src",
        "Edit \u{b7} src",
        &PolicyCondition::tool("Edit"),
        true,
    );
    store.save(&charter).expect("save");
    assert_eq!(store.load().expect("load"), charter);
}

#[test]
fn a_corrupt_file_is_an_error_not_a_fresh_start() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = CharterStore::at(dir.path());
    std::fs::create_dir_all(store.path().parent().expect("parent")).expect("mkdir");
    std::fs::write(store.path(), "{ not json").expect("write");
    assert!(store.load().is_err());
}
