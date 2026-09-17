use qs_gpu::color::Srgba;

use super::*;
use crate::icon::Icon;
use crate::mark::Mark;

/// A minimal tree with a mark on it: the still converges (the mark's
/// picture is admitted and drawn) and two runs are byte-identical.
#[test]
fn a_still_with_a_mark_is_deterministic_and_not_blank() {
    let tree = || {
        Element::surface()
            .fill(Srgba::new(0.1, 0.1, 0.15, 1.0))
            .pad(4.0)
            .child(Element::mark(Mark::Icon(Icon::Done), 16, (255, 255, 255)))
    };
    let a = atom_still(&tree(), &ElementState::default(), Medium::Night, 40, 30)
        .expect("the still renders");
    let b = atom_still(&tree(), &ElementState::default(), Medium::Night, 40, 30)
        .expect("the still renders");
    assert_eq!(a.data(), b.data(), "two fresh stills are the same bytes");
    let blank = atom_still(
        &Element::surface().fill(Srgba::new(0.1, 0.1, 0.15, 1.0)),
        &ElementState::default(),
        Medium::Night,
        40,
        30,
    )
    .expect("the bare still renders");
    assert_ne!(a.data(), blank.data(), "the mark put ink on the page");
}

/// The canvas outside the tree is the medium's own base — the ground
/// the specimen captures were cut over.
#[test]
fn the_canvas_is_the_mediums_base() {
    let empty = Element::surface();
    let night = atom_still(&empty, &ElementState::default(), Medium::Night, 8, 8)
        .expect("the still renders");
    let px = night.pixel(0, 0).expect("a pixel");
    assert_eq!(
        (px.red(), px.green(), px.blue()),
        (0x16, 0x16, 0x22),
        "night's ground is the volume's own base"
    );
    let day =
        atom_still(&empty, &ElementState::default(), Medium::Day, 8, 8).expect("the still renders");
    let px = day.pixel(0, 0).expect("a pixel");
    assert_eq!((px.red(), px.green(), px.blue()), (0xf1, 0xf0, 0xf5));
}

/// A zero-sided viewport is refused, never a panic.
#[test]
fn a_zero_viewport_is_refused() {
    let outcome = atom_still(
        &Element::surface(),
        &ElementState::default(),
        Medium::Night,
        0,
        10,
    );
    assert!(matches!(outcome, Err(Gui2Error::Raster(0, 10))));
}
