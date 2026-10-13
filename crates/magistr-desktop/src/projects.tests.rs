use super::*;
fn inputs(root: &Path) -> ProjectInputs {
    ProjectInputs::from_lookup(root, |_| None)
}

#[test]
fn preview_writes_nothing_and_apply_uses_the_retained_plan_once() {
    let temp = tempfile::tempdir().unwrap();
    let reviews = ProjectReviews::default();
    let preview = reviews
        .preview(temp.path(), &inputs(temp.path()), false)
        .unwrap();
    let ProjectPreview::Setup { review, files, .. } = preview else {
        panic!("setup required")
    };
    assert!(!temp.path().join(".magistr").exists());
    let approved = files
        .iter()
        .enumerate()
        .filter(|(_, file)| file.selectable)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let result = reviews.apply(review, &approved).unwrap();
    assert!(result.complete);
    assert!(reviews.apply(review, &approved).is_err());
    assert!(matches!(
        reviews
            .preview(temp.path(), &inputs(temp.path()), false)
            .unwrap(),
        ProjectPreview::Ready { .. }
    ));
}

#[test]
fn cancellation_and_new_reviews_revoke_old_intents() {
    let temp = tempfile::tempdir().unwrap();
    let reviews = ProjectReviews::default();
    let ProjectPreview::Setup { review: old, .. } = reviews
        .preview(temp.path(), &inputs(temp.path()), false)
        .unwrap()
    else {
        panic!("setup required")
    };
    let ProjectPreview::Setup { review, .. } = reviews
        .preview(temp.path(), &inputs(temp.path()), false)
        .unwrap()
    else {
        panic!("setup required")
    };
    assert!(reviews.apply(old, &[]).is_err());
    reviews.discard(review).unwrap();
    assert!(reviews.apply(review, &[]).is_err());
    assert!(!temp.path().join(".magistr").exists());
}

#[test]
fn partial_selection_returns_real_outcomes_without_admission() {
    let temp = tempfile::tempdir().unwrap();
    let reviews = ProjectReviews::default();
    let ProjectPreview::Setup { review, files, .. } = reviews
        .preview(temp.path(), &inputs(temp.path()), false)
        .unwrap()
    else {
        panic!("setup required")
    };
    let config = files
        .iter()
        .position(|file| file.path == ".magistr/project.json")
        .unwrap();
    let result = reviews.apply(review, &[config]).unwrap();
    assert!(!result.complete);
    assert_eq!(
        result
            .files
            .iter()
            .filter(|file| file.outcome == "applied")
            .count(),
        1
    );
    assert!(matches!(
        reviews
            .preview(temp.path(), &inputs(temp.path()), true)
            .unwrap(),
        ProjectPreview::Setup { .. }
    ));
}

#[test]
fn invalid_folder_or_settings_are_visible_errors() {
    let temp = tempfile::tempdir().unwrap();
    let reviews = ProjectReviews::default();
    assert!(
        reviews
            .preview(&temp.path().join("missing"), &inputs(temp.path()), false)
            .is_err()
    );
    std::fs::create_dir(temp.path().join(".magistr")).unwrap();
    std::fs::write(temp.path().join(".magistr/project.json"), "invalid").unwrap();
    assert!(
        reviews
            .preview(temp.path(), &inputs(temp.path()), false)
            .unwrap_err()
            .contains("project.json")
    );
}
