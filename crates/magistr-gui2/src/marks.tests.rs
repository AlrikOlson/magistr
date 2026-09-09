use super::*;

use magistr_cockpit::theme::ThemeMode;

fn marks(mode: ThemeMode) -> Marks {
    Marks::for_theme(&Theme::forced(mode))
}

/// A role is what a mark is *drawn in*, not a label on the call: the four roles resolve
/// to real palette ink, and the quiet end of language is quieter than the loud end.
#[test]
fn the_role_picks_the_ink() {
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        let set = marks(mode);
        assert_ne!(
            set.ink(IconRole::Kind),
            set.ink(IconRole::Nav),
            "a classifier and a place are not the same ink"
        );
        // Two roles deliberately share: a nav chip and a utility button
        // are both acted on, and differ in whether a word sits beside
        // the mark rather than in colour.
        assert_eq!(set.ink(IconRole::Nav), set.ink(IconRole::Utility));
        assert_ne!(set.ink(IconRole::Empty), set.ink(IconRole::Nav));
    }
}

/// The two media are two sets of ink, so a still in one says nothing
/// about the other.
#[test]
fn the_two_media_ink_the_set_differently() {
    let (night, day) = (marks(ThemeMode::Dark), marks(ThemeMode::Light));
    for role in IconRole::ALL {
        assert_ne!(night.ink(role), day.ink(role), "{}", role.name());
    }
    assert_ne!(night.ground(), day.ground());
}

/// The specimen's arithmetic: every mark it declares has a place on the
/// sheet, and the sheet is big enough to hold them.
#[test]
fn the_sheet_holds_every_mark_it_declares() {
    assert_eq!(Marks::SPECIMEN, 27 * 2 + 4 * 2 + 4);
    let (width, height) = Marks::sheet();
    for (band, count, px) in [
        (Sheet::GLYPHS_LARGE, Icon::ALL.len(), Marks::LARGE),
        (Sheet::GLYPHS_SMALL, Icon::ALL.len(), Marks::SMALL),
        (Sheet::SIGILS_LARGE, 4, Marks::LARGE),
        (Sheet::SIGILS_SMALL, 4, Marks::SMALL),
        (Sheet::ROLES, IconRole::ALL.len(), Marks::LARGE),
    ] {
        for index in 0..count {
            let (x, y) = Sheet::grid(band, index, px);
            let size = px as f32;
            assert!(x >= 0.0 && y >= 0.0, "({x}, {y})");
            assert!(
                x + size <= width as f32 && y + size <= height as f32,
                "a mark at ({x}, {y}) falls off a {width}x{height} sheet"
            );
        }
    }
}

/// A mark is drawn at exactly the size it was rasterized at, on whole
/// pixels — the reason the set is geometry and not one bitmap a
/// sampler stretches.
#[test]
fn every_mark_lands_on_a_whole_pixel() {
    for index in 0..Icon::ALL.len() {
        for (band, px) in [
            (Sheet::GLYPHS_LARGE, Marks::LARGE),
            (Sheet::GLYPHS_SMALL, Marks::SMALL),
        ] {
            let (x, y) = Sheet::grid(band, index, px);
            assert!((x.fract()).abs() < f32::EPSILON, "{x}");
            assert!((y.fract()).abs() < f32::EPSILON, "{y}");
        }
    }
}

/// The atlas key is what makes a settled sheet cost no upload: the same
/// mark at the same size in the same ink is one entry, and any of the
/// three changing is another.
#[test]
fn a_marks_identity_is_the_mark_the_size_and_the_ink() {
    let a = key((Mark::Icon(Icon::Run), 13u32, (1u8, 2u8, 3u8)));
    assert_eq!(a, key((Mark::Icon(Icon::Run), 13u32, (1u8, 2u8, 3u8))));
    assert_ne!(a, key((Mark::Icon(Icon::Edit), 13u32, (1u8, 2u8, 3u8))));
    assert_ne!(a, key((Mark::Icon(Icon::Run), 24u32, (1u8, 2u8, 3u8))));
    assert_ne!(a, key((Mark::Icon(Icon::Run), 13u32, (9u8, 2u8, 3u8))));
    assert_ne!(a, key((Mark::Sigil("run"), 13u32, (1u8, 2u8, 3u8))));
}
