use super::*;

fn assert_public_error(error: eyre::Report, message: &str) {
    assert_eq!(error.to_string(), message);
    assert_eq!(format!("{error:#}"), message);
}

fn fixture() -> (tempfile::TempDir, ProjectInputs, DesktopPreferencesService) {
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None)
        .with_config_dir(dir.path().join("personal"));
    let service = DesktopPreferencesService::new(inputs.clone());
    (dir, inputs, service)
}

#[tokio::test]
async fn captured_preferences_survive_service_restart_without_touching_the_project() {
    let (dir, inputs, service) = fixture();
    let initial = service.load().await.unwrap();
    assert_eq!(initial.preferences, DesktopPreferences::default());
    assert!(!inputs.config_dir().unwrap().exists());
    let selected = DesktopPreferences {
        theme: ThemePreference::Dark,
        reduce_motion: true,
    };
    let saved = service.save(initial.revision, selected).await.unwrap();
    assert_eq!(saved.preferences, selected);
    assert!(saved.revision > initial.revision);
    let restarted = DesktopPreferencesService::new(inputs.clone());
    assert_eq!(restarted.load().await.unwrap().preferences, selected);
    let config = inputs.user_config().unwrap().value;
    assert_eq!(config.theme.default, ThemePreference::Dark);
    assert_eq!(config.accessibility.reduce_motion, Some(true));
    assert!(!dir.path().join(".magistr").exists());
}

#[tokio::test]
async fn stale_save_and_old_view_after_refresh_cannot_replace_accepted_values() {
    let (_dir, _inputs, service) = fixture();
    let initial = service.load().await.unwrap();
    let selected = DesktopPreferences {
        theme: ThemePreference::Light,
        reduce_motion: false,
    };
    let saved = service.save(initial.revision, selected).await.unwrap();
    assert_public_error(
        service
            .save(initial.revision, DesktopPreferences::default())
            .await
            .unwrap_err(),
        "Settings changed elsewhere. Choose Retry preferences to save your choices.",
    );
    assert_eq!(service.state.lock().unwrap().snapshot(), saved);
    let refreshed = service.load().await.unwrap();
    assert!(refreshed.revision > saved.revision);
    assert_public_error(
        service
            .save(saved.revision, DesktopPreferences::default())
            .await
            .unwrap_err(),
        "Settings changed elsewhere. Choose Retry preferences to save your choices.",
    );
    assert_eq!(refreshed.preferences, selected);
}

#[tokio::test]
async fn external_change_requires_reload_and_retains_unrelated_preferences() {
    let (_dir, inputs, service) = fixture();
    let initial = service.load().await.unwrap();
    let path = inputs.user_config_path().unwrap();
    UserConfigDocument::load(&path)
        .unwrap()
        .save(&[("models", "codex", "chosen-model".into())])
        .unwrap();
    let draft = DesktopPreferences {
        theme: ThemePreference::Dark,
        reduce_motion: true,
    };
    assert_public_error(
        service.save(initial.revision, draft).await.unwrap_err(),
        "Settings changed elsewhere. Choose Retry preferences to save your choices.",
    );
    assert_eq!(service.state.lock().unwrap().snapshot(), initial);
    let refreshed = service.load().await.unwrap();
    assert_eq!(
        service
            .save(refreshed.revision, draft)
            .await
            .unwrap()
            .preferences,
        draft
    );
    assert_eq!(
        inputs
            .user_config()
            .unwrap()
            .value
            .models
            .for_provider("codex"),
        Some("chosen-model")
    );
}

#[tokio::test]
async fn malformed_settings_refuse_load_and_save_without_discarding_the_last_observation() {
    let (_dir, inputs, service) = fixture();
    let initial = service.load().await.unwrap();
    let saved = service
        .save(initial.revision, DesktopPreferences::default())
        .await
        .unwrap();
    let path = inputs.user_config_path().unwrap();
    for text in [
        "[theme]\ndefault = 'unknown'\n",
        "models = 'wrong type'\n",
        "bad = [",
    ] {
        std::fs::write(&path, text).unwrap();
        assert_public_error(
            service.load().await.unwrap_err(),
            "Your settings file contains an error. Correct it, then choose Retry preferences.",
        );
        assert_public_error(
            service
                .save(saved.revision, DesktopPreferences::default())
                .await
                .unwrap_err(),
            "Settings changed elsewhere. Choose Retry preferences to save your choices.",
        );
        assert_eq!(service.state.lock().unwrap().snapshot(), saved);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
}

#[tokio::test]
async fn unreadable_settings_preserve_the_last_observation_without_exposing_file_errors() {
    let (_dir, inputs, service) = fixture();
    let initial = service.load().await.unwrap();
    let path = inputs.user_config_path().unwrap();
    std::fs::create_dir_all(&path).unwrap();
    assert_public_error(
        service.load().await.unwrap_err(),
        "Could not read your saved preferences. Check that your settings folder is available, then try again.",
    );
    assert_eq!(service.state.lock().unwrap().snapshot(), initial);
    assert!(path.is_dir());
    std::fs::remove_dir(&path).unwrap();
    let selected = DesktopPreferences {
        theme: ThemePreference::Dark,
        reduce_motion: true,
    };
    let saved = service.save(initial.revision, selected).await.unwrap();
    assert_eq!(saved.preferences, selected);
}

#[tokio::test]
async fn busy_settings_offer_retry_without_changing_the_observed_revision() {
    let (_dir, inputs, service) = fixture();
    let initial = service.load().await.unwrap();
    let path = inputs.user_config_path().unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut lock_path = path.as_os_str().to_os_string();
    lock_path.push(".lock");
    let held = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .unwrap();
    held.try_lock().unwrap();
    let selected = DesktopPreferences {
        theme: ThemePreference::Light,
        reduce_motion: true,
    };
    assert_public_error(
        service.save(initial.revision, selected).await.unwrap_err(),
        "Settings are being saved. Try again in a moment.",
    );
    assert_eq!(service.state.lock().unwrap().snapshot(), initial);
    assert!(!path.exists());
    drop(held);
    let saved = service.save(initial.revision, selected).await.unwrap();
    assert_eq!(saved.preferences, selected);
    assert_eq!(service.load().await.unwrap().preferences, selected);
}

#[tokio::test]
async fn failed_save_keeps_the_same_revision_valid_for_retry() {
    let (_dir, inputs, service) = fixture();
    let initial = service.load().await.unwrap();
    let path = inputs.user_config_path().unwrap();
    let parent = path.parent().unwrap();
    let obstruction = "A file occupies the settings directory.";
    std::fs::write(parent, obstruction).unwrap();
    let draft = DesktopPreferences {
        theme: ThemePreference::Light,
        reduce_motion: true,
    };
    assert_public_error(
        service.save(initial.revision, draft).await.unwrap_err(),
        "Could not save your preferences. Check that you can save files in your settings folder, then try again.",
    );
    assert_eq!(service.state.lock().unwrap().snapshot(), initial);
    assert_eq!(std::fs::read_to_string(parent).unwrap(), obstruction);
    assert!(!path.exists());
    std::fs::remove_file(parent).unwrap();
    let saved = service.save(initial.revision, draft).await.unwrap();
    assert_eq!(saved.preferences, draft);
    assert!(saved.revision > initial.revision);
    assert_eq!(
        DesktopPreferencesService::new(inputs)
            .load()
            .await
            .unwrap()
            .preferences,
        draft
    );
}

#[tokio::test]
async fn two_commands_for_one_revision_accept_only_one_save() {
    let (_dir, _inputs, service) = fixture();
    let initial = service.load().await.unwrap();
    let a = DesktopPreferences {
        theme: ThemePreference::Light,
        reduce_motion: true,
    };
    let b = DesktopPreferences {
        theme: ThemePreference::Dark,
        reduce_motion: false,
    };
    let (left, right) = tokio::join!(
        service.save(initial.revision, a),
        service.save(initial.revision, b)
    );
    assert_ne!(left.is_ok(), right.is_ok());
    let winner = left.or(right).unwrap();
    assert_eq!(
        service.load().await.unwrap().preferences,
        winner.preferences
    );
}

#[tokio::test]
async fn unlocated_settings_are_default_but_cannot_claim_to_be_saved() {
    let dir = tempfile::tempdir().unwrap();
    let service = DesktopPreferencesService::new(ProjectInputs::from_lookup(dir.path(), |_| None));
    let initial = service.load().await.unwrap();
    assert_eq!(initial.preferences, DesktopPreferences::default());
    assert_public_error(
        service
            .save(initial.revision, DesktopPreferences::default())
            .await
            .unwrap_err(),
        "Magistr could not find your settings folder. Restart Magistr and try again.",
    );
    assert_eq!(service.state.lock().unwrap().snapshot(), initial);
}

#[tokio::test(flavor = "current_thread")]
async fn reading_settings_leaves_the_async_runtime_available() {
    use std::time::{Duration, Instant};
    let (_dir, _inputs, service) = fixture();
    let state = Arc::clone(&service.state);
    let (held, held_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let blocker = std::thread::spawn(move || {
        let _guard = state.lock().unwrap();
        held.send(()).unwrap();
        // A broken implementation must fail the test instead of deadlocking it.
        let _ = release_rx.recv_timeout(Duration::from_secs(2));
    });
    held_rx.await.unwrap();
    let started = Instant::now();
    let load = tokio::spawn(async move { service.load().await });
    tokio::time::sleep(Duration::from_millis(10)).await;
    let runtime_delay = started.elapsed();
    let _ = release.send(());
    load.await.unwrap().unwrap();
    blocker.join().unwrap();
    assert!(runtime_delay < Duration::from_secs(1), "{runtime_delay:?}");
}

#[test]
fn preferences_accept_only_supported_complete_values() {
    let preferences: DesktopPreferences = serde_json::from_value(serde_json::json!({
        "theme": "system", "reduce_motion": false
    }))
    .unwrap();
    assert_eq!(preferences, DesktopPreferences::default());
    for value in [
        serde_json::json!({"theme": "unknown", "reduce_motion": false}),
        serde_json::json!({"theme": "dark"}),
        serde_json::json!({"theme": "dark", "reduce_motion": true, "unsupported": true}),
    ] {
        assert!(serde_json::from_value::<DesktopPreferences>(value).is_err());
    }
}
