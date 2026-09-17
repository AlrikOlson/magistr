use super::*;
use crate::medium::Medium;

/// The bar is a 3px track with the share filled in the success hue,
/// clamped, and read-only.
#[test]
fn the_bar_fills_its_share() {
    let lex = Lexicon::of(Medium::Night);
    let bar = ThinBar::el(0.15, 150.0, &lex);
    assert_eq!(bar.height, Some(3.0));
    assert_eq!(bar.fill, Some(lex.s0));
    let mut fill_width = None;
    bar.each(&mut |_, element| {
        assert!(element.id.is_none() && !element.focusable);
        if element.fill == Some(lex.green) {
            fill_width = element.width;
        }
    });
    assert_eq!(fill_width, Some(150.0 * 0.15));
    let over = ThinBar::el(7.0, 100.0, &lex);
    let mut fill_width = None;
    over.each(&mut |_, element| {
        if element.fill == Some(lex.green) {
            fill_width = element.width;
        }
    });
    assert_eq!(fill_width, Some(100.0), "the share clamps at the track");
}
