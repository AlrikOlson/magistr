use tiny_skia::{Pixmap, PremultipliedColorU8};

use super::*;

/// A PNG of `width × height` whose left half is red and right half blue,
/// with the top row green — enough structure that a sampler that lost
/// its coordinates shows it.
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut pixmap = Pixmap::new(width, height).expect("a pixmap allocates");
    let pixels = pixmap.pixels_mut();
    for y in 0..height {
        for x in 0..width {
            let color = if y == 0 {
                PremultipliedColorU8::from_rgba(0, 255, 0, 255)
            } else if x < width / 2 {
                PremultipliedColorU8::from_rgba(255, 0, 0, 255)
            } else {
                PremultipliedColorU8::from_rgba(0, 0, 255, 255)
            };
            pixels[(y * width + x) as usize] = color.expect("opaque colour is premultiplied");
        }
    }
    pixmap.encode_png().expect("a pixmap encodes")
}

fn image(width: u32, height: u32) -> StreamImage {
    StreamImage::decode(&png(width, height), "a screenshot", CellSize::new(10, 20))
        .expect("a png decodes")
}

#[test]
fn decodes_a_png_to_straight_rgba() {
    let image = image(4, 2);
    assert_eq!((image.width(), image.height()), (4, 2));
    assert_eq!(image.alt(), "a screenshot");
    assert_eq!(image.rgba().len(), 4 * 2 * 4);
    // The top-left pixel is the green top row.
    assert_eq!(&image.rgba()[0..4], &[0, 255, 0, 255]);
}

#[test]
fn refuses_what_is_not_an_image_this_build_reads() {
    assert!(StreamImage::decode(b"not an image", "alt", CellSize::DEFAULT).is_none());
    assert!(StreamImage::decode(&[], "alt", CellSize::DEFAULT).is_none());
    // A GIF: a real image, in a format this build has no decoder for.
    assert!(StreamImage::decode(b"GIF89a\0\0\0\0", "alt", CellSize::DEFAULT).is_none());
    // A JPEG magic with nothing behind it: sniffed as a JPEG, then
    // refused by the decoder rather than trusted for being labelled one.
    assert!(StreamImage::decode(&[0xFF, 0xD8, 0xFF, 0xE0], "alt", CellSize::DEFAULT).is_none());
    // A RIFF container that is not a WebP.
    assert!(StreamImage::decode(b"RIFF\0\0\0\0AVI ", "alt", CellSize::DEFAULT).is_none());
}

/// 40x60, left half red and right half blue, encoded by three real
/// encoders — see `fixtures/README.md`.
const JPEG: &[u8] = include_bytes!("../../fixtures/half.jpg");
const LOSSY_WEBP: &[u8] = include_bytes!("../../fixtures/lossy.webp");
const ALPHA_WEBP: &[u8] = include_bytes!("../../fixtures/alpha.webp");
const WIDE_WEBP: &[u8] = include_bytes!("../../fixtures/wide.webp");

/// A channel a lossy codec landed within `slack` of where it was put.
#[track_caller]
fn near(got: u8, want: u8, slack: i32) {
    let off = i32::from(got) - i32::from(want);
    assert!(off.abs() <= slack, "{got} is not within {slack} of {want}");
}

#[test]
fn a_jpeg_decodes_the_way_a_png_does() {
    let image =
        StreamImage::decode(JPEG, "a photo", CellSize::new(10, 20)).expect("a jpeg decodes");
    assert_eq!((image.width(), image.height()), (40, 60));
    assert_eq!(image.alt(), "a photo");
    assert_eq!(
        image.rgba().len(),
        40 * 60 * 4,
        "four channels, alpha filled in"
    );
    // The same 40x60 the PNG test measures, so the footprint is the same.
    assert_eq!(image.footprint(80), (4, 3));
    // And the sampler reads it left to right, as it reads a PNG.
    let row = image.halfblocks(4, 3, 1);
    near(row[0].0.0, 255, 12);
    near(row[0].0.2, 0, 12);
    near(row[3].0.2, 255, 12);
    near(row[3].0.0, 0, 12);
}

#[test]
fn a_lossy_webp_decodes_and_its_missing_alpha_is_filled_in() {
    let image =
        StreamImage::decode(LOSSY_WEBP, "a frame", CellSize::new(10, 20)).expect("a webp decodes");
    assert_eq!((image.width(), image.height()), (40, 60));
    assert_eq!(image.footprint(80), (4, 3));
    // VP8 hands back RGB; every pixel comes out opaque, not transparent.
    assert_eq!(image.rgba().len(), 40 * 60 * 4);
    assert!(
        image.rgba().chunks_exact(4).all(|px| px[3] == 255),
        "a webp with no alpha channel is opaque, not empty"
    );
    let row = image.halfblocks(4, 3, 1);
    near(row[0].0.0, 255, 12);
    near(row[3].0.2, 255, 12);
}

#[test]
fn a_lossless_webps_straight_alpha_survives() {
    // 2x1: half-alpha red beside opaque blue.
    let image =
        StreamImage::decode(ALPHA_WEBP, "a badge", CellSize::DEFAULT).expect("a webp decodes");
    assert_eq!((image.width(), image.height()), (2, 1));
    assert_eq!(image.rgba(), &[255, 0, 0, 128, 0, 0, 255, 255]);
    // Composited over black, half-alpha red reads as half-bright red.
    let row = image.halfblocks(2, 1, 0);
    assert_eq!(row[0].0, (128, 0, 0));
    assert_eq!(row[1].0, (0, 0, 255));
}

#[test]
fn an_image_past_the_pixel_ceiling_is_refused_from_its_header() {
    // 16383x1 in 44 bytes: the ceiling is read off the WebP header, so
    // the refusal costs nothing a decode bomb could grow.
    assert!(WIDE_WEBP.len() < 128, "the fixture is a header, not pixels");
    assert!(StreamImage::decode(WIDE_WEBP, "alt", CellSize::DEFAULT).is_none());
}

#[test]
fn a_small_image_keeps_its_natural_size() {
    // 40×60 px at 10×20 px cells is 4 columns by 3 rows.
    let image = image(40, 60);
    assert_eq!(image.footprint(80), (4, 3));
}

#[test]
fn a_wide_image_shrinks_to_the_stage_width_keeping_its_aspect() {
    // 800×400 px is 80×20 cells naturally; given 40 columns it halves.
    let image = image(800, 400);
    assert_eq!(image.footprint(40), (40, 10));
}

#[test]
fn a_tall_image_is_capped_at_the_row_ceiling() {
    // 100×2000 px is 10×100 cells naturally; the ceiling scales it down.
    let image = image(100, 2000);
    let (cols, rows) = image.footprint(80);
    assert_eq!(rows, MAX_IMAGE_ROWS);
    assert_eq!(cols, 2, "the aspect is kept as the height is cut");
}

#[test]
fn no_columns_means_no_footprint() {
    assert_eq!(image(40, 60).footprint(0), (0, 0));
}

#[test]
fn a_footprint_is_never_zero_on_a_side() {
    // One pixel is less than a cell on both axes and still takes one.
    let image = image(1, 1);
    assert_eq!(image.footprint(80), (1, 1));
}

#[test]
fn a_new_cell_size_re_measures_the_footprint() {
    let mut image = image(40, 60);
    assert_eq!(image.footprint(80), (4, 3));
    image.set_cell_size(CellSize::new(20, 40));
    assert_eq!(image.cell_size(), CellSize::new(20, 40));
    assert_eq!(image.footprint(80), (2, 2), "bigger cells, fewer of them");
}

#[test]
fn halfblocks_sample_the_image_left_to_right() {
    let image = image(40, 40);
    let row = image.halfblocks(4, 2, 1);
    assert_eq!(row.len(), 4);
    // The bottom half of the image: red on the left, blue on the right.
    assert_eq!(row[0].0, (255, 0, 0));
    assert_eq!(row[3].0, (0, 0, 255));
}

#[test]
fn halfblocks_read_two_image_rows_per_cell_row() {
    // 2 px tall over 1 cell row: the green top row over the blue bottom.
    let image = image(1, 2);
    let row = image.halfblocks(1, 1, 0);
    assert_eq!(row, vec![((0, 255, 0), (0, 0, 255))]);
}

#[test]
fn halfblocks_outside_the_placement_are_empty() {
    let image = image(40, 40);
    assert!(image.halfblocks(4, 2, 2).is_empty(), "past the last row");
    assert!(image.halfblocks(0, 2, 0).is_empty(), "no columns");
    assert!(image.halfblocks(4, 0, 0).is_empty(), "no rows");
}

#[test]
fn a_transparent_pixel_composites_over_black() {
    let mut pixmap = Pixmap::new(1, 1).expect("a pixmap allocates");
    pixmap.pixels_mut()[0] =
        PremultipliedColorU8::from_rgba(64, 0, 0, 128).expect("premultiplied red");
    let png = pixmap.encode_png().expect("a pixmap encodes");
    let image = StreamImage::decode(&png, "alt", CellSize::DEFAULT).expect("a png decodes");
    let (upper, _) = image.halfblocks(1, 1, 0)[0];
    assert_eq!(upper.0, 64, "half-alpha red reads as half-bright red");
    assert_eq!((upper.1, upper.2), (0, 0));
}

#[test]
fn the_row_cap_is_the_terminals_twenty_until_a_surface_says_otherwise() {
    // 100×2000 px is 10×100 cells naturally; the default cap cuts it to
    // twenty rows and keeps the aspect.
    let mut image = image(100, 2000);
    assert_eq!(image.row_cap(), MAX_IMAGE_ROWS);
    assert_eq!(image.footprint(80), (2, MAX_IMAGE_ROWS));
    // A window with the room for it lets the picture stand at its
    // natural height.
    image.set_row_cap(100);
    assert_eq!(image.row_cap(), 100);
    assert_eq!(image.footprint(80), (10, 100), "the natural size, uncut");
    // A cap between the two still scales by whichever bound bites.
    image.set_row_cap(50);
    assert_eq!(image.footprint(80), (5, 50));
    // A cap of zero is a cap of one: an image always takes a row.
    image.set_row_cap(0);
    assert_eq!(image.footprint(80), (1, 1));
}

#[test]
fn the_digest_names_the_pixels_and_nothing_else() {
    // Decoded twice, the same bytes: one identity — what lets a
    // renderer admit a picture once and find it again.
    assert_eq!(image(4, 2).digest(), image(4, 2).digest());
    // Different pixels, a different identity.
    assert_ne!(image(4, 2).digest(), image(4, 3).digest());
    // The identity is the pixels: neither the alt text nor the cell the
    // image is measured against is part of it, since neither changes
    // what a quad of it shows.
    let mut other = StreamImage::decode(&png(4, 2), "another label", CellSize::new(20, 40))
        .expect("a png decodes");
    other.set_row_cap(3);
    assert_eq!(image(4, 2).digest(), other.digest());
}
