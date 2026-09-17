use super::*;

use crate::element::Kind;
use crate::element_layout::{ElementLayout, shape_voice};
use crate::fonts::EmbeddedFonts;
use crate::medium::Medium;
use crate::text::TextEngine;

/// The title is the display voice in the full ink: serif, 24 px, and
/// nothing else on the element — the design's brand moment, not a
/// heading in a bigger size.
#[test]
fn the_title_wears_the_display_voice_in_the_full_ink() {
    let lex = Lexicon::of(Medium::Night);
    let el = ChapterTitle::el("The design language, decided live", &lex);
    let Kind::Text { voice, content } = &el.kind else {
        panic!("a text leaf");
    };
    assert_eq!(*voice, Voice::DISPLAY);
    assert!(voice.is_serif());
    assert!(voice.px >= 20.0, "the serif is never set under 20 px");
    assert_eq!(content, "The design language, decided live");
    assert_eq!(el.fill, Some(lex.tx));
}

/// The title shapes in the embedded serif face — the eighth face, not
/// the sans at a display size, and plain, not engraved — and wraps at
/// its measure like prose rather than clipping like a chip.
#[test]
fn the_title_shapes_in_the_serif_and_wraps_at_its_measure() {
    let lex = Lexicon::of(Medium::Day);
    let mut text = TextEngine::embedded();
    let page = || {
        Element::surface()
            .align_start()
            .child(ChapterTitle::el("The design language, decided live", &lex))
    };
    let narrow = ElementLayout::compute(&page(), 200.0, 65536.0, &mut text).expect("layout");
    let wide = ElementLayout::compute(&page(), 800.0, 65536.0, &mut text).expect("layout");
    let [_, _, _, narrow_h] = narrow.rect(1);
    let [_, _, _, wide_h] = wide.rect(1);
    assert!(narrow_h > wide_h, "a narrow measure wraps the title");
    let run = shape_voice(&mut text, &Voice::DISPLAY, "Chapter");
    assert_eq!(run.set.font, EmbeddedFonts::DISPLAY);
    assert_eq!(run.set.features, qs_text::Features::default());
}
