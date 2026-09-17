use super::*;
use magistr_theme::{catppuccin_latte, catppuccin_mocha};
use qs_gpu::frame::PrimKind;

fn mocha() -> Palette {
    catppuccin_mocha()
}

fn latte() -> Palette {
    catppuccin_latte()
}

fn ground_of(level: Elevation, palette: &Palette) -> Srgba {
    let base = channels(palette.base);
    let band = Srgba::new(base[0], base[1], base[2], 1.0);
    level.ground(band, palette)
}

fn luminance(colour: Srgba) -> f32 {
    0.2126 * colour.r + 0.7152 * colour.g + 0.0722 * colour.b
}

#[test]
fn the_canvas_and_the_bars_have_no_depth_at_all() {
    // Canvas and edge strips are flat: elevation adds neither shadows nor rims.
    {
        let flat = Elevation::canvas();
        assert!(flat.is_flat());
        assert!(flat.shadows([0.0, 0.0, 100.0, 20.0], 6.0, 1.0).is_empty());
        assert!(flat.rim([0.0, 0.0, 100.0, 20.0], 6.0, 1.0).is_none());
        let band = Srgba::new(0.1, 0.2, 0.3, 1.0);
        assert_eq!(flat.ground(band, &mocha()), band);
    }
}

#[test]
fn a_surface_lifts_lighter_by_its_own_palettes_step() {
    // The blessed captures float their surfaces toward the light in
    // both media — a day card is *whiter* than the grey page, not
    // greyer (view 05's edit card, the composer pill). The palette
    // still owns the step's size: Mocha's `surface0` is lighter than
    // its `base` and Latte's is darker, and the lift takes that step's
    // magnitude with the design's direction.
    assert!(Elevation::lifts_lighter(&mocha()));
    assert!(!Elevation::lifts_lighter(&latte()));
    for palette in [&mocha(), &latte()] {
        let canvas = luminance(ground_of(Elevation::canvas(), palette));
        let card = luminance(ground_of(Elevation::row(), palette));
        let pane = luminance(ground_of(Elevation::raised(), palette));
        let sheet = luminance(ground_of(Elevation::popover(), palette));
        assert!(card > canvas, "{}: card", palette.name);
        assert!(pane > card, "{}: pane", palette.name);
        assert!(sheet > pane, "{}: sheet", palette.name);
    }
}

#[test]
fn a_lifted_surface_never_reaches_the_step_the_palette_already_paints_panels_in() {
    // A popover that moved a full `base`→`surface0` step would land on
    // the colour the panels are already painted in and stop reading as
    // over them.
    for palette in [&mocha(), &latte()] {
        let step = (luminance(ground_of(Elevation::popover(), palette))
            - luminance(ground_of(Elevation::canvas(), palette)))
        .abs();
        let base = channels(palette.base);
        let up = channels(palette.surface0);
        let full =
            (0.2126 * (up[0] - base[0]) + 0.7152 * (up[1] - base[1]) + 0.0722 * (up[2] - base[2]))
                .abs();
        assert!(step < full, "{}: {step} vs {full}", palette.name);
        assert!(step > 0.0, "{}: no lift at all", palette.name);
    }
}

#[test]
fn depth_is_two_shadows_and_a_lit_edge_rather_than_a_border() {
    // asks for a "soft two-layer shadow" and refuses a 1px border on a card. Two glows
    // and a rim is what that is: the rim reaches *inward* from the boundary, so it puts
    // no ink outside the shape and is a lit edge rather than a stroke.
    let card = Elevation::row();
    let shadows = card.shadows([10.0, 20.0, 200.0, 40.0], 6.0, 1.0);
    assert_eq!(shadows.len(), 2);
    for shadow in &shadows {
        assert_eq!(PrimKind::from_raw(shadow.kind), Some(PrimKind::Glow));
    }
    let rim = card
        .rim([10.0, 20.0, 200.0, 40.0], 6.0, 1.0)
        .expect("a rim");
    assert_eq!(PrimKind::from_raw(rim.kind), Some(PrimKind::Rim));
}

#[test]
fn the_higher_a_surface_the_further_and_softer_its_shadow_falls() {
    let rect = [0.0, 0.0, 100.0, 20.0];
    let drop = |level: Elevation| {
        let shadows = level.shadows(rect, 6.0, 1.0);
        // The ambient layer is first; its offset is what "how far off" is
        // actually said with.
        shadows[0].rect[1]
    };
    let (card, pane, sheet) = (
        drop(Elevation::row()),
        drop(Elevation::raised()),
        drop(Elevation::popover()),
    );
    assert!(
        card > 0.0 && pane > card && sheet > pane,
        "{card} {pane} {sheet}"
    );
}

#[test]
fn the_grid_scale_carries_the_depth_so_a_hidpi_card_is_lifted_by_the_same_amount() {
    let rect = [0.0, 0.0, 100.0, 20.0];
    let one = Elevation::raised().shadows(rect, 6.0, 1.0);
    let two = Elevation::raised().shadows(rect, 6.0, 2.0);
    assert!((two[0].rect[1] - one[0].rect[1] * 2.0).abs() < 1e-4);
}
