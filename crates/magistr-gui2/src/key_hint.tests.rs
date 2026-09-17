use super::*;

/// The pair is a cap and a label — two text runs, the cap on its key
/// face — and it is read-only.
#[test]
fn the_pair_is_a_cap_and_a_label() {
    for medium in Medium::ALL {
        let lex = Lexicon::of(medium);
        let hint = KeyHint::el("\u{23ce}", "Open", medium, lex.base, &lex);
        let mut runs = Vec::new();
        let mut beds = 0;
        hint.each(&mut |_, element| {
            assert!(element.id.is_none() && !element.focusable && !element.dashed);
            if let crate::element::Kind::Text { content, .. } = &element.kind {
                runs.push(content.clone());
            }
            if element.fill.is_some() && element.stroke.is_some() {
                beds += 1;
            }
        });
        assert_eq!(runs, ["\u{23ce}", "Open"]);
        assert_eq!(beds, 1, "one key face under the cap");
    }
}

/// The cap's well is darker than the ground in the night and a wash in
/// the day — the volume's own kbd pair — and its ink is dimmed below
/// the label's.
#[test]
fn the_cap_face_follows_the_medium() {
    for medium in Medium::ALL {
        let lex = Lexicon::of(medium);
        let hint = KeyHint::el("A", "Allow", medium, lex.base, &lex);
        let mut bed = None;
        hint.each(&mut |_, element| {
            if element.fill.is_some() && element.stroke.is_some() {
                bed = element.fill;
            }
        });
        let bed = bed.expect("the key face");
        match medium {
            Medium::Night => assert!(bed.r < lex.base.r, "a well in the night"),
            Medium::Day => assert!(bed.r < lex.base.r, "a lilac wash sits under day too"),
        }
    }
}
