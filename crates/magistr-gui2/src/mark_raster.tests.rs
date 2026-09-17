use super::*;

use crate::sigil::Sigil;

/// How much ink a rasterization carries, as a share of its pixels.
fn coverage(image: &RgbaImage) -> f32 {
    let total: u32 = image
        .rgba
        .chunks_exact(4)
        .map(|pixel| u32::from(pixel[3]))
        .sum();
    total as f32 / (image.rgba.len() / 4) as f32 / 255.0
}

/// The two sizes the design's `.ic` and `.ic.lg` declare, and the two
/// the acceptance names.
const SIZES: [u32; 2] = [13, 24];

/// Every glyph rasterizes at both sizes and puts ink on the page. A
/// glyph that rasterized blank would be indistinguishable from a
/// missing one, which is the failure this set is most exposed to.
#[test]
fn every_glyph_rasterizes_with_ink_at_both_sizes() {
    for icon in Icon::ALL {
        for px in SIZES {
            let image = mark_raster(icon.shapes(), px, (255, 255, 255))
                .unwrap_or_else(|| panic!("{} at {px} px", icon.name()));
            assert_eq!((image.width, image.height), (px, px));
            assert!(!image.is_malformed());
            let ink = coverage(&image);
            assert!(
                ink > 0.02,
                "{} at {px} px is all but blank ({ink})",
                icon.name()
            );
            assert!(
                ink < 0.85,
                "{} at {px} px is a solid block ({ink})",
                icon.name()
            );
        }
    }
}

/// Every glyph is distinguishable from every other at the *small* size — the one where
/// a 1.5-unit stroke is 1.22 device pixels and a set can collapse into a row of
/// smudges. The volume's one duplicate is the known exception.
#[test]
fn the_set_stays_distinguishable_at_thirteen_pixels() {
    let rendered: Vec<(&str, Vec<u8>)> = Icon::ALL
        .into_iter()
        .map(|icon| {
            let image = mark_raster(icon.shapes(), 13, (255, 255, 255)).expect("it rasterizes");
            (icon.name(), image.rgba)
        })
        .collect();
    let mut same = Vec::new();
    for (index, (name, pixels)) in rendered.iter().enumerate() {
        for (other, other_pixels) in &rendered[index + 1..] {
            if pixels == other_pixels {
                same.push((*name, *other));
            }
        }
    }
    assert_eq!(same, vec![("read", "search")]);
}

/// The signature at the small size: a star-dot is a *filled* disc, so
/// the glyph that wears one carries more ink than the same strokes
/// without it.
#[test]
fn a_star_dot_puts_ink_where_the_stroke_ends() {
    let dotted = mark_raster(Icon::Done.shapes(), 24, (255, 255, 255)).expect("it rasterizes");
    let bare = mark_raster(&[Icon::Done.shapes()[0]], 24, (255, 255, 255)).expect("it rasterizes");
    assert!(
        coverage(&dotted) > coverage(&bare),
        "the check's terminal dot adds ink: {} vs {}",
        coverage(&dotted),
        coverage(&bare)
    );
    // And it lands at the terminal the sprite names — (12.5, 4) on the
    // 16-grid, which at 24 px is (18.75, 6).
    let at = |x: usize, y: usize, image: &RgbaImage| image.rgba[(y * 24 + x) * 4 + 3];
    assert!(at(18, 6, &dotted) > at(18, 6, &bare));
}

/// The pixels are straight alpha in the ink's own colour: a flat fill
/// demultiplies back to exactly the colour it was painted in, which is
/// what the colour page needs.
#[test]
fn the_pixels_are_straight_alpha_in_the_inks_own_colour() {
    let image = mark_raster(Icon::Recording.shapes(), 24, (203, 166, 247)).expect("it rasterizes");
    let opaque = image
        .rgba
        .chunks_exact(4)
        .find(|pixel| pixel[3] == 255)
        .expect("a filled disc has opaque pixels");
    assert_eq!(&opaque[..3], &[203, 166, 247]);
}

/// A rasterization is bigger at the bigger size, and it is a fresh
/// drawing rather than the small one scaled — the geometry is the
/// point of holding the set as vectors.
#[test]
fn the_two_sizes_are_two_rasterizations() {
    let small = mark_raster(Icon::Time.shapes(), 13, (255, 255, 255)).expect("it rasterizes");
    let large = mark_raster(Icon::Time.shapes(), 24, (255, 255, 255)).expect("it rasterizes");
    assert_eq!((small.width, large.width), (13, 24));
    // The stroke is a *share* of the box, so coverage is comparable
    // across sizes rather than growing with the area.
    let ratio = coverage(&large) / coverage(&small);
    assert!((0.5..2.0).contains(&ratio), "coverage ratio {ratio}");
}

/// A derived mark takes the same route as a copied one.
#[test]
fn a_sigil_rasterizes_like_a_glyph() {
    for name in ["magistr", "tidewatch", "think-and-ship", "quicksilver"] {
        let shapes = Sigil::of(name).shapes();
        for px in SIZES {
            let image = mark_raster(&shapes, px, (255, 255, 255))
                .unwrap_or_else(|| panic!("{name} at {px} px"));
            assert!(coverage(&image) > 0.02, "{name} at {px} px is blank");
        }
    }
}

/// Two projects wear two visibly different marks — the claim a
/// structural inequality cannot make. The first specimen sheet had four
/// sigils that compared unequal and looked identical, because every
/// star owned a sector and the chain always ran through them in sector
/// order; the visiting order is derived now, and this is the test that
/// would have caught it.
#[test]
fn a_sigil_is_visibly_a_different_mark_per_project() {
    let drawn: Vec<(&str, Vec<u8>)> = Sigil::VOLUME_PROJECTS
        .into_iter()
        .map(|name| {
            let shapes = Sigil::of(name).shapes();
            (
                name,
                mark_raster(&shapes, 24, (255, 255, 255))
                    .expect("it rasterizes")
                    .rgba,
            )
        })
        .collect();
    for (index, (name, pixels)) in drawn.iter().enumerate() {
        for (other, other_pixels) in &drawn[index + 1..] {
            let differing = pixels
                .chunks_exact(4)
                .zip(other_pixels.chunks_exact(4))
                .filter(|(a, b)| a[3].abs_diff(b[3]) > 24)
                .count();
            assert!(
                differing > 40,
                "{name} and {other} differ in only {differing} of 576 pixels"
            );
        }
    }
}

/// Nothing to draw, or nowhere to draw it, is `None` rather than an
/// empty picture the atlas would admit for ever.
#[test]
fn nothing_to_draw_is_no_rasterization() {
    assert!(mark_raster(&[], 24, (255, 255, 255)).is_none());
    assert!(mark_raster(Icon::Run.shapes(), 0, (255, 255, 255)).is_none());
    assert!(mark_raster(&[Shape::Stroke("M0 0 C1 1 2 2 3 3")], 24, (255, 255, 255)).is_none());
}
