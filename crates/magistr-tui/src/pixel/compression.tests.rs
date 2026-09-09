use std::io::Read as _;

use super::*;

#[test]
fn none_copies_the_bytes_and_says_so() {
    let mut out = vec![9, 9];
    assert!(!Compression::None.apply(&[1, 2, 3], &mut out));
    assert_eq!(out, [1, 2, 3]);
}

#[test]
fn zlib_produces_an_rfc_1950_stream_that_inflates_back() {
    let raw: Vec<u8> = (0..4096u32).map(|i| (i % 5) as u8).collect();
    let mut out = Vec::new();
    assert!(Compression::Zlib(1).apply(&raw, &mut out));
    assert!(out.len() < raw.len() / 4, "{} bytes", out.len());
    assert_eq!(out[0] & 0x0f, 8, "zlib header: deflate method");
    let mut back = Vec::new();
    flate2::read::ZlibDecoder::new(out.as_slice())
        .read_to_end(&mut back)
        .expect("inflate");
    assert_eq!(back, raw);
}

#[test]
fn the_default_is_a_named_choice() {
    assert_eq!(Compression::DEFAULT.key(), Some("o=z"));
    assert_eq!(Compression::None.key(), None);
}
