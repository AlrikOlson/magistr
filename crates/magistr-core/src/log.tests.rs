use super::*;

#[test]
fn log_message_clone() {
    let msg = LogMessage {
        level: LogLevel::Error,
        text: "something broke".into(),
    };
    let cloned = msg.clone();
    assert_eq!(cloned.level, LogLevel::Error);
    assert_eq!(cloned.text, "something broke");
}

#[test]
fn log_level_equality() {
    assert_eq!(LogLevel::Info, LogLevel::Info);
    assert_ne!(LogLevel::Info, LogLevel::Error);
}
