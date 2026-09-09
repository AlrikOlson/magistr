use super::*;

#[test]
fn turn_role_labels() {
    assert_eq!(TurnRole::User.label(), "you");
    assert_eq!(TurnRole::Assistant.label(), "mag");
    assert_eq!(TurnRole::System.label(), "sys");
}

#[test]
fn text_style_discriminant_roundtrip() {
    // Every variant should survive a discriminant roundtrip.
    for n in 0..TextStyle::COUNT {
        let style =
            TextStyle::from_discriminant(n as u8).unwrap_or_else(|| panic!("missing variant {n}"));
        assert_eq!(style.discriminant() as usize, n);
    }
}

#[test]
fn text_style_from_discriminant_out_of_range() {
    assert!(TextStyle::from_discriminant(TextStyle::COUNT as u8).is_none());
    assert!(TextStyle::from_discriminant(255).is_none());
}

#[test]
fn text_style_known_discriminants() {
    // Spot-check key variants to guard against accidental reordering.
    assert_eq!(TextStyle::Normal.discriminant(), 0);
    assert_eq!(TextStyle::Bold.discriminant(), 1);
    assert_eq!(TextStyle::Error.discriminant(), 6);
    assert_eq!(TextStyle::SyntaxKeyword.discriminant(), 22);
    assert_eq!(TextStyle::SearchMatchActive.discriminant(), 48);
    assert_eq!(TextStyle::ToolToggle.discriminant(), 60);
}

#[test]
fn styled_segment_construction() {
    let seg = StyledSegment {
        text: "hello".into(),
        style: TextStyle::Bold,
    };
    assert_eq!(seg.text, "hello");
    assert_eq!(seg.style, TextStyle::Bold);
}

#[test]
fn stream_block_construction() {
    let block = StreamBlock {
        id: 1,
        turn_number: 1,
        created_at_ms: 1_700_000_000_000,
        collapsed: false,
        cost_usd: Some(0.05),
        role: TurnRole::Assistant,
    };
    assert_eq!(block.turn_number, 1);
    assert_eq!(block.role, TurnRole::Assistant);
    assert!(!block.collapsed);
}

#[test]
fn serde_roundtrip_text_style() {
    let style = TextStyle::SyntaxFunction;
    let json = serde_json::to_string(&style).unwrap();
    let back: TextStyle = serde_json::from_str(&json).unwrap();
    assert_eq!(back, style);
}

#[test]
fn serde_roundtrip_styled_segment() {
    let seg = StyledSegment {
        text: "fn main()".into(),
        style: TextStyle::SyntaxKeyword,
    };
    let json = serde_json::to_string(&seg).unwrap();
    let back: StyledSegment = serde_json::from_str(&json).unwrap();
    assert_eq!(back, seg);
}

#[test]
fn serde_roundtrip_stream_block() {
    let block = StreamBlock {
        id: 42,
        turn_number: 3,
        created_at_ms: 1_700_000_000_000,
        collapsed: true,
        cost_usd: None,
        role: TurnRole::System,
    };
    let json = serde_json::to_string(&block).unwrap();
    let back: StreamBlock = serde_json::from_str(&json).unwrap();
    assert_eq!(back, block);
}

#[test]
fn text_style_count_matches_variants() {
    // If a new variant is added to TextStyle, COUNT must be updated.
    // This test ensures from_discriminant covers all variants.
    // The last valid discriminant should be COUNT - 1.
    assert!(TextStyle::from_discriminant((TextStyle::COUNT - 1) as u8).is_some());
    assert!(TextStyle::from_discriminant(TextStyle::COUNT as u8).is_none());
}
