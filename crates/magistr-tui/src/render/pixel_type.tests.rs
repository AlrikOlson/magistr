use super::*;
use magistr_cockpit::layout::{Measure, TextMetrics, TextRole};
use magistr_cockpit::pixel::CellSize;

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
fn missing_cjk_glyphs_have_real_distinct_outlines_and_counted_columns() {
    let mut face = PixelType::default();
    let missing = face.glyph('\u{10ffff}', TextRole::Prose, false).1.clone();
    let mut seen = Vec::new();
    for ch in "中文漢字かなカナ한국".chars() {
        let (metrics, bitmap) = face.glyph(ch, TextRole::Prose, false);
        assert!(metrics.width > 0 && metrics.height > 0, "{ch}");
        assert!(bitmap.iter().any(|coverage| *coverage > 0), "{ch}");
        assert_ne!(bitmap, &missing, "{ch} must not use the missing-glyph box");
        assert!(!seen.contains(bitmap), "{ch} must have its own outline");
        seen.push(bitmap.clone());
    }
    face.set_cell(CellSize::new(13, 29));
    assert_eq!(face.advance("中文", TextRole::Data), 52.0);
    assert_eq!(face.advance("a中b", TextRole::Data), 52.0);
}

#[test]
fn changing_terminal_metrics_resizes_the_pixel_type() {
    let mut face = PixelType::default();
    let small = face.advance("Message", TextRole::Label);
    face.set_cell(CellSize::new(20, 40));
    assert!((face.advance("Message", TextRole::Label) - small * 2.0).abs() < 0.01);
}
