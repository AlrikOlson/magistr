use super::*;
use crate::config::inputs::ProjectInputs;
use crate::scaffold::FileAction;

#[test]
fn linked_working_copy_setup_writes_to_the_shared_project_owner() {
    let temp = tempfile::tempdir().unwrap();
    let main = temp.path().join("main");
    let linked = temp.path().join("linked");
    std::fs::create_dir(&main).unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .current_dir(&main)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "--initial-branch=main"]);
    std::fs::write(main.join("README.md"), "fixture").unwrap();
    git(&["add", "."]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-m",
        "fixture",
    ]);
    git(&["worktree", "add", "--detach", linked.to_str().unwrap()]);
    let setup = ProjectSetup::preview(&linked, &inputs(&linked)).unwrap();
    assert_eq!(setup.directory(), main.canonicalize().unwrap());
    let approved: Vec<_> = (0..setup.plan().files.len()).collect();
    assert!(setup.apply(&approved).unwrap().complete);
    assert!(main.join(".magistr/project.json").is_file());
    let folder = ProjectFolder::inspect(&linked).unwrap();
    assert!(folder.prepared);
    assert_eq!(folder.directory, linked.canonicalize().unwrap());
    assert!(!linked.join(".magistr/project.json").exists());
    assert!(!linked.join(".think-and-ship/project.json").exists());
    assert_eq!(
        crate::validate::load_project_config(&main).unwrap(),
        crate::validate::load_project_config(&linked).unwrap()
    );
}

fn inputs(root: &Path) -> ProjectInputs {
    ProjectInputs::from_lookup(root, |_| None)
}

#[test]
fn review_is_read_only_and_applies_the_exact_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let setup = ProjectSetup::preview(temp.path(), &inputs(temp.path())).unwrap();
    assert!(!temp.path().join(".magistr").exists());
    let approved: Vec<_> = setup
        .plan()
        .files
        .iter()
        .enumerate()
        .filter(|(_, file)| file.action.is_applied())
        .map(|(index, _)| index)
        .collect();
    let result = setup.apply(&approved).unwrap();
    assert!(result.complete, "{:?}", result.receipt);
    for file in &setup.plan().files {
        if matches!(file.action, FileAction::Create | FileAction::Overwrite) {
            assert_eq!(
                std::fs::read_to_string(setup.directory().join(&file.path)).unwrap(),
                file.new_content
            );
        }
    }
    assert!(crate::validate::load_project_config(temp.path()).is_ok());
}

#[test]
fn declined_and_partial_setup_never_report_ready() {
    let temp = tempfile::tempdir().unwrap();
    let setup = ProjectSetup::preview(temp.path(), &inputs(temp.path())).unwrap();
    let declined = setup.apply(&[]).unwrap();
    assert!(!declined.complete);
    assert!(!temp.path().join(".magistr").exists());
    let index = setup
        .plan()
        .files
        .iter()
        .position(|file| file.path == ".magistr/project.json")
        .unwrap();
    let partial = setup.apply(&[index]).unwrap();
    assert!(!partial.complete);
    assert_eq!(partial.receipt.applied_paths(), [".magistr/project.json"]);
}

#[test]
fn stale_preview_refuses_before_any_writes_and_invalid_selection_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let setup = ProjectSetup::preview(temp.path(), &inputs(temp.path())).unwrap();
    assert!(setup.apply(&[usize::MAX]).is_err());
    assert!(!temp.path().join(".magistr").exists());
    std::fs::write(temp.path().join("CLAUDE.md"), "Written after review").unwrap();
    let approved: Vec<_> = setup
        .plan()
        .files
        .iter()
        .enumerate()
        .filter(|(_, file)| file.action.is_applied())
        .map(|(index, _)| index)
        .collect();
    assert!(setup.apply(&approved).is_err());
    assert!(!temp.path().join(".magistr").exists());
    assert_eq!(
        std::fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap(),
        "Written after review"
    );
}

#[test]
fn refusal_and_a_write_failure_remain_partial() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join(".claude/rules/conventions.md")).unwrap();
    let setup = ProjectSetup::preview(temp.path(), &inputs(temp.path())).unwrap();
    assert!(
        setup
            .plan()
            .files
            .iter()
            .any(|file| matches!(file.action, FileAction::Refused(_)))
    );
    let approved: Vec<_> = setup
        .plan()
        .files
        .iter()
        .enumerate()
        .filter(|(_, file)| file.action.is_applied())
        .map(|(index, _)| index)
        .collect();
    let result = setup.apply(&approved).unwrap();
    assert!(!result.complete);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let another = tempfile::tempdir().unwrap();
        let parent = another.path().join(".claude");
        std::fs::create_dir(&parent).unwrap();
        let setup = ProjectSetup::preview(another.path(), &inputs(another.path())).unwrap();
        let approved: Vec<_> = setup
            .plan()
            .files
            .iter()
            .enumerate()
            .filter(|(_, file)| file.action.is_applied())
            .map(|(index, _)| index)
            .collect();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o500)).unwrap();
        let result = setup.apply(&approved);
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
        let result = result.unwrap();
        assert!(!result.complete);
        assert!(result.receipt.is_partial());
        assert!(result.receipt.failed().next().is_some());
    }
}

#[test]
fn existing_settings_identity_and_user_customizations_are_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let setup = ProjectSetup::preview(temp.path(), &inputs(temp.path())).unwrap();
    let all: Vec<_> = (0..setup.plan().files.len()).collect();
    assert!(setup.apply(&all).unwrap().complete);
    let settings = std::fs::read_to_string(temp.path().join(".magistr/project.json")).unwrap();
    let identity =
        std::fs::read_to_string(temp.path().join(".think-and-ship/project.json")).unwrap();
    std::fs::write(
        temp.path().join(".claude/rules/conventions.md"),
        "My conventions",
    )
    .unwrap();
    let setup = ProjectSetup::preview(temp.path(), &inputs(temp.path())).unwrap();
    let all: Vec<_> = (0..setup.plan().files.len()).collect();
    assert!(setup.apply(&all).unwrap().complete);
    assert_eq!(
        std::fs::read_to_string(temp.path().join(".magistr/project.json")).unwrap(),
        settings
    );
    assert_eq!(
        std::fs::read_to_string(temp.path().join(".think-and-ship/project.json")).unwrap(),
        identity
    );
    assert_eq!(
        std::fs::read_to_string(temp.path().join(".claude/rules/conventions.md")).unwrap(),
        "My conventions"
    );
}

#[test]
fn invalid_directories_and_current_settings_are_clear_errors() {
    let temp = tempfile::tempdir().unwrap();
    assert!(ProjectSetup::preview(&temp.path().join("missing"), &inputs(temp.path())).is_err());
    std::fs::create_dir(temp.path().join(".magistr")).unwrap();
    std::fs::write(temp.path().join(".magistr/project.json"), "invalid").unwrap();
    assert!(ProjectSetup::preview(temp.path(), &inputs(temp.path())).is_err());
}
