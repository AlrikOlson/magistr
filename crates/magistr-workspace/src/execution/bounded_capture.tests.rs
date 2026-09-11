use super::*;

#[test]
fn small_output_arrives_whole_without_a_marker() {
    let mut capture = BoundedCapture::new(64);
    capture.push(b"hello ");
    capture.push(b"world");
    assert_eq!(capture.text(), "hello world");
    assert_eq!(capture.dropped(), 0);
}

#[test]
fn overflow_keeps_head_and_the_newest_tail_and_counts_the_gap() {
    let mut capture = BoundedCapture::new(16); // head 12, tail 4
    for chunk in [&b"0123456789"[..], b"abcdef", b"ghij", b"KLMNOP"] {
        capture.push(chunk);
    }
    // 26 bytes in: 12 head, 4 tail, 10 dropped.
    assert_eq!(capture.dropped(), 10);
    let text = capture.text();
    assert!(text.starts_with("0123456789ab"));
    assert!(text.ends_with("MNOP"));
    assert!(text.contains("10 bytes of output dropped"));
}

#[test]
fn one_huge_chunk_is_cut_in_a_single_push() {
    let mut capture = BoundedCapture::new(16);
    let bytes: Vec<u8> = (0..100u8).map(|byte| b'a' + byte % 26).collect();
    capture.push(&bytes);
    assert_eq!(capture.dropped(), 84);
    let text = capture.text();
    assert!(text.starts_with(std::str::from_utf8(&bytes[..12]).unwrap()));
    assert!(text.ends_with(std::str::from_utf8(&bytes[96..]).unwrap()));
}
