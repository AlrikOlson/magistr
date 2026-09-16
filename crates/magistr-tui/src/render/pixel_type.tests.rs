use super::*;

#[test]
fn ui_text_has_proportional_pixel_advances_and_code_keeps_columns() {
    let face = PixelType::default();
    assert!(face.advance("WWW", TextRole::Label) > face.advance("iii", TextRole::Label) * 2.0);
    assert_eq!(
        face.advance("WWW", TextRole::Data),
        face.advance("iii", TextRole::Data)
    );
    assert_eq!(face.advance("漢字", TextRole::Data), 40.0);
}

#[test]
fn measured_labels_fit_both_weights_without_a_rounded_off_last_letter() {
    let mut face = PixelType::default();
    for word in ["Session", "Magistr", "harbor", "Send", "Verify"] {
        let text = TextMetrics::new(word);
        let (width, _) = face.intrinsic(&text);
        assert_eq!(face.wrap(&text, width), [word]);
        let drawn: f32 = word
            .chars()
            .map(|ch| face.glyph(ch, TextRole::Label, true).0.advance_width)
            .sum();
        assert!(drawn < width, "{word}: {drawn} / {width}");
    }
}

#[test]
fn changing_terminal_metrics_resizes_the_pixel_type() {
    let mut face = PixelType::default();
    let small = face.advance("Message", TextRole::Label);
    face.set_cell(CellSize::new(20, 40));
    assert!((face.advance("Message", TextRole::Label) - small * 2.0).abs() < 0.01);
}
