use super::*;
use magistr_cockpit::turn::{Span, Voice as TurnVoice};
#[test]
fn native_spans_keep_their_type_and_words() {
    let lex = Lexicon::of(crate::Medium::Night);
    let row = Row::styled(
        RowKind::Prose(TurnVoice::Agent),
        "bold italic code",
        vec![
            Span::new(0, 4, SpanStyle::Bold),
            Span::new(5, 11, SpanStyle::Italic),
            Span::new(12, 16, SpanStyle::Code),
        ],
    );
    let el = RichText::row(&row, lex.tx, &lex);
    let mut words = Vec::new();
    el.each(&mut |_, e| {
        if let crate::element::Kind::Text { voice, content } = &e.kind {
            words.push((content.clone(), *voice));
        }
    });
    assert!(words.iter().any(|(s, v)| s == "bold" && v.weight == 700));
    assert!(words.iter().any(|(s, v)| s == "italic" && v.is_italic()));
    assert!(words.iter().any(|(s, v)| s == "code" && v.is_mono()));
    assert_eq!(
        words.iter().map(|(s, _)| s.as_str()).collect::<String>(),
        "bold italic code"
    );
}

#[test]
fn highlighted_code_uses_readable_syntax_colors_in_both_themes() {
    use magistr_cockpit::render::Color;
    for medium in [crate::Medium::Night, crate::Medium::Day] {
        let lex = Lexicon::of(medium);
        let row = Row::styled(
            RowKind::Code,
            "fn ",
            vec![Span::new(0, 2, SpanStyle::Fg(Color::Rgb(120, 80, 150)))],
        );
        let root = RichText::row(&row, lex.tx, &lex);
        let mut painted = None;
        root.each(&mut |_, e| {
            if let crate::element::Kind::Text { content, .. } = &e.kind
                && content == "fn"
            {
                painted = e.fill;
            }
        });
        let ink = painted.expect("the keyword is painted");
        assert_ne!(ink, lex.tx, "syntax is not discarded");
        let color = |c: Srgba| {
            magistr_theme::ThemeColor::new(
                (c.r * 255.0).round() as u8,
                (c.g * 255.0).round() as u8,
                (c.b * 255.0).round() as u8,
            )
        };
        assert!(magistr_theme::math::contrast_ratio(color(ink), color(lex.mantle)) >= 4.4);
    }
}
