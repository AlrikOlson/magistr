use super::*;
use crate::medium::Medium;

/// A LED is a reading: one 9px surface, no id, no focus, no pointer
/// face, and the two states are two different fills.
#[test]
fn a_led_is_one_inert_light() {
    let lex = Lexicon::of(Medium::Night);
    for on in [true, false] {
        let led = Led::el(on, &lex);
        let mut nodes = 0;
        led.each(&mut |_, element| {
            nodes += 1;
            assert!(element.id.is_none() && !element.focusable);
            assert!(element.hover_fill.is_none() && element.press_fill.is_none());
        });
        assert_eq!(nodes, 1, "a light is one surface");
    }
    assert_ne!(
        Led::el(true, &lex).fill,
        Led::el(false, &lex).fill,
        "lit and dark are different fills"
    );
}

/// Lit is the success hue, dark the second surface step — the volume's
/// own `.gled` pair.
#[test]
fn the_two_states_wear_the_volumes_colors() {
    let lex = Lexicon::of(Medium::Day);
    assert_eq!(Led::el(true, &lex).fill, Some(lex.green));
    assert_eq!(Led::el(false, &lex).fill, Some(lex.s1));
}
