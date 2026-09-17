use std::time::Duration;

use super::*;
use crate::harness::{FakeTerminal, Script};
use crate::input::Event;
use crate::term::Terminal;
use magistr_cockpit::input::{KeyCode, KeyEvent};

const MS: fn(u64) -> Duration = Duration::from_millis;

fn spawn(
    script: Script,
) -> (
    InputThread,
    tokio::sync::mpsc::UnboundedReceiver<InputEvent>,
) {
    let mut terminal = FakeTerminal::new(Size::new(80, 24), script);
    let input = terminal.input().expect("input");
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let thread = InputThread::spawn(input, Decoder::new(), Size::new(80, 24), tx).expect("spawn");
    (thread, rx)
}

fn key(c: char) -> InputEvent {
    InputEvent::Event(Event::Key(KeyEvent::new(KeyCode::Char(c))))
}

#[test]
fn negotiated_pixel_reports_preserve_subcell_positions() {
    let mut terminal = FakeTerminal::new(
        Size::new(80, 24),
        Script::new()
            .bytes(MS(0), b"\x1b[<0;138;264M".as_slice())
            .close(MS(10)),
    );
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let thread = InputThread::spawn_with_mouse(
        terminal.input().expect("input"),
        Decoder::new(),
        Size::new(80, 24),
        tx,
        true,
    )
    .expect("spawn");
    let InputEvent::Event(Event::Mouse(mouse)) = rx.blocking_recv().expect("report") else {
        panic!("mouse report");
    };
    assert_eq!(mouse.px, Some((137, 263)));
    assert_eq!(
        mouse.kind,
        magistr_cockpit::input::MouseKind::Down(magistr_cockpit::input::Button::Left)
    );
    thread.stop();
}

#[test]
fn keys_a_resize_and_the_close_arrive_in_order_then_the_thread_is_done() {
    let (thread, mut rx) = spawn(
        Script::new()
            .text(MS(0), "ab")
            .resize(MS(10), Size::new(100, 30))
            .text(MS(20), "c")
            .close(MS(30)),
    );
    let mut got = Vec::new();
    while let Some(event) = rx.blocking_recv() {
        got.push(event);
    }
    assert_eq!(
        got,
        vec![
            key('a'),
            key('b'),
            InputEvent::Resize(Size::new(100, 30)),
            key('c'),
            InputEvent::Closed
        ]
    );
    // The sender drops as the thread's loop returns; the OS thread itself
    // may still be unwinding its stack for a few microseconds.
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !thread.is_finished() && std::time::Instant::now() < deadline {
        std::thread::sleep(MS(1));
    }
    assert!(
        thread.is_finished(),
        "the thread ends on its own once the input closes"
    );
    thread.stop();
}

#[test]
fn a_lone_escape_is_resolved_after_a_quiet_read() {
    let (thread, mut rx) = spawn(Script::new().bytes(MS(0), b"\x1b".as_slice()));
    assert_eq!(
        rx.blocking_recv(),
        Some(InputEvent::Event(Event::Key(KeyEvent::new(KeyCode::Esc))))
    );
    thread.stop();
}

#[test]
fn stop_joins_within_a_read_timeout_and_the_size_known_is_not_reported() {
    let (thread, mut rx) = spawn(Script::new());
    let began = std::time::Instant::now();
    assert!(!thread.is_finished());
    thread.stop();
    assert!(began.elapsed() < InputThread::READ_TIMEOUT * 4);
    assert!(
        rx.try_recv().is_err(),
        "nothing was sent: {:?}",
        rx.try_recv()
    );
}

#[test]
fn dropping_the_receiver_ends_the_thread() {
    let (thread, rx) = spawn(Script::new().text(MS(0), "x").text(MS(10), "y"));
    drop(rx);
    std::thread::sleep(MS(100));
    assert!(thread.is_finished());
}
