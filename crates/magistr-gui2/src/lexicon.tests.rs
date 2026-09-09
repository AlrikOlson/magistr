use super::*;

fn bytes(c: Srgba) -> (u8, u8, u8) {
    let b = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    (b(c.r), b(c.g), b(c.b))
}

/// The night table round-trips the volume's own hex — the ground the
/// specimen corner pixels show, and the accent every capture wears.
#[test]
fn the_night_palette_is_the_volumes_own_hex() {
    let night = Lexicon::of(Medium::Night);
    assert_eq!(bytes(night.base), (0x16, 0x16, 0x22));
    assert_eq!(bytes(night.mauve), (0xc7, 0xa4, 0xf5));
    assert_eq!(bytes(night.green), (0xa3, 0xe0, 0xa0));
    assert!((night.hairc.a - 0.4).abs() < 1e-6);
    assert_eq!(bytes(night.hairc), (0x41, 0x41, 0x60), "hairc is s1 at 40%");
}

/// Day is its own palette, not inverted night.
#[test]
fn the_day_palette_is_its_own_and_not_inverted_night() {
    let day = Lexicon::of(Medium::Day);
    assert_eq!(bytes(day.base), (0xf1, 0xf0, 0xf5));
    assert_eq!(bytes(day.mauve), (0x7d, 0x3f, 0xd0));
    assert!((day.hairc.a - 0.34).abs() < 1e-6);
    let night = Lexicon::of(Medium::Night);
    // Inverting night's base would give #e9e9dd, which is not day's.
    assert_ne!(
        bytes(day.base),
        (255 - 0x16, 255 - 0x16, 255 - 0x22),
        "day must not be night through a NOT gate"
    );
    assert_ne!(bytes(day.mauve), bytes(night.mauve));
}

/// The sixteen ANSI colours are the medium's own tokens in SGR order —
/// the surface steps for the blacks, the subdued and text inks for the
/// whites, the six state codes for the hues (mauve as magenta, teal as
/// cyan), the bright hues the same six — and day's are not night's.
#[test]
fn the_ansi_sixteen_are_the_mediums_own_tokens() {
    for medium in [Medium::Night, Medium::Day] {
        let lex = Lexicon::of(medium);
        let a = lex.ansi.map(bytes);
        assert_eq!(a[0], bytes(lex.s0), "{medium:?} black");
        assert_eq!(a[8], bytes(lex.s1), "{medium:?} bright black");
        assert_ne!(a[0], a[8]);
        assert_eq!(a[7], bytes(lex.sb1), "{medium:?} white");
        assert_eq!(a[15], bytes(lex.tx), "{medium:?} bright white");
        assert_eq!(a[1], bytes(lex.red));
        assert_eq!(a[2], bytes(lex.green));
        assert_eq!(a[3], bytes(lex.yellow));
        assert_eq!(a[4], bytes(lex.blue));
        assert_eq!(a[5], bytes(lex.mauve), "magenta is the accent");
        assert_eq!(a[6], bytes(lex.teal), "cyan is the teal");
        assert_eq!(&a[9..15], &a[1..7], "the bright hues are the same six");
    }
    assert_ne!(
        Lexicon::of(Medium::Night).ansi.map(bytes),
        Lexicon::of(Medium::Day).ansi.map(bytes)
    );
}
