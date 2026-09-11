use super::*;
use crate::contracts::Recovery;

#[test]
fn explicit_identity_is_preserved_and_validates_only_for_its_admission() {
    let root = tempfile::tempdir().expect("workspace");
    let generation = WorkspaceGeneration::new(7);
    let session = WorkspaceSession::new(root.path().to_path_buf(), generation, " alice ".into())
        .expect("explicit admission");
    assert_eq!(session.id(), " alice ");
    assert_eq!(session.generation(), generation);
    assert_eq!(session.validate(root.path(), generation), Ok(()));
    assert_eq!(session.clone().validate(root.path(), generation), Ok(()));
}

#[test]
fn empty_blank_and_oversized_identities_are_refused_without_anonymous_fallback() {
    let root = tempfile::tempdir().expect("workspace");
    for id in [
        String::new(),
        " \t\n".into(),
        "\u{2003}".into(),
        "a".repeat(257),
        "\u{1f680}".repeat(129),
    ] {
        let error =
            WorkspaceSession::new(root.path().to_path_buf(), WorkspaceGeneration::new(1), id)
                .expect_err("invalid identity");
        assert_eq!(error.kind, ErrorKind::ConfigPolicyDenied);
        assert_eq!(error.recovery(), Recovery::Abort);
        assert!(error.message.contains("256 UTF-16 units"));
    }
    for id in ["a".repeat(256), "\u{1f680}".repeat(128)] {
        let session = WorkspaceSession::new(
            root.path().to_path_buf(),
            WorkspaceGeneration::new(1),
            id.clone(),
        )
        .expect("exact identity ceiling");
        assert_eq!(session.id(), id);
    }
}

#[test]
fn another_workspace_is_refused_even_when_generations_and_ids_match() {
    let first = tempfile::tempdir().expect("first workspace");
    let second = tempfile::tempdir().expect("second workspace");
    let generation = WorkspaceGeneration::new(7);
    let session = WorkspaceSession::new(first.path().to_path_buf(), generation, "alice".into())
        .expect("first workspace admission");
    let error = session
        .validate(second.path(), generation)
        .expect_err("a generation is scoped to its workspace");
    assert_eq!(error.kind, ErrorKind::EngineUnavailable);
    assert_eq!(error.recovery(), Recovery::Retry);
    assert!(error.message.contains("another workspace"));
}

#[test]
fn replacement_owner_refuses_old_handle_but_can_readmit_the_same_durable_identity() {
    let root = tempfile::tempdir().expect("workspace");
    let previous = WorkspaceGeneration::new(7);
    let current = WorkspaceGeneration::new(8);
    let stale = WorkspaceSession::new(root.path().to_path_buf(), previous, "alice".into())
        .expect("previous admission");
    let error = stale
        .validate(root.path(), current)
        .expect_err("old handle is fenced");
    assert_eq!(error.kind, ErrorKind::EngineUnavailable);
    assert_eq!(error.recovery(), Recovery::Retry);
    assert!(error.message.contains("generation 7"));
    assert!(error.message.contains("generation 8"));
    assert!(error.message.contains("re-admit"));

    let fresh = WorkspaceSession::new(root.path().to_path_buf(), current, stale.id().into())
        .expect("explicit re-admission");
    assert_eq!(fresh.id(), stale.id());
    assert_ne!(fresh.generation(), stale.generation());
    assert_eq!(fresh.validate(root.path(), current), Ok(()));
    assert_eq!(stale.validate(root.path(), previous), Ok(()));
}
