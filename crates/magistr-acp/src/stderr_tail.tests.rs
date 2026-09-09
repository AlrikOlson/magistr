use super::*;

#[test]
fn append_to_is_identity_when_nothing_was_captured() {
    let tail = StderrTail::default();
    assert_eq!(tail.append_to("boom".into()), "boom");
}

#[test]
fn append_to_quotes_the_captured_lines() {
    let tail = StderrTail::default();
    tail.push("first".into());
    tail.push("second".into());
    let msg = tail.append_to("boom".into());
    assert!(msg.starts_with("boom\n  agent stderr:\n"));
    assert!(msg.contains("first"));
    assert!(msg.contains("second"));
}

#[test]
fn keeps_only_the_last_twenty_lines() {
    let tail = StderrTail::default();
    for i in 0..30 {
        tail.push(format!("line {i}"));
    }
    let msg = tail.append_to(String::new());
    assert!(!msg.contains("line 9\n"), "oldest lines are dropped");
    assert!(msg.contains("line 29"));
}

#[test]
fn attach_only_touches_message_bearing_variants() {
    let tail = StderrTail::default();
    tail.push("why".into());
    assert!(matches!(
        tail.attach(AcpError::connection("x")),
        AcpError::Connection(m) if m.contains("why")
    ));
    assert!(matches!(
        tail.attach(AcpError::transport("x")),
        AcpError::Transport(m) if m.contains("why")
    ));
    assert!(matches!(
        tail.attach(AcpError::timeout("x")),
        AcpError::Timeout(m) if m.contains("why")
    ));
    assert!(matches!(
        tail.attach(AcpError::CliNotFound("x".into())),
        AcpError::CliNotFound(m) if m == "x"
    ));
}
