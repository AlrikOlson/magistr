use super::*;

#[test]
fn from_hex_with_hash() {
    let c = ThemeColor::from_hex("#89b4fa").unwrap();
    assert_eq!(c, ThemeColor::new(0x89, 0xb4, 0xfa));
}

#[test]
fn from_hex_without_hash() {
    let c = ThemeColor::from_hex("1e1e2e").unwrap();
    assert_eq!(c, ThemeColor::new(0x1e, 0x1e, 0x2e));
}

#[test]
fn from_hex_invalid_length() {
    assert!(ThemeColor::from_hex("#fff").is_none());
    assert!(ThemeColor::from_hex("").is_none());
    assert!(ThemeColor::from_hex("#1234567").is_none());
}

#[test]
fn from_hex_invalid_chars() {
    assert!(ThemeColor::from_hex("#gggggg").is_none());
}

#[test]
fn to_hex_roundtrip() {
    let c = ThemeColor::new(0x89, 0xb4, 0xfa);
    assert_eq!(c.to_hex(), "#89b4fa");
    assert_eq!(ThemeColor::from_hex(&c.to_hex()), Some(c));
}

#[test]
fn display_matches_to_hex() {
    let c = ThemeColor::new(255, 0, 128);
    assert_eq!(format!("{c}"), c.to_hex());
}

#[test]
fn black_and_white() {
    assert_eq!(
        ThemeColor::from_hex("#000000"),
        Some(ThemeColor::new(0, 0, 0))
    );
    assert_eq!(
        ThemeColor::from_hex("#ffffff"),
        Some(ThemeColor::new(255, 255, 255))
    );
}
