use super::*;

#[test]
fn io_error_display() {
    let err = TransportError::Io(std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "pipe broken",
    ));
    assert!(err.to_string().contains("I/O error"));
    assert!(err.to_string().contains("pipe broken"));
}

#[test]
fn serialization_error_display() {
    let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let err = TransportError::Serialization(json_err);
    assert!(err.to_string().contains("serialization error"));
}

#[test]
fn connection_closed_display() {
    let err = TransportError::ConnectionClosed;
    assert_eq!(err.to_string(), "connection closed");
}

#[test]
fn not_connected_display() {
    let err = TransportError::NotConnected;
    assert_eq!(err.to_string(), "not connected");
}

#[test]
fn invalid_frame_display() {
    let err = TransportError::InvalidFrame("missing Content-Length".into());
    assert_eq!(err.to_string(), "invalid frame: missing Content-Length");
}

#[test]
fn from_io_error() {
    let io_err = std::io::Error::other("test");
    let err: TransportError = io_err.into();
    assert!(matches!(err, TransportError::Io(_)));
}

#[test]
fn from_serde_error() {
    let json_err = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    let err: TransportError = json_err.into();
    assert!(matches!(err, TransportError::Serialization(_)));
}
