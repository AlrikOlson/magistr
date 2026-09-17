use tiny_skia::{Color, PixmapMut};

use super::*;
use crate::graphics::Passthrough;
use magistr_cockpit::layout::Rect;
use magistr_cockpit::pixel::CellSize;

fn surface(rect: Rect) -> PixelSurface {
    let mut surface = PixelSurface::new(rect, CellSize::new(2, 2));
    surface.paint(&|pixmap: &mut PixmapMut<'_>| pixmap.fill(Color::from_rgba8(1, 2, 3, 255)));
    surface
}

/// Show one surface and finish the frame.
fn frame(
    backend: &mut Backend,
    encoder: &mut Encoder,
    shown: Option<&mut PixelSurface>,
) -> (Vec<u8>, Vec<Rect>) {
    if let Some(surface) = shown {
        backend.show(WidgetId::new(1), surface);
    }
    let mut out = Vec::new();
    let vacated = backend.prepare();
    backend.finish(encoder, &mut out, &[]);
    (out, vacated)
}

#[test]
fn the_kitty_protocol_builds_the_kitty_compositor() {
    let mut backend = Backend::new(
        Protocol::Kitty(Passthrough::Direct),
        Compression::None,
        IdSpace::FIRST,
    );
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 4, 2));

    let (out, vacated) = frame(&mut backend, &mut encoder, Some(&mut s));

    assert!(matches!(backend, Backend::Kitty(_)));
    assert!(
        out.windows(6).any(|w| w == b"\x1b_Ga=t"),
        "a kitty transmit"
    );
    assert!(
        vacated.is_empty(),
        "kitty places under the text, so it covers no cell"
    );
}

#[test]
fn the_sixel_protocol_builds_the_sixel_compositor() {
    let mut backend = Backend::new(Protocol::Sixel, Compression::DEFAULT, IdSpace::FIRST);
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 4, 2));

    let (out, vacated) = frame(&mut backend, &mut encoder, Some(&mut s));

    assert!(matches!(backend, Backend::Sixel(_)));
    assert!(out.windows(2).any(|w| w == b"\x1bP"), "a sixel DCS");
    assert!(!out.windows(3).any(|w| w == b"\x1b_G"), "no kitty command");
    assert!(vacated.is_empty());
}

#[test]
fn only_the_sixel_backend_reports_a_vacated_rect() {
    for protocol in [Protocol::Kitty(Passthrough::Direct), Protocol::Sixel] {
        let mut backend = Backend::new(protocol, Compression::None, IdSpace::FIRST);
        let mut encoder = Encoder::new();
        let mut s = surface(Rect::new(0, 0, 4, 2));
        frame(&mut backend, &mut encoder, Some(&mut s));

        let (_, vacated) = frame(&mut backend, &mut encoder, None);

        assert_eq!(
            !vacated.is_empty(),
            protocol == Protocol::Sixel,
            "{protocol:?} after the surface left"
        );
    }
}

#[test]
fn release_deletes_kitty_images_and_writes_nothing_for_sixel() {
    let mut kitty = Backend::new(
        Protocol::Kitty(Passthrough::Direct),
        Compression::None,
        IdSpace::FIRST,
    );
    let mut sixel = Backend::new(Protocol::Sixel, Compression::None, IdSpace::FIRST);
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 4, 2));
    frame(&mut kitty, &mut encoder, Some(&mut s));
    let mut s = surface(Rect::new(0, 0, 4, 2));
    frame(&mut sixel, &mut encoder, Some(&mut s));

    let mut out = Vec::new();
    kitty.release(&mut out);
    assert!(out.windows(6).any(|w| w == b"\x1b_Ga=d"), "an image delete");

    let mut out = Vec::new();
    sixel.release(&mut out);
    assert!(out.is_empty(), "a sixel terminal stores nothing");
}

#[test]
fn forgetting_placements_shows_every_surface_again_on_both_paths() {
    for protocol in [Protocol::Kitty(Passthrough::Direct), Protocol::Sixel] {
        let mut backend = Backend::new(protocol, Compression::None, IdSpace::FIRST);
        let mut encoder = Encoder::new();
        let mut s = surface(Rect::new(0, 0, 4, 2));
        frame(&mut backend, &mut encoder, Some(&mut s));

        backend.forget_placements();
        encoder.reset();
        let (out, _) = frame(&mut backend, &mut encoder, Some(&mut s));

        assert!(!out.is_empty(), "{protocol:?} puts the surface back");
    }
}
