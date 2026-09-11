use super::*;

#[test]
fn binary_classification_is_lossless_for_nul_and_invalid_utf8() {
    for bytes in [vec![0, 65], vec![0xff, 0xfe], vec![0xf0, 0x9f, 0x92]] {
        let content = FileContent::decode(bytes.clone());
        assert!(matches!(content, FileContent::Binary(_)));
        assert_eq!(content.as_bytes(), bytes);
    }
}

#[test]
fn utf8_bom_and_non_ascii_text_remain_exact() {
    for text in ["", "hello\r\n", "\u{feff}hello", "λ 🦀", "\u{fffd}"] {
        let content = FileContent::decode(text.as_bytes().to_vec());
        assert!(matches!(content, FileContent::Text(_)));
        assert_eq!(content.as_bytes(), text.as_bytes());
    }
}

#[test]
fn variant_identity_does_not_change_equality_or_hashes() {
    let text = FileContent::Text("abc".into());
    let binary = FileContent::Binary(b"abc".to_vec());
    assert_eq!(text, binary);
    assert_eq!(text.content_hash(), binary.content_hash());
    assert_eq!(
        text.content_hash(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_ne!(
        FileContent::Binary(vec![0xff]),
        FileContent::Text("\u{fffd}".into())
    );
    assert_ne!(Some(FileContent::Text(String::new())), None);
}

#[test]
fn all_byte_values_round_trip_without_replacement() {
    let bytes: Vec<u8> = (0..=255).collect();
    let content = FileContent::decode(bytes.clone());
    assert_eq!(content.as_bytes(), bytes);
}
