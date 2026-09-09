use tiny_skia::Pixmap;

use super::*;
use crate::layout::Rect;
use crate::pixel::{CellSize, PixelSurface};

/// Straight RGBA for a `width × height` rectangle, opaque, with the left
/// column red and the rest blue.
fn rgba(width: u16, height: u16) -> Vec<u8> {
    let mut out = Vec::new();
    for _ in 0..height {
        for x in 0..width {
            if x == 0 {
                out.extend_from_slice(&[255, 0, 0, 255]);
            } else {
                out.extend_from_slice(&[0, 0, 255, 255]);
            }
        }
    }
    out
}

fn painted(image: &Image, width: u32, height: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("a pixmap allocates");
    image.paint(&mut pixmap.as_mut());
    pixmap
}

#[test]
fn pixels_that_do_not_fill_the_rectangle_are_refused() {
    assert!(!Image::new(&[0, 0, 0, 255], 2, 2).is_drawable());
    assert!(!Image::new(&[], 0, 0).is_drawable());
    assert!(Image::new(&rgba(2, 2), 2, 2).is_drawable());
}

#[test]
fn an_undrawable_image_is_still_its_background() {
    let image = Image::new(&[], 4, 4).background(10, 20, 30);
    let pixmap = painted(&image, 4, 4);
    let first = pixmap.pixels()[0];
    assert_eq!(
        (first.red(), first.green(), first.blue()),
        (10, 20, 30),
        "the stage's ground fills the surface"
    );
}

#[test]
fn the_image_scales_to_fill_the_surface_keeping_left_and_right() {
    // A 2×2 source over a 20×20 surface: the left half stays red, the
    // right half stays blue.
    let image = Image::new(&rgba(2, 2), 2, 2);
    let pixmap = painted(&image, 20, 20);
    let at = |x: u32, y: u32| {
        let px = pixmap.pixels()[(y * 20 + x) as usize];
        (px.red(), px.green(), px.blue())
    };
    assert_eq!(at(1, 10).0, 255, "the left edge is red");
    assert_eq!(at(18, 10).2, 255, "the right edge is blue");
}

#[test]
fn a_transparent_source_shows_the_background_through() {
    let transparent = vec![0u8; 4];
    let image = Image::new(&transparent, 1, 1).background(200, 100, 50);
    let pixmap = painted(&image, 4, 4);
    let first = pixmap.pixels()[0];
    assert_eq!((first.red(), first.green(), first.blue()), (200, 100, 50));
}

#[test]
fn a_surface_takes_the_painter_and_goes_dirty() {
    let mut surface = PixelSurface::new(Rect::new(0, 0, 4, 2), CellSize::new(10, 20));
    surface.paint(&Image::new(&rgba(8, 8), 8, 8));
    assert!(surface.is_dirty());
    assert_eq!((surface.width(), surface.height()), (40, 40));
}

/// Straight RGBA whose top half is red and bottom half blue.
fn split(width: u16, height: u16) -> Vec<u8> {
    let mut out = Vec::new();
    for y in 0..height {
        for _ in 0..width {
            if y < height / 2 {
                out.extend_from_slice(&[255, 0, 0, 255]);
            } else {
                out.extend_from_slice(&[0, 0, 255, 255]);
            }
        }
    }
    out
}

#[test]
fn a_row_window_shows_that_slice_of_the_image() {
    let pixels = split(4, 40);
    // A placement four rows tall, of which only the last two are on
    // screen: the surface shows the bottom half — blue.
    let bottom = Image::new(&pixels, 4, 40).rows(2, 2, 4);
    let pixmap = painted(&bottom, 8, 20);
    let px = pixmap.pixels()[(10 * 8 + 4) as usize];
    assert_eq!((px.red(), px.blue()), (0, 255), "the bottom half is blue");

    let top = Image::new(&pixels, 4, 40).rows(0, 2, 4);
    let pixmap = painted(&top, 8, 20);
    let px = pixmap.pixels()[(10 * 8 + 4) as usize];
    assert_eq!((px.red(), px.blue()), (255, 0), "the top half is red");
}

#[test]
fn a_nonsense_window_is_ignored() {
    let full = Image::new(&split(4, 40), 4, 40);
    let same = |image: &Image| {
        let pixmap = painted(image, 8, 40);
        pixmap.pixels()[(30 * 8 + 4) as usize].blue()
    };
    assert_eq!(same(&full.clone().rows(0, 0, 4)), same(&full), "no rows");
    assert_eq!(
        same(&full.clone().rows(9, 2, 4)),
        same(&full),
        "past the end"
    );
    assert_eq!(
        same(&full.clone().rows(0, 2, 0)),
        same(&full),
        "no placement"
    );
}
