use super::*;

#[tokio::test]
async fn a_refusal_routes_nothing_says_why_and_releases_nothing() {
    let held = WorkspaceAdmission::Refused(SourceRefusal::OwnerElsewhere);
    assert!(held.hosted().is_none());
    assert!(matches!(
        held.source_route(),
        SourceRoute::Refused(SourceRefusal::OwnerElsewhere)
    ));
    assert!(
        held.report().contains(
            "agent source writes are refused (no attachment for writing, no second owner)"
        )
    );
    held.shutdown().await.expect("nothing to release");

    let broken = WorkspaceAdmission::Refused(SourceRefusal::NotAdmitted(
        "workspace lock needs migration".into(),
    ));
    assert!(
        matches!(broken.source_route(), SourceRoute::Refused(SourceRefusal::NotAdmitted(reason)) if reason.contains("needs migration"))
    );
    assert!(
        broken
            .report()
            .contains("workspace not admitted (workspace lock needs migration)")
    );
}

#[tokio::test]
async fn a_missing_root_is_not_admitted_rather_than_an_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let missing = tmp.path().join("never-created");
    let admission = WorkspaceAdmission::admit(&missing).await;
    let WorkspaceAdmission::Refused(SourceRefusal::NotAdmitted(reason)) = &admission else {
        panic!(
            "a missing root is a refusal in the owner's words: {}",
            admission.report()
        );
    };
    assert!(reason.contains("does not exist"), "{reason}");
    assert!(!admission.source_route().is_admitted());
}
