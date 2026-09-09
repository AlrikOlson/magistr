use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use super::*;

fn leave() -> Vec<u8> {
    let mut out = Vec::new();
    Session::cockpit().leave(&mut out);
    out
}

#[test]
fn a_panic_runs_the_restorer_with_the_leave_sequence_before_the_previous_hook() {
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&log);
    panic::set_hook(Box::new(move |_| {
        seen.lock().expect("log").push("previous hook".to_owned());
    }));
    let restored = Arc::clone(&log);
    let hook = PanicHook::install(
        Session::cockpit(),
        Box::new(move |bytes| {
            restored
                .lock()
                .expect("log")
                .push(format!("restore {}", String::from_utf8_lossy(bytes)));
        }),
    );
    assert!(PanicHook::is_installed());
    let result = catch_unwind(AssertUnwindSafe(|| panic!("the view blew up")));
    assert!(result.is_err());
    assert_eq!(
        *log.lock().expect("log"),
        vec![
            format!("restore {}", String::from_utf8_lossy(&leave())),
            "previous hook".to_owned()
        ],
        "the terminal is put back, then the message prints"
    );
    assert!(
        !PanicHook::is_installed(),
        "the restorer is spent by the first panic"
    );
    // A second panic finds no restorer and only the previous hook runs.
    let _ = catch_unwind(AssertUnwindSafe(|| panic!("again")));
    assert_eq!(log.lock().expect("log").len(), 3);
    drop(hook);
    let _ = panic::take_hook();
}

#[test]
fn dropping_the_hook_disarms_it_and_puts_the_previous_hook_back() {
    let log: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&log);
    panic::set_hook(Box::new(move |_| {
        seen.lock().expect("log").push("previous")
    }));
    let restored = Arc::clone(&log);
    let hook = PanicHook::install(
        Session::cockpit(),
        Box::new(move |_| restored.lock().expect("log").push("restore")),
    );
    assert!(PanicHook::is_installed());
    drop(hook);
    assert!(!PanicHook::is_installed());
    let _ = catch_unwind(AssertUnwindSafe(|| panic!("after the run")));
    assert_eq!(*log.lock().expect("log"), vec!["previous"]);
    let _ = panic::take_hook();
}

#[test]
fn a_panic_on_a_progress_terminal_clears_the_bar() {
    let seen: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    panic::set_hook(Box::new(|_| {}));
    let restored = Arc::clone(&seen);
    let hook = PanicHook::install(
        Session::cockpit().with_progress(true),
        Box::new(move |bytes| restored.lock().expect("log").extend_from_slice(bytes)),
    );
    let result = catch_unwind(AssertUnwindSafe(|| panic!("the view blew up")));
    assert!(result.is_err());
    assert!(
        seen.lock().expect("log").starts_with(b"\x1b]9;4;0;0\x1b\\"),
        "a crashed session never leaves the dock stuck part-full"
    );
    drop(hook);
    let _ = panic::take_hook();
}
