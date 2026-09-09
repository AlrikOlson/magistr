use super::*;

use crate::graphics::{ImageId, Kitty, Passthrough, PlacementId};

/// An encoder writing straight to the terminal, the way the
/// screenshot's `Screen` does.
fn kitty() -> Kitty {
    Kitty::new(Passthrough::Direct)
}

/// `CSI row;col H` for a zero-based cell, the bytes `Encoder::move_to`
/// writes.
fn cup(col: u16, row: u16) -> Vec<u8> {
    format!("\x1b[{};{}H", row + 1, col + 1).into_bytes()
}

/// The image id `raw`, which is in range for every fixture here.
fn image(raw: u32) -> ImageId {
    ImageId::new(raw).expect("a small id is in range")
}

/// The one placement id every fixture uses.
fn slot() -> PlacementId {
    PlacementId::new(1).expect("1 is in range")
}

/// A `w × h` RGBA image whose pixels are their own index, so a
/// round-trip that reorders or truncates is visible.
fn rgba(w: u32, h: u32) -> Vec<u8> {
    (0..w * h * 4).map(|i| (i % 251) as u8).collect()
}

#[test]
fn a_transmit_and_a_placement_round_trip_through_the_encoder() {
    let (kitty, pixels) = (kitty(), rgba(4, 3));
    let mut out = Vec::new();
    kitty
        .transmit(&mut out, image(1), PixelFormat::Rgba, 4, 3, &pixels)
        .expect("the buffer matches the dimensions");
    out.extend_from_slice(&cup(7, 2));
    kitty.place(&mut out, image(1), slot(), 2, 1, -1);

    let placements = decode(&out);
    assert_eq!(placements.len(), 1, "one image on screen");
    let shown = &placements[0];
    assert_eq!((shown.col, shown.row), (7, 2), "the cursor located it");
    assert_eq!((shown.cols, shown.rows), (2, 1), "the cell span is c/r");
    assert_eq!((shown.width, shown.height), (4, 3), "the pixel size is s/v");
    assert_eq!(shown.rgba, pixels, "the pixels survive base64");
    assert_eq!(shown.z, -1, "placed under the text");
}

#[test]
fn an_rgb_transmit_widens_to_opaque_rgba() {
    let (kitty, pixels) = (kitty(), vec![10, 20, 30, 40, 50, 60]);
    let mut out = Vec::new();
    kitty
        .transmit(&mut out, image(3), PixelFormat::Rgb, 2, 1, &pixels)
        .expect("the buffer matches the dimensions");
    out.extend_from_slice(&cup(0, 0));
    kitty.place(&mut out, image(3), slot(), 1, 1, -1);

    let placements = decode(&out);
    assert_eq!(
        placements[0].rgba,
        vec![10, 20, 30, 255, 40, 50, 60, 255],
        "f=24 gains a full alpha channel rather than being refused"
    );
}

#[test]
fn a_zlib_transmit_is_inflated() {
    use std::io::Write as _;
    let (kitty, pixels) = (kitty(), rgba(8, 8));
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(1));
    encoder.write_all(&pixels).expect("a Vec never fails");
    let deflated = encoder.finish().expect("a Vec never fails");

    let mut out = Vec::new();
    kitty
        .transmit_zlib(&mut out, image(5), PixelFormat::Rgba, 8, 8, &deflated)
        .expect("the dimensions are non-zero");
    out.extend_from_slice(&cup(1, 1));
    kitty.place(&mut out, image(5), slot(), 4, 2, -1);

    assert_eq!(decode(&out)[0].rgba, pixels, "o=z is inflated, not refused");
}

/// The transmit chunker splits at 4096 base64 characters, and only the
/// first chunk carries the image keys — so a payload over the limit is
/// the case a decoder addressing every chunk by `i=` would silently
/// lose.
#[test]
fn a_transmit_over_one_chunk_is_reassembled_from_its_continuations() {
    let (kitty, pixels) = (kitty(), rgba(64, 64));
    assert!(
        pixels.len().div_ceil(3) * 4 > Kitty::CHUNK,
        "the fixture really does span several chunks"
    );
    let mut out = Vec::new();
    kitty
        .transmit(&mut out, image(9), PixelFormat::Rgba, 64, 64, &pixels)
        .expect("the buffer matches the dimensions");
    out.extend_from_slice(&cup(0, 0));
    kitty.place(&mut out, image(9), slot(), 8, 4, -1);

    assert_eq!(decode(&out)[0].rgba, pixels, "every chunk was appended");
}

#[test]
fn a_deleted_placement_is_not_on_screen_and_a_deleted_image_takes_its_own() {
    let (kitty, pixels) = (kitty(), rgba(2, 2));
    let mut out = Vec::new();
    for id in [1, 2] {
        kitty
            .transmit(&mut out, image(id), PixelFormat::Rgba, 2, 2, &pixels)
            .expect("the buffer matches the dimensions");
        out.extend_from_slice(&cup(id as u16, 0));
        kitty.place(&mut out, image(id), slot(), 1, 1, -1);
    }
    kitty.delete_placement(&mut out, image(1), slot());
    assert_eq!(
        decode(&out).iter().map(|p| p.col).collect::<Vec<_>>(),
        vec![2],
        "the deleted placement is gone and the other stays"
    );

    kitty.delete_image(&mut out, image(2));
    assert!(decode(&out).is_empty(), "d=I takes the image's placements");
}

/// The encoder elides a move to where the cursor already is. A
/// placement decoded with no cursor behind it would be placed at a
/// guess, so it is dropped instead — and the screenshot's own test
/// asserts this never happens on a real frame.
#[test]
fn a_placement_with_no_cursor_behind_it_is_refused_rather_than_guessed() {
    let (kitty, pixels) = (kitty(), rgba(2, 2));
    let mut out = Vec::new();
    kitty
        .transmit(&mut out, image(1), PixelFormat::Rgba, 2, 2, &pixels)
        .expect("the buffer matches the dimensions");
    kitty.place(&mut out, image(1), slot(), 1, 1, -1);
    assert!(
        decode(&out).is_empty(),
        "no CUP, no placement — never a placement at (0, 0) by default"
    );
}

#[test]
fn a_placement_whose_image_never_arrived_is_dropped() {
    let kitty = kitty();
    let mut out = cup(3, 3);
    kitty.place(&mut out, image(77), slot(), 1, 1, -1);
    assert!(decode(&out).is_empty(), "an image nobody transmitted");
}

/// A CSI that is not a cursor move must not be read as one, and must
/// not swallow the bytes after it.
#[test]
fn an_sgr_between_the_move_and_the_placement_leaves_the_cursor_alone() {
    let (kitty, pixels) = (kitty(), rgba(2, 2));
    let mut out = cup(5, 6);
    out.extend_from_slice(b"\x1b[0m");
    kitty
        .transmit(&mut out, image(1), PixelFormat::Rgba, 2, 2, &pixels)
        .expect("the buffer matches the dimensions");
    kitty.place(&mut out, image(1), slot(), 1, 1, -1);

    let placements = decode(&out);
    assert_eq!(
        (placements[0].col, placements[0].row),
        (5, 6),
        "SGR moves nothing"
    );
}

/// Placements come back in the order the frame made them, which is the
/// order they must be composited in — every cockpit surface is placed
/// at the same z.
#[test]
fn placements_keep_the_order_the_frame_made_them_in() {
    let (kitty, pixels) = (kitty(), rgba(1, 1));
    let mut out = Vec::new();
    for id in [4, 2, 7] {
        kitty
            .transmit(&mut out, image(id), PixelFormat::Rgba, 1, 1, &pixels)
            .expect("the buffer matches the dimensions");
        out.extend_from_slice(&cup(id as u16, 0));
        kitty.place(&mut out, image(id), slot(), 1, 1, -1);
    }
    assert_eq!(
        decode(&out).iter().map(|p| p.col).collect::<Vec<_>>(),
        vec![4, 2, 7],
        "wire order, not id order"
    );
}
