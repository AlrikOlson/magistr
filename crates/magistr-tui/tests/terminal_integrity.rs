//! Terminal integrity: while the cockpit owns
//! the screen nothing else in the process may write to it. A producer
//! that prints to stderr mid-run — think-and-ship's load banner, a tracing
//! line — reaches the terminal only after the leave sequence; a panic in
//! the view leaves the screen and cooks the tty before its message prints,
//! and the hook is gone once a run ends normally.
//!
//! Unix only: the redirect and the cooked tty it checks are `dup2` and
//! termios, and off Unix the terminal refuses to open at all.

#![cfg(unix)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::time::Duration;

use magistr_cockpit::cockpit::{Model, Size};
use magistr_tui::graphics::Env;
use magistr_tui::harness::{FakeTerminal, Script};
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::term::{OwnedStderr, PanicHook, Session};

const MS: fn(u64) -> Duration = Duration::from_millis;
const SIZE: Size = Size::new(80, 24);
const BANNER: &[u8] = b"think-and-ship: loaded roadmap with 249 chunk(s) from disk";
const LOG_LINE: &[u8] = b"the feed reattached after a reconnect";

fn options() -> Options {
    Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500))
}

fn leave_sequence() -> Vec<u8> {
    let mut out = Vec::new();
    Session::cockpit().leave(&mut out);
    out
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn tokio_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
}

#[test]
fn stderr_written_mid_run_never_reaches_the_screen_and_is_replayed_after_the_leave() {
    let _ = tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .without_time()
            .finish(),
    );
    let terminal = FakeTerminal::new(
        SIZE,
        Script::new().startup_replies(MS(0)).text(MS(400), "\x04"),
    )
    .owning_stderr()
    .expect("own stderr");
    let capture = terminal.capture();
    // A producer that prints mid-run, exactly like an engine reload at an
    // iteration boundary: eprintln! and a tracing line on the stderr writer.
    let producer = std::thread::spawn(|| {
        std::thread::sleep(MS(150));
        eprintln!("{}", String::from_utf8_lossy(BANNER));
        tracing::warn!("{}", String::from_utf8_lossy(LOG_LINE));
    });
    let model = tokio_runtime()
        .block_on(Runtime::new(terminal, options()).run(Model::new(SIZE)))
        .expect("the run succeeds");
    producer.join().expect("producer");
    assert!(model.quit_requested());

    let bytes = capture.bytes();
    let leave = leave_sequence();
    let at = find(&bytes, &leave).expect("the leave sequence is in the stream");
    let (screen, scrollback) = bytes.split_at(at + leave.len());
    assert!(
        find(screen, BANNER).is_none() && find(screen, LOG_LINE).is_none(),
        "nothing foreign reached the screen"
    );
    assert!(
        find(scrollback, BANNER).is_some(),
        "the banner is replayed after the leave sequence"
    );
    assert!(
        find(scrollback, LOG_LINE).is_some(),
        "the tracing line is replayed after the leave sequence"
    );
    assert!(
        !PanicHook::is_installed(),
        "the hook is removed when the run ends normally"
    );
}

#[test]
fn a_panic_in_the_view_leaves_the_screen_and_the_tty_before_the_message_prints() {
    // The test's own stderr goes to a file, so the panic message the
    // default hook prints can be read back afterwards.
    let log = OwnedStderr::take().expect("own the test's stderr");
    let terminal = FakeTerminal::new(
        SIZE,
        Script::new().startup_replies(MS(0)).text(MS(300), "\x04"),
    )
    .owning_stderr()
    .expect("own stderr");
    let capture = terminal.capture();
    let deliveries = terminal.deliveries();
    let runtime = Runtime::new(terminal, options().panic_on_frame(1));
    let result = catch_unwind(AssertUnwindSafe(|| {
        tokio_runtime().block_on(runtime.run(Model::new(SIZE)))
    }));
    let stderr = log.restore().expect("restore the test's stderr");
    assert!(result.is_err(), "the run panicked");

    let bytes = capture.bytes();
    assert!(
        bytes.ends_with(&leave_sequence()),
        "the leave sequence is the last thing written"
    );
    assert!(
        find(&stderr, b"cockpit fault injection").is_some(),
        "the panic message reached stderr: {}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(
        !PanicHook::is_installed(),
        "the restorer is spent and the hook disarmed"
    );
    // The input thread is joined while unwinding: the script's `q` at
    // 300 ms is never delivered.
    let delivered = deliveries.len();
    std::thread::sleep(MS(400));
    assert_eq!(deliveries.len(), delivered, "the reader is gone");
    assert_eq!(capture.len(), bytes.len(), "nothing is written afterwards");
}

#[test]
fn the_hook_is_armed_only_while_the_run_lasts() {
    let terminal = FakeTerminal::new(
        SIZE,
        Script::new().startup_replies(MS(0)).text(MS(300), "\x04"),
    );
    let (tx, rx) = mpsc::channel();
    let observer = std::thread::spawn(move || {
        std::thread::sleep(MS(150));
        let _ = tx.send(PanicHook::is_installed());
    });
    assert!(!PanicHook::is_installed());
    tokio_runtime()
        .block_on(Runtime::new(terminal, options()).run(Model::new(SIZE)))
        .expect("the run succeeds");
    observer.join().expect("observer");
    assert!(rx.recv().expect("observed"), "armed mid-run");
    assert!(!PanicHook::is_installed(), "disarmed after the run");
}
