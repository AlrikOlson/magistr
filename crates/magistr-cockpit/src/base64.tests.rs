use proptest::prelude::*;

use super::*;

fn encoded(input: &[u8]) -> String {
    let mut out = Vec::new();
    encode(input, &mut out);
    String::from_utf8(out).expect("base64 is ascii")
}

/// A reference decoder, only for tests.
fn reference_decode(text: &str) -> Vec<u8> {
    let value = |c: u8| -> u32 {
        u32::try_from(
            ALPHABET
                .iter()
                .position(|&a| a == c)
                .expect("alphabet char"),
        )
        .expect("index fits")
    };
    let mut out = Vec::new();
    for quad in text.as_bytes().chunks(4) {
        let pad = quad.iter().filter(|&&c| c == b'=').count();
        let n = quad
            .iter()
            .map(|&c| if c == b'=' { 0 } else { value(c) })
            .fold(0u32, |acc, v| (acc << 6) | v);
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..4 - pad]);
    }
    out
}

#[test]
fn rfc_4648_test_vectors() {
    assert_eq!(encoded(b""), "");
    assert_eq!(encoded(b"f"), "Zg==");
    assert_eq!(encoded(b"fo"), "Zm8=");
    assert_eq!(encoded(b"foo"), "Zm9v");
    assert_eq!(encoded(b"foob"), "Zm9vYg==");
    assert_eq!(encoded(b"fooba"), "Zm9vYmE=");
    assert_eq!(encoded(b"foobar"), "Zm9vYmFy");
}

#[test]
fn every_byte_value_uses_the_whole_alphabet() {
    let all: Vec<u8> = (0..=255).collect();
    let text = encoded(&all);
    for &c in ALPHABET {
        assert!(text.contains(c as char), "{} missing", c as char);
    }
    assert_eq!(reference_decode(&text), all);
}

#[test]
fn appends_after_existing_bytes() {
    let mut out = b"prefix;".to_vec();
    encode(b"foo", &mut out);
    assert_eq!(out, b"prefix;Zm9v");
}

proptest! {
    #[test]
    fn length_alphabet_padding_and_round_trip(input in proptest::collection::vec(any::<u8>(), 0..300)) {
        let text = encoded(&input);
        prop_assert_eq!(text.len(), input.len().div_ceil(3) * 4);
        prop_assert!(text.bytes().all(|c| ALPHABET.contains(&c) || c == b'='));
        let pad = text.bytes().filter(|&c| c == b'=').count();
        prop_assert_eq!(pad, (3 - input.len() % 3) % 3);
        prop_assert_eq!(reference_decode(&text), input);
    }
}

#[test]
fn decodes_the_rfc_4648_test_vectors() {
    assert_eq!(decode(b"").as_deref(), Some(&b""[..]));
    assert_eq!(decode(b"Zg==").as_deref(), Some(&b"f"[..]));
    assert_eq!(decode(b"Zm8=").as_deref(), Some(&b"fo"[..]));
    assert_eq!(decode(b"Zm9v").as_deref(), Some(&b"foo"[..]));
    assert_eq!(decode(b"Zm9vYg==").as_deref(), Some(&b"foob"[..]));
    assert_eq!(decode(b"Zm9vYmE=").as_deref(), Some(&b"fooba"[..]));
    assert_eq!(decode(b"Zm9vYmFy").as_deref(), Some(&b"foobar"[..]));
}

#[test]
fn decode_skips_whitespace_between_quads() {
    assert_eq!(decode(b"Zm9v\n YmFy").as_deref(), Some(&b"foobar"[..]));
}

#[test]
fn decode_refuses_what_it_cannot_read() {
    assert_eq!(decode(b"Zm9"), None, "length is not a multiple of four");
    assert_eq!(decode(b"Zm9*"), None, "character outside the alphabet");
    assert_eq!(decode(b"Zg==Zg=="), None, "padding before the last quad");
    assert_eq!(decode(b"Z==="), None, "three pad characters");
    assert_eq!(decode(b"Z=g="), None, "padding inside the quad");
}

proptest! {
    #[test]
    fn decode_inverts_encode(input in proptest::collection::vec(any::<u8>(), 0..300)) {
        let text = encoded(&input);
        prop_assert_eq!(decode(text.as_bytes()), Some(input));
    }
}
