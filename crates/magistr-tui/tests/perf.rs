//! The renderer's performance bar: a full 120×40 frame —
//! tree build, layout, paint, diff and encode — under 1 ms at p99.
//!
//! Timing only means something optimised, so the gate is ignored by
//! default and run as
//! `cargo test -p magistr-tui --release --test perf -- --ignored`.

use std::time::{Duration, Instant};

use magistr_cockpit::layout::LayoutTree;
use magistr_tui::render::Screen;

#[path = "support/frame.rs"]
mod frame;

#[test]
fn a_cockpit_frame_paints_every_region() {
    let mut tree = LayoutTree::new();
    let mut screen = Screen::new(frame::WIDTH, frame::HEIGHT);
    let mut out = Vec::new();
    let first = frame::render_frame(&mut tree, &mut screen, &mut out, 0);
    assert!(
        first > 4_000,
        "a first frame repaints the screen: {first} bytes"
    );
    let second = frame::render_frame(&mut tree, &mut screen, &mut out, 1);
    assert!(
        second < first,
        "a one-line change is a small diff: {second} bytes"
    );
    assert!(second > 0);
}

#[test]
#[ignore = "release-mode timing gate: cargo nextest run -p magistr-tui --release -P perf --run-ignored only"]
fn full_frame_p99_under_one_millisecond() {
    const WARMUP: u64 = 200;
    const SAMPLES: usize = 2_000;
    let mut tree = LayoutTree::new();
    let mut screen = Screen::new(frame::WIDTH, frame::HEIGHT);
    let mut out = Vec::with_capacity(64 * 1024);
    for n in 0..WARMUP {
        frame::render_frame(&mut tree, &mut screen, &mut out, n);
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for n in 0..SAMPLES {
        let start = Instant::now();
        frame::render_frame(&mut tree, &mut screen, &mut out, WARMUP + n as u64);
        samples.push(start.elapsed());
    }
    samples.sort();
    let p50 = samples[SAMPLES / 2];
    let p99 = samples[SAMPLES * 99 / 100];
    let max = samples[SAMPLES - 1];
    assert!(
        p99 < Duration::from_millis(1),
        "full frame p99 {p99:?} (p50 {p50:?}, max {max:?}) exceeds 1 ms"
    );
}
