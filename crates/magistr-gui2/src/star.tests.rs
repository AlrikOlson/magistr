use super::*;
use crate::medium::Medium;

fn lex() -> Lexicon {
    Lexicon::of(Medium::Night)
}

/// Every state draws the figure's own geometry: boxes are `2r` for
/// dots and `2r + stroke` for rings, and only the two "nothing can
/// happen" states — pending and blocked — dash.
#[test]
fn every_state_draws_the_figures_geometry() {
    let lex = lex();
    let sizes = |star: &Element| (star.width, star.height);
    assert_eq!(sizes(&Star::pending(&lex)), (Some(10.0), Some(10.0)));
    assert_eq!(sizes(&Star::ready(&lex)), (Some(15.5), Some(15.5)));
    assert_eq!(sizes(&Star::executing(&lex)), (Some(10.0), Some(10.0)));
    assert_eq!(sizes(&Star::stuck(&lex)), (Some(9.0), Some(9.0)));
    assert_eq!(sizes(&Star::blocked(&lex)), (Some(16.0), Some(16.0)));
    assert_eq!(sizes(&Star::done(&lex)), (Some(6.0), Some(6.0)));
    assert_eq!(sizes(&Star::hue(lex.blue)), (Some(8.0), Some(8.0)));
    let dashes = |star: &Element| {
        let mut n = 0;
        star.each(&mut |_, element| {
            if element.dashed {
                n += 1;
            }
        });
        n
    };
    assert_eq!(dashes(&Star::pending(&lex)), 1);
    assert_eq!(dashes(&Star::blocked(&lex)), 1);
    for settled in [
        Star::ready(&lex),
        Star::executing(&lex),
        Star::stuck(&lex),
        Star::done(&lex),
    ] {
        assert_eq!(dashes(&settled), 0);
    }
}

/// The compound states carry their core dot centered in the ring, and
/// every star is read-only.
#[test]
fn compound_states_center_their_core() {
    let lex = lex();
    for star in [Star::ready(&lex), Star::blocked(&lex)] {
        let side = star.width.expect("a star knows its size");
        let mut core = None;
        star.each(&mut |_, element| {
            assert!(element.id.is_none() && !element.focusable);
            if element.pin[0].is_some() {
                core = Some((element.pin[0], element.width));
            }
        });
        let (pin, width) = core.expect("the core dot");
        assert_eq!(pin, Some((side - 5.0) / 2.0));
        assert_eq!(width, Some(5.0));
    }
}
