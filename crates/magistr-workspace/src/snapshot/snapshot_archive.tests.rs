use super::*;
use crate::snapshot::SelectionPolicy;
use std::fs;

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "pub fn unchanged() {}\n").unwrap();
    fs::write(root.path().join("README.md"), "before\n").unwrap();
    fs::write(root.path().join(".gitignore"), "ignored/\n").unwrap();
    root
}
fn capture(root: &Path, inputs: &[&str]) -> SourceSnapshot {
    SourceSnapshot::capture(
        root,
        SelectionPolicy::default().with_sources(inputs.iter().map(|path| (*path).into()).collect()),
    )
    .unwrap()
}

#[test]
fn retained_snapshot_survives_reopening_and_does_not_change_the_observed_tree() {
    let root = fixture();
    let before = capture(root.path(), &[]);
    before.retain().unwrap();
    let saved = SourceSnapshot::retained(root.path(), &before.revision())
        .unwrap()
        .unwrap();
    assert_eq!(saved, before);
    before.retain().unwrap();
    assert_eq!(capture(root.path(), &[]).manifest, before.manifest);
    assert!(
        SourceSnapshot::retained(root.path(), &"0".repeat(64))
            .unwrap()
            .is_none()
    );
    assert!(SourceSnapshot::retained(root.path(), "../outside").is_err());
}

#[test]
fn narrower_scope_reuses_equal_inputs_after_excluded_metadata_changes() {
    let root = fixture();
    let before = capture(root.path(), &[]);
    fs::write(root.path().join("README.md"), "after\n").unwrap();
    let after = capture(root.path(), &["src"]);
    assert_ne!(before.policy_digest, after.policy_digest);
    assert!(before.covers(&after));
    assert!(!after.covers(&before), "a narrow proof cannot widen itself");
}

#[test]
fn changed_added_removed_and_widened_inputs_never_receive_prior_coverage() {
    for change in ["changed", "added", "removed", "widened", "policy"] {
        let root = fixture();
        let before = capture(root.path(), &["src"]);
        match change {
            "changed" => fs::write(root.path().join("src/lib.rs"), "changed").unwrap(),
            "added" => fs::write(root.path().join("src/added.rs"), "added").unwrap(),
            "removed" => fs::remove_file(root.path().join("src/lib.rs")).unwrap(),
            _ => {}
        }
        let mut after = capture(
            root.path(),
            if change == "widened" { &[] } else { &["src"] },
        );
        if change == "policy" {
            after.policy.max_entries -= 1;
            after.policy_digest = after.policy.digest();
        }
        assert!(!before.covers(&after), "{change}");
    }
}

#[test]
fn corrupted_retained_manifest_is_refused_instead_of_relabelled() {
    let root = fixture();
    let before = capture(root.path(), &[]);
    before.retain().unwrap();
    let path = archive(root.path()).join(format!("{}.json", before.revision()));
    let mut forged = before.clone();
    forged.manifest.entries.get_mut("src/lib.rs").unwrap().mode ^= 1;
    fs::write(&path, serde_json::to_vec(&forged).unwrap()).unwrap();
    assert!(SourceSnapshot::retained(root.path(), &before.revision()).is_err());
    assert!(
        before.retain().is_err(),
        "retention never silently repairs forged evidence"
    );
}

#[test]
fn missing_inputs_and_file_modes_are_part_of_the_certified_selection() {
    let root = fixture();
    let before = capture(root.path(), &["src/missing.rs"]);
    fs::write(root.path().join("src/missing.rs"), "now present").unwrap();
    assert!(!before.covers(&capture(root.path(), &["src/missing.rs"])));
    let before = capture(root.path(), &["src"]);
    let mut changed = before.clone();
    changed.manifest.entries.get_mut("src/lib.rs").unwrap().mode ^= 1;
    assert!(!before.covers(&changed));
}

#[test]
fn runtime_owner_locks_never_enter_source_evidence_but_project_settings_do() {
    let root = fixture();
    fs::create_dir(root.path().join(".magistr")).unwrap();
    for name in [
        "workflow-owner.lock",
        "project-owner.lock",
        "working-copy.lock",
    ] {
        fs::write(root.path().join(".magistr").join(name), "live owner").unwrap();
    }
    fs::write(root.path().join(".magistr/project.json"), "{}").unwrap();
    let before = capture(root.path(), &[]);
    assert!(
        before
            .manifest
            .entries
            .contains_key(".magistr/project.json")
    );
    for name in [
        "workflow-owner.lock",
        "project-owner.lock",
        "working-copy.lock",
    ] {
        assert!(
            !before
                .manifest
                .entries
                .contains_key(&format!(".magistr/{name}"))
        );
        fs::write(root.path().join(".magistr").join(name), "another owner").unwrap();
        assert!(
            SourceSnapshot::capture(
                root.path(),
                SelectionPolicy::default().with_sources(vec![format!(".magistr/{name}")])
            )
            .is_err()
        );
    }
    assert_eq!(before.manifest, capture(root.path(), &[]).manifest);
}

#[test]
fn explicit_ignored_files_are_bound_and_excluded_directories_are_not_silently_empty() {
    let root = fixture();
    fs::create_dir(root.path().join("ignored")).unwrap();
    fs::write(root.path().join("ignored/facts.json"), "{}").unwrap();
    let broad = capture(root.path(), &[]);
    let before = capture(root.path(), &["ignored/facts.json"]);
    assert!(before.manifest.entries.contains_key("ignored/facts.json"));
    assert!(
        !broad.covers(&before),
        "an old ignored input was never certified"
    );
    assert!(
        SourceSnapshot::capture(
            root.path(),
            SelectionPolicy::default().with_sources(vec!["ignored".into()])
        )
        .is_err()
    );
    fs::write(root.path().join("ignored/facts.json"), "{\"changed\":true}").unwrap();
    assert!(!before.covers(&capture(root.path(), &["ignored/facts.json"])));
    fs::create_dir(root.path().join("empty")).unwrap();
    assert!(capture(root.path(), &["empty"]).covers(&capture(root.path(), &["empty"])));
}
