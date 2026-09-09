use super::*;

#[test]
fn the_plain_underline_is_a_single_rule() {
    assert_eq!(Attrs::UNDERLINE.underline(), Some(Underline::Single));
}

#[test]
fn nothing_underlined_wears_no_style() {
    assert_eq!(Attrs::NONE.underline(), None);
    assert_eq!((Attrs::BOLD | Attrs::ITALIC).underline(), None);
}

#[test]
fn every_styled_constant_round_trips_and_carries_the_underline_bit() {
    for (attrs, style) in [
        (Attrs::UNDERLINE, Underline::Single),
        (Attrs::UNDERLINE_DOUBLE, Underline::Double),
        (Attrs::UNDERLINE_CURLY, Underline::Curly),
        (Attrs::UNDERLINE_DOTTED, Underline::Dotted),
        (Attrs::UNDERLINE_DASHED, Underline::Dashed),
    ] {
        assert_eq!(attrs.underline(), Some(style), "{style:?}");
        assert!(
            attrs.contains(Attrs::UNDERLINE),
            "{style:?} is underlined, whatever the terminal can draw"
        );
    }
}

#[test]
fn a_style_survives_the_flags_laid_beside_it() {
    let attrs = Attrs::UNDERLINE_CURLY | Attrs::BOLD | Attrs::ITALIC;
    assert_eq!(attrs.underline(), Some(Underline::Curly));
    assert!(attrs.contains(Attrs::BOLD) && attrs.contains(Attrs::ITALIC));
}

#[test]
fn laying_a_style_on_replaces_the_one_there() {
    let attrs = Attrs::UNDERLINE_DOTTED | Attrs::UNDERLINE_CURLY;
    assert_eq!(attrs.underline(), Some(Underline::Curly));
    let plain = Attrs::UNDERLINE_CURLY | Attrs::UNDERLINE;
    assert_eq!(
        plain.underline(),
        Some(Underline::Curly),
        "the plain bit names no style, so it cannot erase one"
    );
}

#[test]
fn contains_tells_two_styles_apart() {
    assert!(!Attrs::UNDERLINE_DOTTED.contains(Attrs::UNDERLINE_CURLY));
    assert!(Attrs::UNDERLINE_CURLY.contains(Attrs::UNDERLINE_CURLY));
    assert!(!Attrs::UNDERLINE.contains(Attrs::UNDERLINE_CURLY));
}

#[test]
fn or_assign_agrees_with_or() {
    let mut attrs = Attrs::UNDERLINE_DOTTED;
    attrs |= Attrs::BOLD;
    assert_eq!(attrs, Attrs::UNDERLINE_DOTTED | Attrs::BOLD);
    assert_eq!(attrs.underline(), Some(Underline::Dotted));
}

#[test]
fn the_six_flags_keep_their_bits() {
    let every = Attrs::BOLD
        | Attrs::DIM
        | Attrs::ITALIC
        | Attrs::UNDERLINE
        | Attrs::REVERSE
        | Attrs::STRIKETHROUGH;
    for flag in [
        Attrs::BOLD,
        Attrs::DIM,
        Attrs::ITALIC,
        Attrs::UNDERLINE,
        Attrs::REVERSE,
        Attrs::STRIKETHROUGH,
    ] {
        assert!(every.contains(flag), "{flag:?}");
    }
    assert!(!every.is_empty());
    assert!(Attrs::NONE.is_empty());
}

#[test]
fn without_clears_the_named_flags_and_an_underlines_style_with_its_bit() {
    let all = Attrs::BOLD | Attrs::REVERSE | Attrs::UNDERLINE_CURLY;
    assert_eq!(
        all.without(Attrs::REVERSE),
        Attrs::BOLD | Attrs::UNDERLINE_CURLY
    );
    assert_eq!(all.without(Attrs::UNDERLINE), Attrs::BOLD | Attrs::REVERSE);
    assert_eq!(all.without(Attrs::UNDERLINE).underline(), None);
    assert_eq!(
        Attrs::BOLD.without(Attrs::DIM),
        Attrs::BOLD,
        "a flag not set is nothing to clear"
    );
    assert_eq!(all.without(Attrs::NONE), all);
}
