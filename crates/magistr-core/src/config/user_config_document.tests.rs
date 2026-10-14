use super::*;
use crate::config::user_config::ThemePreference;

#[test]
fn read_is_side_effect_free_and_first_save_creates_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("personal/nested");
    let path = parent.join("config.toml");
    let observed = UserConfigDocument::load(&path).unwrap();
    assert!(!observed.is_present());
    assert!(!parent.exists());
    let saved = observed
        .save(&[("theme", "default", "light".into())])
        .unwrap();
    assert!(saved.is_present());
    assert_eq!(saved.config().theme.default, ThemePreference::Light);
    assert_eq!(
        UserConfigDocument::load(&path).unwrap().config(),
        saved.config()
    );
}

#[test]
fn edits_preserve_comments_unrelated_fields_and_unknown_sections() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let original = "# Personal settings\n[theme] # Theme comment\ndefault = 'system' # Follow desktop\n\n[models]\ncodex = 'keep-this'\n\n[extension]\nvalue = 42 # Preserve extensions\n";
    fs::write(&path, original).unwrap();
    let saved = UserConfigDocument::load(&path)
        .unwrap()
        .save(&[
            ("theme", "default", "dark".into()),
            ("accessibility", "reduce_motion", true.into()),
        ])
        .unwrap();
    assert_eq!(saved.config().theme.default, ThemePreference::Dark);
    assert_eq!(saved.config().accessibility.reduce_motion, Some(true));
    let text = fs::read_to_string(path).unwrap();
    for preserved in [
        "# Personal settings",
        "# Theme comment",
        "# Follow desktop",
        "codex = 'keep-this'",
        "[extension]",
        "value = 42 # Preserve extensions",
    ] {
        assert!(text.contains(preserved), "{preserved}: {text}");
    }
}

#[test]
fn inline_tables_keep_their_other_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "theme = { default = 'system' }\naccessibility = { reduce_motion = false, high_contrast = true }\n").unwrap();
    let saved = UserConfigDocument::load(&path)
        .unwrap()
        .save(&[
            ("theme", "default", "light".into()),
            ("accessibility", "reduce_motion", true.into()),
        ])
        .unwrap();
    assert_eq!(saved.config().theme.default, ThemePreference::Light);
    assert_eq!(saved.config().accessibility.high_contrast, Some(true));
    assert_eq!(saved.config().accessibility.reduce_motion, Some(true));
}

#[test]
fn stale_document_cannot_overwrite_another_writers_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let first = UserConfigDocument::load(&path).unwrap();
    let second = UserConfigDocument::load(&path).unwrap();
    let saved = first
        .save(&[("models", "codex", "new-model".into())])
        .unwrap();
    assert!(matches!(
        second.save(&[("theme", "default", "dark".into())]),
        Err(UserConfigError::Conflict(_))
    ));
    assert_eq!(
        UserConfigDocument::load(&path).unwrap().config(),
        saved.config()
    );
    let refreshed = UserConfigDocument::load(&path).unwrap();
    let saved = refreshed
        .save(&[("theme", "default", "dark".into())])
        .unwrap();
    assert_eq!(
        saved.config().models.for_provider("codex"),
        Some("new-model")
    );
    assert_eq!(saved.config().theme.default, ThemePreference::Dark);
}

#[test]
fn creating_an_empty_file_invalidates_an_absent_observation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let observed = UserConfigDocument::load(&path).unwrap();
    fs::write(&path, "").unwrap();
    assert!(matches!(
        observed.save(&[("theme", "default", "dark".into())]),
        Err(UserConfigError::Conflict(_))
    ));
    assert_eq!(fs::read_to_string(path).unwrap(), "");
}

#[test]
fn invalid_existing_or_proposed_settings_are_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    for text in [
        "not = [valid",
        "models = 'wrong type'\n",
        "[theme]\ndefault = 'unknown'\n",
    ] {
        fs::write(&path, text).unwrap();
        assert!(UserConfigDocument::load(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
    let text = "# Keep\n[theme]\ndefault = 'system'\n";
    fs::write(&path, text).unwrap();
    let observed = UserConfigDocument::load(&path).unwrap();
    assert!(matches!(
        observed.save(&[("theme", "default", "unknown".into())]),
        Err(UserConfigError::Parse(_, _))
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), text);
    observed
        .save(&[("theme", "default", "dark".into())])
        .unwrap();
}

#[test]
fn lock_contention_refuses_without_changing_settings_and_releases_on_drop() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let observed = UserConfigDocument::load(&path).unwrap();
    let held = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path(&path))
        .unwrap();
    held.try_lock().unwrap();
    assert!(matches!(
        observed.save(&[("theme", "default", "light".into())]),
        Err(UserConfigError::Busy(_))
    ));
    assert!(!path.exists());
    drop(held);
    observed
        .save(&[("theme", "default", "light".into())])
        .unwrap();
}

#[test]
fn failed_save_keeps_original_observation_available_for_retry() {
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("personal");
    fs::create_dir(&parent).unwrap();
    let path = parent.join("config.toml");
    let original = "# Keep\n[theme]\ndefault = 'system'\n";
    fs::write(&path, original).unwrap();
    let observed = UserConfigDocument::load(&path).unwrap();
    let retained = dir.path().join("retained");
    fs::rename(&parent, &retained).unwrap();
    let obstruction = "A file occupies the settings directory.";
    fs::write(&parent, obstruction).unwrap();
    assert!(matches!(
        observed.save(&[("theme", "default", "dark".into())]),
        Err(UserConfigError::Io(failed_path, _)) if failed_path == parent
    ));
    assert_eq!(fs::read_to_string(&parent).unwrap(), obstruction);
    assert_eq!(
        fs::read_to_string(retained.join("config.toml")).unwrap(),
        original
    );
    fs::remove_file(&parent).unwrap();
    fs::rename(&retained, &parent).unwrap();
    let saved = observed
        .save(&[("theme", "default", "dark".into())])
        .unwrap();
    assert_eq!(saved.config().theme.default, ThemePreference::Dark);
    assert_eq!(
        UserConfigDocument::load(&path).unwrap().config(),
        saved.config()
    );
}

#[test]
fn competing_writers_cannot_both_commit_the_same_observation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let first = UserConfigDocument::load(&path).unwrap();
    let second = UserConfigDocument::load(&path).unwrap();
    let ready = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let a = scope.spawn(|| {
            ready.wait();
            first.save(&[("theme", "default", "light".into())])
        });
        let b = scope.spawn(|| {
            ready.wait();
            second.save(&[("theme", "default", "dark".into())])
        });
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_ne!(results.0.is_ok(), results.1.is_ok());
    let winner = results.0.or(results.1).unwrap();
    assert_eq!(
        UserConfigDocument::load(&path).unwrap().config(),
        winner.config()
    );
}
