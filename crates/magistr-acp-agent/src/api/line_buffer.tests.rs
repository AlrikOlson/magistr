use super::*;

#[test]
fn a_line_split_across_pushes_is_one_line() {
    let mut buffer = LineBuffer::default();
    buffer.push(b"data: {\"a\":");
    assert!(buffer.drain_lines().expect("utf-8").is_empty());
    buffer.push(b" 1}\n\ndata: [DONE]\n");
    assert_eq!(
        buffer.drain_lines().expect("utf-8"),
        vec!["data: {\"a\": 1}", "", "data: [DONE]"]
    );
}

#[test]
fn crlf_endings_are_stripped() {
    let mut buffer = LineBuffer::default();
    buffer.push(b"one\r\ntwo\r\n");
    assert_eq!(buffer.drain_lines().expect("utf-8"), vec!["one", "two"]);
}

#[test]
fn finish_yields_the_unterminated_tail_once() {
    let mut buffer = LineBuffer::default();
    buffer.push(b"{\"done\":true}");
    assert!(buffer.drain_lines().expect("utf-8").is_empty());
    assert_eq!(
        buffer.finish().expect("utf-8"),
        Some("{\"done\":true}".to_owned())
    );
    assert_eq!(buffer.finish().expect("utf-8"), None);
}

#[test]
fn a_multibyte_character_split_across_pushes_survives() {
    let mut buffer = LineBuffer::default();
    let bytes = "héllo\n".as_bytes();
    buffer.push(&bytes[..2]);
    buffer.push(&bytes[2..]);
    assert_eq!(buffer.drain_lines().expect("utf-8"), vec!["héllo"]);
}

#[test]
fn invalid_utf8_is_refused() {
    let mut buffer = LineBuffer::default();
    buffer.push(b"\xff\xfe\n");
    let err = buffer.drain_lines().expect_err("not UTF-8");
    assert!(err.to_string().contains("not UTF-8"), "{err}");
}
