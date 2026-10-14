use super::*;

#[test]
fn an_empty_history_read_creates_no_folder_or_database() {
    let folder = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(folder.path(), |_| None)
        .with_data_dir(folder.path().join("history"));
    assert!(saved_conversations(&inputs).unwrap().is_empty());
    assert!(!folder.path().join("history").exists());
}
#[test]
fn unreadable_history_is_an_error_and_is_not_replaced() {
    let folder = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(folder.path(), |_| None).with_data_dir(folder.path());
    let path = storage::sessions_db_path(&inputs).unwrap();
    std::fs::write(&path, "broken").unwrap();
    assert!(saved_conversations(&inputs).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "broken");
}
