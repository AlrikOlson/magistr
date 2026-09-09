use std::error::Error as _;

use super::*;

#[test]
fn io_and_layout_errors_convert_display_and_carry_their_source() {
    let io: RuntimeError = std::io::Error::other("gone").into();
    assert_eq!(io.to_string(), "terminal: gone");
    assert!(io.source().is_some());
    assert!(matches!(io, RuntimeError::Io(_)));
}
