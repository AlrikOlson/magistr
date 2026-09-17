//! Measure native layout and rendering costs using representative frames.

use std::time::{Duration, Instant};

use magistr_cockpit::cockpit::{FrameClock, Model, Msg, Size, update};
use magistr_gui2::Frame;
use magistr_gui2::fixture::Fixture;

/// Match the cockpit performance gate's 120x40 viewport for comparable timings.
const GRID: Size = Size::new(120, 40);

/// Samples per measurement. Enough that the warm cache — a shaped run is
/// re-shaped once and then hit — is what is being measured, which is the
/// steady state a session actually runs in.
const SAMPLES: u32 = 60;

/// Layout + paint, warm.
const BUILD_BOUND: Duration = if cfg!(debug_assertions) {
    Duration::from_millis(12)
} else {
    Duration::from_millis(1)
};

/// A message folded and the next frame built — the native equivalent of
/// the cockpit's input-to-paint bound, minus the present (past the
/// submit the numbers belong to the driver and the compositor).
const INPUT_BOUND: Duration = if cfg!(debug_assertions) {
    Duration::from_millis(60)
} else {
    Duration::from_millis(20)
};

fn clock() -> FrameClock {
    FrameClock::new(0, Duration::ZERO)
}

/// The pixel viewport `GRID` asks for on `frame`'s grid.
fn viewport(frame: &Frame) -> (u32, u32) {
    let cell = frame.metrics();
    (
        (f32::from(GRID.cols) * cell.width()) as u32,
        (f32::from(GRID.rows) * cell.height()) as u32,
    )
}

/// The median and the worst of `samples`, which is what a latency bound
/// is about — a mean hides exactly the frame a user notices.
fn report(name: &str, mut samples: Vec<Duration>) -> (Duration, Duration) {
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    let worst = *samples.last().expect("at least one sample");
    let p90 = samples[samples.len() * 9 / 10];
    println!(
        "{name}: median {:.3} ms · p90 {:.3} ms · max {:.3} ms · n={}",
        median.as_secs_f64() * 1000.0,
        p90.as_secs_f64() * 1000.0,
        worst.as_secs_f64() * 1000.0,
        samples.len()
    );
    (median, worst)
}

/// One built frame, warm, over and over — `view` + taffy + the second
/// painter, which is exactly what the chunk's "layout + paint" names.
#[test]
fn layout_and_paint_at_a_120x40_viewport() {
    let mut model = Fixture::with_pixels(Fixture::busy(GRID));
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let (w, h) = viewport(&frame);
    // Warm: the first frame shapes every row and admits every glyph —
    // and settles the graph-rect reports — which is a different
    // measurement (and is taken below).
    for _ in 0..4 {
        let _ = frame.build(&mut model, &clock(), w, h).expect("builds");
        frame.text().take_uploads();
        frame.text().take_image_uploads();
    }
    let mut samples = Vec::new();
    for _ in 0..SAMPLES {
        let at = Instant::now();
        let _ = frame.build(&mut model, &clock(), w, h).expect("builds");
        samples.push(at.elapsed());
        frame.text().take_uploads();
    }
    let (median, _) = report("layout+paint 120x40", samples);
    assert!(
        median <= BUILD_BOUND,
        "median {median:?} over the {BUILD_BOUND:?} bound"
    );
    println!(
        "  instances {} · batches {}",
        frame.list().instances.len(),
        frame.list().batches.len()
    );
}

/// The cold frame: everything shaped and every glyph rasterized. Not a
/// bound — a session pays it once — but the number decides whether the
/// first frame of a window is a stutter or a hitch.
#[test]
fn the_first_frame_shapes_and_rasterizes_everything() {
    let mut model = Fixture::with_pixels(Fixture::busy(GRID));
    let mut frame = Frame::new().expect("a grid");
    let (w, h) = viewport(&frame);
    let at = Instant::now();
    let _ = frame.build(&mut model, &clock(), w, h).expect("builds");
    let cold = at.elapsed();
    println!(
        "cold first frame: {:.3} ms · {} instances",
        cold.as_secs_f64() * 1000.0,
        frame.list().instances.len()
    );
}

/// Measure a keystroke update and the resulting frame during streaming. Changed
/// rows miss the shaped-run cache, matching a live session.
#[test]
fn input_to_paint_while_the_turn_is_streaming() {
    let mut model = Fixture::with_pixels(Fixture::turn(GRID));
    let mut frame = Frame::new().expect("a grid");
    let (w, h) = viewport(&frame);
    let _ = frame.build(&mut model, &clock(), w, h).expect("builds");
    frame.text().take_uploads();

    let mut samples = Vec::new();
    for n in 0..SAMPLES {
        // A keystroke into the prompt: the one input that changes the
        // frame on every press, so nothing is measured against a fold
        // that did nothing.
        let key = magistr_cockpit::input::KeyEvent::new(magistr_cockpit::input::KeyCode::Char(
            char::from(b'a' + (n % 26) as u8),
        ));
        let at = Instant::now();
        update(&mut model, Msg::Key(key));
        let _ = frame.build(&mut model, &clock(), w, h).expect("builds");
        samples.push(at.elapsed());
        frame.text().take_uploads();
    }
    let (_, worst) = report("input→paint (streaming)", samples);
    assert!(
        worst <= INPUT_BOUND,
        "worst {worst:?} over the {INPUT_BOUND:?} bound"
    );
}

/// The cell grid is the layout unit, and this is the number behind that
/// decision: laying the *same* tree out costs a fraction of painting it,
/// so a pixel-length tree would be optimizing the cheap half.
///
/// Reported rather than bounded — it is evidence for a decision, not a
/// guarantee about a frame.
#[test]
fn layout_is_the_cheap_half_of_the_frame() {
    use magistr_cockpit::cockpit::view;
    use magistr_cockpit::layout::LayoutTree;

    let model: Model = Fixture::with_pixels(Fixture::busy(GRID));
    let mut tree = LayoutTree::new();
    let root = view(&model, &clock());
    for _ in 0..4 {
        tree.compute(&root, GRID.cols, GRID.rows).expect("lays out");
    }
    let mut layout = Vec::new();
    for _ in 0..SAMPLES {
        let at = Instant::now();
        let root = view(&model, &clock());
        tree.compute(&root, GRID.cols, GRID.rows).expect("lays out");
        layout.push(at.elapsed());
    }
    report("view+layout only 120x40", layout);
}
