use super::*;

/// The transcript rides as base64 between the OSC 52 head and the
/// BEL: nothing of it — not a newline, not an escape — is on the wire
/// as itself, and a decode gives the text back whole.
#[test]
fn the_clipboard_write_is_osc_52_over_base64() {
    let mut out = Vec::new();
    clipboard(&mut out, "line 1\nline 2\x1b[0m");
    assert!(out.starts_with(b"\x1b]52;c;"), "{out:?}");
    assert_eq!(out.last(), Some(&0x07));
    let payload = &out[b"\x1b]52;c;".len()..out.len() - 1];
    assert!(
        payload.iter().all(u8::is_ascii_alphanumeric_or_base64),
        "{payload:?}"
    );
    assert_eq!(
        base64::decode(payload).expect("base64"),
        b"line 1\nline 2\x1b[0m"
    );
}

trait Base64Byte {
    fn is_ascii_alphanumeric_or_base64(&self) -> bool;
}

impl Base64Byte for u8 {
    fn is_ascii_alphanumeric_or_base64(&self) -> bool {
        self.is_ascii_alphanumeric() || matches!(self, b'+' | b'/' | b'=')
    }
}
