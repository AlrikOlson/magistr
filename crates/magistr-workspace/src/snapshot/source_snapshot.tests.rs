use super::*;
use crate::snapshot::{ChangeKind, EntryKind, ManifestEntry, SourceChange};
use std::{fs, path::Path};

/// A workspace with every kind of input: text and binary source, a symlink,
/// ignore rules, ignored build output, ignored configuration that is an
/// explicit input, ignored state that is not, and owner/repository state.
fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    let root = dir.path();
    fs::create_dir_all(root.join(".git/objects")).expect("repository");
    fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").expect("head");
    fs::create_dir_all(root.join(".magistr/workspace")).expect("owner state");
    fs::write(root.join(".magistr/workspace/journal.db"), b"sqlite").expect("journal");
    fs::create_dir_all(root.join("src")).expect("src");
    fs::write(root.join("src/lib.rs"), "pub fn a() {}\n").expect("source");
    fs::create_dir_all(root.join("assets")).expect("assets");
    fs::write(root.join("assets/logo.bin"), [0_u8, 255, 1, 2]).expect("binary");
    fs::write(root.join(".gitignore"), "target/\n.magistr/\n").expect("ignore");
    fs::create_dir_all(root.join("target")).expect("target");
    fs::write(root.join("target/out"), "built").expect("output");
    fs::create_dir_all(root.join(".magistr")).expect("magistr");
    fs::write(root.join(".magistr/project.json"), "{\"gate\":1}").expect("config");
    fs::write(root.join(".magistr/state.json"), "{\"phase\":1}").expect("state");
    #[cfg(unix)]
    std::os::unix::fs::symlink("src/lib.rs", root.join("link")).expect("symlink");
    dir
}

fn entry<'a>(snapshot: &'a SourceSnapshot, path: &str) -> &'a ManifestEntry {
    snapshot
        .manifest
        .entries
        .get(path)
        .unwrap_or_else(|| panic!("{path} in manifest {:?}", snapshot.manifest.entries.keys()))
}

fn changes(snapshot: &SourceSnapshot, root: &Path) -> Vec<SourceChange> {
    match snapshot.validate(root).expect("validation") {
        SnapshotValidation::Changed(changes) => changes,
        SnapshotValidation::Unchanged => Vec::new(),
    }
}

fn change(path: &str, kind: ChangeKind) -> SourceChange {
    SourceChange {
        path: path.into(),
        kind,
    }
}

#[test]
fn the_manifest_selects_source_and_explicit_config_but_never_state_or_ignored_output() {
    let dir = workspace();
    let snapshot =
        SourceSnapshot::capture(dir.path(), SelectionPolicy::default()).expect("snapshot");
    assert_eq!(snapshot.scheme, SCHEME);
    assert_eq!(snapshot.consistency, Consistency::Optimistic);
    assert_eq!(
        snapshot.identity.root,
        dir.path().canonicalize().expect("canonical")
    );
    assert_eq!(snapshot.identity.generation, None);
    assert_eq!(snapshot.policy_digest, SelectionPolicy::default().digest());
    assert_eq!(entry(&snapshot, "src/lib.rs").kind, EntryKind::Text);
    assert_eq!(entry(&snapshot, "src/lib.rs").size, 14);
    assert_eq!(entry(&snapshot, "assets/logo.bin").kind, EntryKind::Binary);
    assert_eq!(entry(&snapshot, ".gitignore").kind, EntryKind::Text);
    assert_eq!(
        entry(&snapshot, ".magistr/project.json").kind,
        EntryKind::Text
    );
    assert_eq!(
        entry(&snapshot, ".magistr/workspace.toml"),
        &ManifestEntry::missing()
    );
    #[cfg(unix)]
    {
        let link = entry(&snapshot, "link");
        assert_eq!(link.kind, EntryKind::Symlink);
        assert_eq!(link.size, "src/lib.rs".len() as u64);
        assert_ne!(link.digest, entry(&snapshot, "src/lib.rs").digest);
    }
    for absent in [
        "target/out",
        ".magistr/state.json",
        ".magistr/workspace/journal.db",
        ".git/HEAD",
    ] {
        assert!(
            !snapshot.manifest.entries.contains_key(absent),
            "{absent} must not be selected"
        );
    }
    assert_eq!(
        entry(&snapshot, "src/lib.rs").digest,
        crate::file_content::FileContent::Text("pub fn a() {}\n".into()).content_hash()
    );
}

#[test]
fn the_revision_is_content_addressed_and_blind_to_mtime_and_the_clock() {
    let dir = workspace();
    let first = SourceSnapshot::capture(dir.path(), SelectionPolicy::default()).expect("first");
    let file = fs::File::options()
        .write(true)
        .open(dir.path().join("src/lib.rs"))
        .expect("open");
    file.set_modified(
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000),
    )
    .expect("touch");
    drop(file);
    let second = SourceSnapshot::capture(dir.path(), SelectionPolicy::default()).expect("second");
    assert_eq!(first.revision(), second.revision());
    assert_eq!(first.manifest, second.manifest);
    assert!(first.validate(dir.path()).expect("validate").is_unchanged());
    // Same size, same mtime, different bytes: still a change.
    fs::write(dir.path().join("src/lib.rs"), "pub fn b() {}\n").expect("rewrite");
    let file = fs::File::options()
        .write(true)
        .open(dir.path().join("src/lib.rs"))
        .expect("open");
    file.set_modified(
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000),
    )
    .expect("touch");
    drop(file);
    assert_eq!(
        changes(&first, dir.path()),
        vec![change("src/lib.rs", ChangeKind::Modified)]
    );
    let third = SourceSnapshot::capture(dir.path(), SelectionPolicy::default()).expect("third");
    assert_ne!(first.revision(), third.revision());
}

#[test]
fn every_kind_of_external_change_is_observed() {
    let dir = workspace();
    let root = dir.path();
    let snapshot = SourceSnapshot::capture(root, SelectionPolicy::default()).expect("snapshot");
    assert!(snapshot.validate(root).expect("fresh").is_unchanged());

    fs::remove_file(root.join("assets/logo.bin")).expect("delete");
    fs::write(root.join("NEW.md"), "untracked\n").expect("untracked");
    fs::write(root.join(".magistr/project.json"), "{\"gate\":2}").expect("config change");
    std::fs::create_dir_all(root.join(".magistr")).unwrap();
    fs::write(root.join(".magistr/workspace.toml"), "[commands]\n").expect("config appears");
    fs::write(root.join("src/lib.rs"), [0_u8, 1, 2]).expect("text becomes binary");
    fs::write(root.join("target/out"), "rebuilt").expect("ignored output changes");
    fs::write(root.join(".magistr/state.json"), "{\"phase\":2}").expect("state changes");
    let mut expected = vec![
        change("NEW.md", ChangeKind::Added),
        change(".magistr/project.json", ChangeKind::Modified),
        change("assets/logo.bin", ChangeKind::Removed),
        change("src/lib.rs", ChangeKind::Modified),
        change(".magistr/workspace.toml", ChangeKind::Modified),
    ];
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root.join(".gitignore"), fs::Permissions::from_mode(0o755))
            .expect("mode");
        fs::remove_file(root.join("link")).expect("unlink");
        std::os::unix::fs::symlink("assets/logo.bin", root.join("link")).expect("retarget");
        expected.push(change(".gitignore", ChangeKind::Modified));
        expected.push(change("link", ChangeKind::Modified));
    }
    expected.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(changes(&snapshot, root), expected);
    let later = SourceSnapshot::capture(root, SelectionPolicy::default()).expect("later");
    assert_eq!(entry(&later, "src/lib.rs").kind, EntryKind::Binary);
    assert_eq!(
        entry(&later, ".magistr/workspace.toml").kind,
        EntryKind::Text
    );
    #[cfg(unix)]
    assert_eq!(entry(&later, ".gitignore").mode, 0o755);
}

#[test]
fn another_workspace_never_substitutes_even_with_identical_bytes() {
    let first = workspace();
    let second = workspace();
    let snapshot =
        SourceSnapshot::capture(first.path(), SelectionPolicy::default()).expect("first");
    let twin = SourceSnapshot::capture(second.path(), SelectionPolicy::default()).expect("second");
    assert_eq!(snapshot.manifest.digest(), twin.manifest.digest());
    assert_ne!(snapshot.revision(), twin.revision());
    let refused = snapshot.validate(second.path()).expect_err("foreign root");
    assert!(
        matches!(refused, SnapshotError::ForeignWorkspace { .. }),
        "{refused}"
    );
    assert!(
        snapshot
            .validate(first.path())
            .expect("own root")
            .is_unchanged()
    );
    #[cfg(unix)]
    {
        let alias = tempfile::tempdir().expect("alias parent");
        let link = alias.path().join("alias");
        std::os::unix::fs::symlink(first.path(), &link).expect("alias");
        assert!(
            snapshot
                .validate(&link)
                .expect("alias resolves")
                .is_unchanged()
        );
    }
}

#[test]
fn explicit_artifacts_are_hashed_even_when_ignored_and_state_paths_are_refused() {
    let dir = workspace();
    let policy = SelectionPolicy::default().with_artifact("target/out");
    let snapshot = SourceSnapshot::capture(dir.path(), policy.clone()).expect("artifact");
    assert_eq!(entry(&snapshot, "target/out").kind, EntryKind::Text);
    assert_ne!(policy.digest(), SelectionPolicy::default().digest());
    fs::write(dir.path().join("target/out"), "rebuilt").expect("artifact changes");
    assert_eq!(
        changes(&snapshot, dir.path()),
        vec![change("target/out", ChangeKind::Modified)]
    );
    for refused in [
        "../escape",
        "/etc/passwd",
        ".magistr/workspace/key",
        ".git/HEAD",
        "",
    ] {
        let error = SourceSnapshot::capture(
            dir.path(),
            SelectionPolicy::default().with_artifact(refused),
        )
        .expect_err(refused);
        assert!(
            matches!(error, SnapshotError::InvalidInput { .. }),
            "{refused}: {error}"
        );
    }
    let directory =
        SourceSnapshot::capture(dir.path(), SelectionPolicy::default().with_artifact("src"))
            .expect_err("directory");
    assert!(matches!(directory, SnapshotError::InvalidInput { .. }));
}

#[test]
fn a_selection_past_the_ceiling_is_refused_not_truncated() {
    let dir = workspace();
    let policy = SelectionPolicy {
        max_entries: 2,
        ..SelectionPolicy::default()
    };
    let error = SourceSnapshot::capture(dir.path(), policy).expect_err("ceiling");
    assert!(
        matches!(error, SnapshotError::SelectionTruncated { limit: 2 }),
        "{error}"
    );
    let missing = SourceSnapshot::capture(&dir.path().join("absent"), SelectionPolicy::default())
        .expect_err("missing root");
    assert!(matches!(missing, SnapshotError::RootMissing(_)));
}

#[test]
fn the_durable_generation_is_observed_strictly_and_never_advanced() {
    let dir = workspace();
    let counter = dir.path().join(".magistr/workspace/server.generation");
    fs::write(&counter, format!("{:020}", 7)).expect("counter");
    let snapshot =
        SourceSnapshot::capture(dir.path(), SelectionPolicy::default()).expect("snapshot");
    assert_eq!(snapshot.identity.generation, Some(7));
    assert_eq!(
        fs::read_to_string(&counter).expect("unchanged"),
        format!("{:020}", 7)
    );
    let bare = SourceSnapshot::capture(dir.path(), SelectionPolicy::default()).expect("again");
    assert_eq!(bare.revision(), snapshot.revision());
    fs::write(&counter, "7").expect("malformed");
    let error =
        SourceSnapshot::capture(dir.path(), SelectionPolicy::default()).expect_err("malformed");
    assert!(matches!(error, SnapshotError::Generation(_)), "{error}");
    // A new owner generation is authority, not source: validation still compares bytes.
    fs::write(&counter, format!("{:020}", 8)).expect("advanced");
    assert!(
        snapshot
            .validate(dir.path())
            .expect("bytes unchanged")
            .is_unchanged()
    );
    let identity = WorkspaceIdentity::observe(dir.path()).expect("identity");
    assert_eq!(identity.generation, Some(8));
    assert_ne!(identity.digest(), snapshot.identity.digest());
    assert!(identity.same_workspace(&snapshot.identity));
}

#[test]
fn an_optimistic_snapshot_round_trips_and_refuses_immutable_certification() {
    let dir = workspace();
    let snapshot =
        SourceSnapshot::capture(dir.path(), SelectionPolicy::default()).expect("snapshot");
    let json = serde_json::to_string(&snapshot).expect("serialize");
    let parsed: SourceSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, snapshot);
    assert_eq!(parsed.revision(), snapshot.revision());
    assert!(
        parsed
            .validate(dir.path())
            .expect("validate a parsed snapshot")
            .is_unchanged()
    );
    let refused = parsed.certify_immutable().expect_err("optimistic");
    assert!(
        matches!(refused, SnapshotError::UnsupportedCertification(_)),
        "{refused}"
    );
}

#[test]
fn scoped_observations_track_membership_ignore_rules_and_missing_roots() {
    let dir = workspace();
    let policy = SelectionPolicy::default().with_sources(vec!["src".into(), "future".into()]);
    let snapshot = SourceSnapshot::capture(dir.path(), policy).unwrap();
    assert!(!snapshot.manifest.entries.contains_key("assets/logo.bin"));
    assert!(snapshot.manifest.entries.contains_key(".gitignore"));
    assert_eq!(entry(&snapshot, "future"), &ManifestEntry::missing());
    fs::write(dir.path().join("assets/logo.bin"), b"irrelevant").unwrap();
    assert!(snapshot.validate(dir.path()).unwrap().is_unchanged());
    fs::write(dir.path().join("src/new.rs"), "pub fn new() {}").unwrap();
    assert!(
        changes(&snapshot, dir.path())
            .iter()
            .any(|change| change.path == "src/new.rs")
    );
    fs::create_dir(dir.path().join("future")).unwrap();
    fs::write(dir.path().join("future/input"), "new dependency").unwrap();
    assert!(
        changes(&snapshot, dir.path())
            .iter()
            .any(|change| change.path == "future/input")
    );
    fs::write(dir.path().join(".gitignore"), "src/new.rs\n").unwrap();
    assert!(
        changes(&snapshot, dir.path())
            .iter()
            .any(|change| change.path == ".gitignore")
    );
    for invalid in [
        "..",
        "../outside",
        "/absolute",
        ".magistr/workspace",
        ".git",
    ] {
        assert!(
            SourceSnapshot::capture(
                dir.path(),
                SelectionPolicy::default().with_sources(vec![invalid.into()])
            )
            .is_err()
        );
    }
}
