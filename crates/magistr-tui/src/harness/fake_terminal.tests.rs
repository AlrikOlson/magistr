use std::time::Duration;

use super::*;
use crate::term::TerminalInput;

#[test]
fn writes_are_captured_and_readable_after_the_terminal_is_gone() {
    let mut terminal = FakeTerminal::new(Size::new(80, 24), Script::new());
    let capture = terminal.capture();
    terminal.write(b"abc").expect("write");
    terminal.write(b"abc").expect("write");
    drop(terminal);
    assert_eq!(capture.bytes(), b"abcabc");
    assert_eq!(capture.count(b"abc"), 2);
    assert_eq!(capture.count(b"bca"), 1);
    assert_eq!(capture.count(b""), 0);
    assert_eq!(capture.len(), 6);
    assert!(!capture.is_empty());
}

#[test]
fn the_input_is_taken_once_and_shares_the_size() {
    let mut terminal = FakeTerminal::new(
        Size::new(80, 24),
        Script::new().resize(Duration::ZERO, Size::new(50, 10)),
    );
    assert_eq!(terminal.size().expect("size"), Size::new(80, 24));
    let mut input = terminal.input().expect("input");
    assert!(terminal.input().is_err());
    let mut buf = [0u8; 4];
    assert_eq!(
        input
            .read(&mut buf, Duration::from_millis(10))
            .expect("read"),
        None
    );
    assert_eq!(terminal.size().expect("size"), Size::new(50, 10));
    assert_eq!(input.size().expect("size"), Size::new(50, 10));
}
