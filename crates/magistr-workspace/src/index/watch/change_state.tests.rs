use super::*;
use notify::event::{AccessKind, Flag, ModifyKind};

#[test]
fn old_acknowledgement_preserves_newer_invalidation_and_burst_is_bounded() {
    let state = ChangeState::new(WatchMode::Auto, WatchMode::Native);
    let observed = state.epoch();
    for _ in 0..20_000 {
        state.changed(false, None);
    }
    state.acknowledge(observed);
    let status = state.snapshot();
    assert_eq!(status.epoch, 20_001);
    assert_eq!(status.events, 20_000);
    assert!(status.pending);
    state.acknowledge(status.epoch);
    assert!(!state.snapshot().pending);
    assert_eq!(state.snapshot().lag_ms, 0);
}

#[test]
fn overflow_and_failure_invalidate_even_without_a_named_path() {
    let state = ChangeState::new(WatchMode::Native, WatchMode::Native);
    state.receive(
        Path::new("/root"),
        Ok(Event::new(EventKind::Any).set_flag(Flag::Rescan)),
    );
    state.receive(
        Path::new("/root"),
        Err(notify::Error::generic(&"x".repeat(2_000))),
    );
    let status = state.snapshot();
    assert_eq!(status.epoch, 3);
    assert_eq!(status.rescans, 2);
    assert_eq!(status.failures, 1);
    assert!(status.last_error.unwrap().len() <= 512);
}

#[test]
fn observation_and_owned_cache_writes_do_not_cause_rescan_loops() {
    let state = ChangeState::new(WatchMode::Native, WatchMode::Native);
    let root = Path::new("/root");
    for path in [
        "/outside.rs",
        "/root/.workbench/index.db",
        "/root/.git/HEAD",
        "/root/file.rs.workbench-tmp-123",
    ] {
        state.receive(
            root,
            Ok(Event::new(EventKind::Modify(ModifyKind::Any)).add_path(path.into())),
        );
    }
    state.receive(
        root,
        Ok(Event::new(EventKind::Access(AccessKind::Read)).add_path("/root/file.rs".into())),
    );
    assert_eq!(state.epoch(), 1);
    state.receive(
        root,
        Ok(Event::new(EventKind::Modify(ModifyKind::Any)).add_path("/root/.gitignore".into())),
    );
    assert_eq!(state.epoch(), 2);
    state.stop();
    state.changed(true, Some("late callback"));
    assert_eq!(state.epoch(), 2);
}

#[test]
fn native_success_recovers_active_error_and_preserves_failure_history() {
    let state = ChangeState::new(WatchMode::Native, WatchMode::Native);
    let root = Path::new("/root");
    state.receive(root, Err(notify::Error::generic("temporary event failure")));
    let failed = state.snapshot();
    assert!(failed.last_error.is_some());
    state.acknowledge(failed.epoch);
    state.receive(
        root,
        Ok(Event::new(EventKind::Modify(ModifyKind::Any)).add_path(root.join("source.rs"))),
    );
    let recovered = state.snapshot();
    assert!(recovered.last_error.is_none());
    assert_eq!(recovered.failures, 1);
    assert!(recovered.epoch > failed.epoch);
    assert!(recovered.pending);
}
