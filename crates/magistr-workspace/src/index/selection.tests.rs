use super::*;
use std::fs;
#[test]
fn live_ignore_validation_tracks_nested_rules_and_negation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join(".gitignore"), "*.rs\n!src/keep.rs\n").unwrap();
    fs::write(root.join("src/keep.rs"), "fn kept() {}").unwrap();
    fs::write(root.join("src/hide.rs"), "fn hidden() {}").unwrap();
    assert!(Selection::admission(&root)("src/keep.rs"));
    assert!(!Selection::admission(&root)("src/hide.rs"));
    fs::write(root.join("src/.ignore"), "keep.rs\n").unwrap();
    assert!(!Selection::admission(&root)("src/keep.rs"));
}
#[test]
fn bounded_selection_excludes_state_and_does_not_claim_full_walk() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let state = root.join(".workbench");
    fs::create_dir(&state).unwrap();
    fs::write(state.join("journal.db"), "durable").unwrap();
    for n in 0..10 {
        fs::write(root.join(format!("{n}.rs")), "fn source() {}").unwrap();
    }
    let selection = Selection::scan(&root, &state, 3, &CancellationToken::new()).unwrap();
    assert_eq!(selection.paths.len(), 3);
    assert!(selection.truncated);
    assert!(!selection.paths.iter().any(|p| p.contains("journal")));
}

#[test]
fn nested_anchored_rules_are_relative_to_the_ignore_file() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/.gitignore"), "/hidden.rs\n").unwrap();
    fs::write(root.join("src/hidden.rs"), "fn hidden() {}").unwrap();
    fs::write(root.join("hidden.rs"), "fn visible() {}").unwrap();
    assert!(!Selection::admission(&root)("src/hidden.rs"));
    assert!(Selection::admission(&root)("hidden.rs"));
    let scan = Selection::scan(
        &root,
        &root.join(".workbench"),
        100,
        &CancellationToken::new(),
    )
    .unwrap();
    assert!(!scan.paths.contains("src/hidden.rs"));
    assert!(scan.paths.contains("hidden.rs"));
}

#[test]
fn parent_ignore_rules_take_precedence_over_nested_gitignore() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join(".ignore"), "*.rs\n").unwrap();
    fs::write(root.join("src/.gitignore"), "!kept.rs\n").unwrap();
    fs::write(root.join("src/kept.rs"), "fn hidden() {}").unwrap();
    assert!(!Selection::admission(&root)("src/kept.rs"));
    let scan = Selection::scan(
        &root,
        &root.join(".workbench"),
        100,
        &CancellationToken::new(),
    )
    .unwrap();
    assert!(!scan.paths.contains("src/kept.rs"));
    fs::remove_file(root.join(".ignore")).unwrap();
    assert!(Selection::admission(&root)("src/kept.rs"));
}

#[test]
fn live_validation_inherits_parent_rules_and_reloads_them() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().canonicalize().unwrap();
    let root = parent.join("project");
    fs::create_dir(&root).unwrap();
    fs::write(parent.join(".ignore"), "private.rs\n").unwrap();
    fs::write(root.join("private.rs"), "fn hidden() {}").unwrap();
    assert!(!Selection::admission(&root)("private.rs"));
    fs::write(parent.join(".ignore"), "").unwrap();
    assert!(Selection::admission(&root)("private.rs"));
}
