//! The pixel layer's byte budget: what a dirty sparkline and a
//! dirty icon cost per frame on a retina display under each wire encoding.
//!
//! The numbers only mean something optimised, so the report is ignored by
//! default and run as
//! `cargo test -p magistr-tui --release --test pixel_perf -- --ignored --nocapture`.
//! The unignored test pins the one relation the default encoding rests on.

use std::time::{Duration, Instant};

use magistr_cockpit::WidgetId;
use magistr_cockpit::layout::Rect;
use magistr_cockpit::pixel::{CellSize, Icon, Painter, PixelSurface, Sparkline};
use magistr_tui::graphics::{Passthrough, Protocol};
use magistr_tui::pixel::{Compression, IdSpace};
use magistr_tui::render::Screen;

/// A retina cell.
const RETINA: CellSize = CellSize::new(16, 33);
/// The spike's region.
const SPARK: Rect = Rect::new(0, 3, 40, 12);
/// Two cells for a glyph.
const ICON: Rect = Rect::new(50, 1, 2, 1);
const FRAMES: u32 = 30;

/// Bytes per frame and the time from painting to the encoded frame,
/// averaged over `FRAMES` dirty frames.
struct Cost {
    bytes: usize,
    paint: Duration,
    encode: Duration,
}

fn measure(
    mut surface: PixelSurface,
    compression: Compression,
    painter: impl Fn(u32) -> Box<dyn Painter>,
) -> Cost {
    let mut screen = Screen::new(120, 40);
    screen.enable_graphics(
        Protocol::Kitty(Passthrough::Direct),
        compression,
        IdSpace::FIRST,
    );
    let mut out = Vec::with_capacity(4 << 20);
    let (mut bytes, mut paint, mut encode) = (0usize, Duration::ZERO, Duration::ZERO);
    for frame in 0..FRAMES {
        let painter = painter(frame);
        let began = Instant::now();
        surface.paint(&*painter);
        let painted = Instant::now();
        out.clear();
        screen.surface(WidgetId::new(1), &mut surface);
        screen.present(&mut out);
        encode += painted.elapsed();
        paint += painted - began;
        bytes += out.len();
    }
    Cost {
        bytes: bytes / FRAMES as usize,
        paint: paint / FRAMES,
        encode: encode / FRAMES,
    }
}

fn sparkline(frame: u32) -> Box<dyn Painter> {
    let values: Vec<f32> = (0..64)
        .map(|i| ((i + frame) as f32 * 0.37).sin() * 3.0 + (i as f32 * 0.05))
        .collect();
    Box::new(Sparkline::new(values))
}

fn icon(frame: u32) -> Box<dyn Painter> {
    let icon = match frame % 4 {
        0 => Icon::Done,
        1 => Icon::Failed,
        2 => Icon::Ready,
        _ => Icon::InProgress,
    };
    Box::new(move |pixmap: &mut tiny_skia::PixmapMut<'_>| {
        icon.draw(pixmap, tiny_skia::Color::WHITE);
    })
}

const ENCODINGS: [(&str, Compression); 4] = [
    ("raw", Compression::None),
    ("o=z level 1", Compression::Zlib(1)),
    ("o=z level 6", Compression::Zlib(6)),
    ("o=z level 9", Compression::Zlib(9)),
];

#[test]
fn zlib_shrinks_a_sparkline_frame_several_fold() {
    let raw = measure(
        PixelSurface::new(SPARK, RETINA),
        Compression::None,
        sparkline,
    );
    let zlib = measure(
        PixelSurface::new(SPARK, RETINA),
        Compression::Zlib(1),
        sparkline,
    );
    assert!(
        zlib.bytes * 4 < raw.bytes,
        "zlib {} bytes vs raw {} bytes",
        zlib.bytes,
        raw.bytes
    );
}

#[test]
#[ignore = "release-mode report: cargo nextest run -p magistr-tui --release -P perf --run-ignored only --no-capture"]
fn report_bytes_and_encode_time_per_encoding() {
    println!(
        "{:<12} {:<15} {:>12} {:>10} {:>10}",
        "encoding", "surface", "bytes/frame", "paint µs", "encode µs"
    );
    for (label, compression) in ENCODINGS {
        let spark = measure(PixelSurface::new(SPARK, RETINA), compression, sparkline);
        let glyph = measure(PixelSurface::with_alpha(ICON, RETINA), compression, icon);
        for (name, cost) in [("sparkline f=24", spark), ("icon f=32", glyph)] {
            println!(
                "{label:<12} {name:<15} {:>12} {:>10} {:>10}",
                cost.bytes,
                cost.paint.as_micros(),
                cost.encode.as_micros()
            );
        }
    }
}
