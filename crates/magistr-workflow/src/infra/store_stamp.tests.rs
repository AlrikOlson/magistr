use super::*;

#[test]
fn a_missing_file_has_no_stamp_and_a_rename_moves_it() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("store.json");
    assert_eq!(StoreStamp::of(&path).unwrap(), None);

    fs::write(&path, "{}").unwrap();
    let first = StoreStamp::of(&path).unwrap().expect("a file");
    assert_eq!(
        StoreStamp::of(&path).unwrap(),
        Some(first),
        "stable while untouched"
    );

    // The same bytes written the way every store is written — a fresh
    // temporary renamed over the file — is a different stamp even inside
    // one timestamp tick at the same length.
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, "{}").unwrap();
    fs::rename(&tmp_path, &path).unwrap();
    let second = StoreStamp::of(&path).unwrap().expect("a file");
    if cfg!(unix) {
        assert_ne!(first, second);
    }
}
