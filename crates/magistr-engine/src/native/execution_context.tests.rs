use super::*;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn repository() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("main");
    std::fs::create_dir(&root).expect("root");
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Test"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(root.join("file"), "original").expect("source");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    (temp, root.canonicalize().expect("canonical"))
}

#[test]
fn linked_worktrees_share_authority_but_not_execution_selection() {
    let (temp, main) = repository();
    let linked = temp.path().join("linked");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-b",
            "task",
            linked.to_str().expect("path"),
        ],
    );
    let config = super::super::NativeConfig::from_lookup(&main, |_| None)
        .with_data_dir(temp.path().join("store"));
    let linked_config = config.clone().with_project_root(&linked);
    assert_eq!(
        config.resolve_project_id(),
        linked_config.resolve_project_id()
    );
    let writer = super::super::ProjectAuthority::admit(&config, super::super::WriterRole::Host)
        .expect("writer")
        .open();
    let other = writer
        .for_working_copy(&linked, "default")
        .expect("linked context");
    writer
        .ship_set_objective_with_verification(
            "main".into(),
            vec![],
            vec![],
            "chunk:main".into(),
            magistr_workflow::ship::domain::objective::VerificationMode::Focused,
        )
        .expect("main objective");
    other
        .ship_set_objective_with_verification(
            "linked".into(),
            vec![],
            vec![],
            "chunk:linked".into(),
            magistr_workflow::ship::domain::objective::VerificationMode::Focused,
        )
        .expect("linked objective");
    assert_eq!(writer.ship_objective().expect("main").scope, "chunk:main");
    assert_eq!(
        other.ship_objective().expect("linked").scope,
        "chunk:linked"
    );
    let again = writer
        .for_working_copy(&linked, "default")
        .expect("same context");
    assert_eq!(
        again.ship_objective().expect("retained").scope,
        "chunk:linked"
    );
    let parallel = writer
        .for_working_copy(&linked, "second")
        .expect("second execution");
    assert!(parallel.ship_objective().is_none());
    assert_eq!(
        other.ship_objective().expect("unchanged").scope,
        "chunk:linked"
    );
    let reader = super::super::NativeReader::open(&linked_config);
    assert_eq!(
        reader.ship_objective().expect("persisted").scope,
        "chunk:linked"
    );
}

#[test]
fn a_copied_project_declaration_does_not_admit_an_unrelated_tree() {
    let (temp, main) = repository();
    let copy = temp.path().join("copy");
    std::fs::create_dir(&copy).expect("copy");
    let config = super::super::NativeConfig::from_lookup(&main, |_| None)
        .with_data_dir(temp.path().join("store"));
    let writer = super::super::ProjectAuthority::admit(&config, super::super::WriterRole::Host)
        .expect("writer")
        .open();
    assert!(writer.for_working_copy(&copy, "default").is_err());
    assert!(
        writer
            .for_working_copy(&temp.path().join("absent"), "default")
            .is_err()
    );
}
