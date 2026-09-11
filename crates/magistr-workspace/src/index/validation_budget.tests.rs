use super::*;
#[test]
fn growth_since_indexing_is_charged_against_live_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let path = root.join("file.rs");
    std::fs::write(&path, "x").unwrap();
    let indexed = Observation::read(&root, "file.rs", 1024).unwrap().file;
    std::fs::write(&path, vec![b'x'; 1024]).unwrap();
    let mut budget = ValidationBudget::new(1500, 1024);
    assert!(!budget.validate(&root, &indexed));
    assert_eq!(budget.remaining, 476);
    assert!(!budget.validate(&root, &indexed));
    assert_eq!(budget.remaining, 0);
    assert!(!budget.validate(&root, &indexed));
}
#[test]
fn unchanged_small_files_charge_only_their_live_size() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::write(root.join("file.rs"), "abc").unwrap();
    let indexed = Observation::read(&root, "file.rs", 1024).unwrap().file;
    let mut budget = ValidationBudget::new(20, 1024);
    assert!(budget.validate(&root, &indexed));
    assert_eq!(budget.remaining, 17);
}
