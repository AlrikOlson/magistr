use super::*;

use magistr_cockpit::pixel::CellSize;

const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn assert_uv(got: [f32; 4], want: [f32; 4]) {
    assert!(
        got.iter().zip(want).all(|(g, w)| close(*g, w)),
        "uv {got:?}, wanted {want:?}"
    );
}

/// A red PNG of `width × height`, decoded as the stream decodes one.
fn image(width: u32, height: u32) -> StreamImage {
    let mut pixmap = Pixmap::new(width, height).expect("a pixmap allocates");
    pixmap.fill(tiny_skia::Color::from_rgba8(200, 40, 40, 255));
    let png = pixmap.encode_png().expect("a pixmap encodes");
    StreamImage::decode(&png, "a shot", CellSize::new(9, 20)).expect("a png decodes")
}

#[test]
fn a_picture_its_box_holds_is_drawn_at_its_own_size_from_the_boxs_corner() {
    // A 40×40 picture in a 5-column, 2-row box on 9×20 cells: the box
    // is 45×40, so the picture stands at 1:1 with a sliver spare.
    let placement = PxRect::new(18.0, 60.0, 45.0, 40.0);
    let quad = ImageQuad::place(placement, placement, (40, 40), WHOLE).expect("a quad");
    assert_eq!(quad.rect, PxRect::new(18.0, 60.0, 40.0, 40.0));
    assert_uv(quad.uv, WHOLE);
}

#[test]
fn a_picture_wider_than_its_box_is_fit_with_its_aspect_kept_and_never_grown() {
    // 180×60 in a 90×100 box: the width bites, so the picture halves on
    // both axes rather than filling the box's height.
    let placement = PxRect::new(0.0, 0.0, 90.0, 100.0);
    let quad = ImageQuad::place(placement, placement, (180, 60), WHOLE).expect("a quad");
    assert_eq!(quad.rect, PxRect::new(0.0, 0.0, 90.0, 30.0));
    // 10×10 in a 90×100 box stays 10×10: a tiny picture is not blown up.
    let quad = ImageQuad::place(placement, placement, (10, 10), WHOLE).expect("a quad");
    assert_eq!(quad.rect, PxRect::new(0.0, 0.0, 10.0, 10.0));
}

#[test]
fn a_band_of_the_middle_rows_draws_the_slice_those_rows_hold() {
    // The placement is 40 px tall; the stage shows the rows from 20 to
    // 30 of it — the picture's third quarter.
    let placement = PxRect::new(0.0, 0.0, 40.0, 40.0);
    let band = PxRect::new(0.0, 20.0, 40.0, 10.0);
    let quad = ImageQuad::place(placement, band, (40, 40), WHOLE).expect("a quad");
    assert_eq!(quad.rect, band);
    assert_uv(quad.uv, [0.0, 0.5, 1.0, 0.75]);
    // The entry sits somewhere inside the page: the slice is of *its*
    // rectangle, not of the page.
    let entry = [0.25, 0.5, 0.5, 1.0];
    let quad = ImageQuad::place(placement, band, (40, 40), entry).expect("a quad");
    assert_uv(quad.uv, [0.25, 0.75, 0.5, 0.875]);
}

#[test]
fn a_band_below_a_picture_shorter_than_its_box_draws_nothing() {
    // The row cap rounded the box up to two rows but the picture stands
    // 20 px: the second row holds none of it.
    let placement = PxRect::new(0.0, 0.0, 40.0, 40.0);
    let below = PxRect::new(0.0, 20.0, 40.0, 20.0);
    assert!(ImageQuad::place(placement, below, (40, 20), WHOLE).is_none());
    assert!(ImageQuad::place(PxRect::default(), below, (40, 20), WHOLE).is_none());
    assert!(ImageQuad::place(placement, below, (0, 20), WHOLE).is_none());
}

#[test]
fn admission_is_the_pictures_own_bytes_when_the_page_holds_them() {
    let image = image(40, 60);
    let admitted = ImageQuad::admission(&image, 2048);
    assert_eq!((admitted.width, admitted.height), (40, 60));
    assert_eq!(admitted.rgba, image.rgba());
    assert!(!admitted.is_malformed());
}

#[test]
fn a_picture_the_page_cannot_hold_is_admitted_fit_to_the_page_with_its_aspect_kept() {
    // A 32-texel page keeps 28 for a picture; 40×60 fits by the height.
    let portrait = image(40, 60);
    let admitted = ImageQuad::admission(&portrait, 32);
    assert_eq!((admitted.width, admitted.height), (18, 28));
    assert!(!admitted.is_malformed());
    assert!(
        admitted
            .rgba
            .chunks_exact(4)
            .all(|px| px == [200, 40, 40, 255]),
        "a flat picture scaled is the same flat colour, straight alpha"
    );
    // The other way round: the width bites.
    let landscape = image(60, 40);
    let admitted = ImageQuad::admission(&landscape, 32);
    assert_eq!((admitted.width, admitted.height), (28, 18));
}
