//! Renderer benchmarks: `cargo bench -p magistr-tui`.
//!
//! - `full_frame`: diff + encode of a fully redrawn 120×40 frame against an
//!   identical front buffer (the steady-state cost of an immediate-mode
//!   redraw with nothing changed).
//! - `full_repaint`: every cell differs.
//! - `one_cell_change`: one cell differs on a settled screen.
//! - `full_frame_with_layout`: the umbrella's bar — build a cockpit tree,
//!   lay it out, paint every region, diff and encode, with one stream line
//!   changing per frame (`tests/support/frame.rs`).

use magistr_cockpit::layout::LayoutTree;
use magistr_cockpit::render::{Color, Style};
use magistr_tui::render::Screen;

#[path = "../tests/support/frame.rs"]
mod frame;

const WIDTH: u16 = 120;
const HEIGHT: u16 = 40;

fn draw(screen: &mut Screen, marker: &str, style: &Style) {
    let (w, h) = screen.size();
    let buffer = screen.buffer();
    for row in 0..h {
        let line: String = (0..w)
            .map(|c| if (c + row) % 7 == 0 { '#' } else { '.' })
            .collect();
        buffer.put_str(0, row, &line, style);
    }
    buffer.put_str(2, 1, marker, style);
}

fn settled(style: &Style) -> Screen {
    let mut screen = Screen::new(WIDTH, HEIGHT);
    draw(&mut screen, "a", style);
    screen.present(&mut Vec::new());
    screen
}

#[divan::bench]
fn full_frame(bencher: divan::Bencher<'_, '_>) {
    let style = Style::new();
    let mut screen = settled(&style);
    let mut out = Vec::with_capacity(64 * 1024);
    bencher.bench_local(|| {
        draw(&mut screen, "a", &style);
        out.clear();
        screen.present(&mut out);
        out.len()
    });
}

#[divan::bench]
fn full_repaint(bencher: divan::Bencher<'_, '_>) {
    let styles = [
        Style::new().fg(Color::Rgb(200, 200, 200)),
        Style::new().fg(Color::Rgb(100, 100, 100)),
    ];
    let mut screen = settled(&styles[0]);
    let mut out = Vec::with_capacity(256 * 1024);
    let mut flip = 0usize;
    bencher.bench_local(|| {
        flip ^= 1;
        draw(&mut screen, "a", &styles[flip]);
        out.clear();
        screen.present(&mut out);
        out.len()
    });
}

#[divan::bench]
fn one_cell_change(bencher: divan::Bencher<'_, '_>) {
    let style = Style::new();
    let mut screen = settled(&style);
    let mut out = Vec::with_capacity(64 * 1024);
    let markers = ["a", "b"];
    let mut flip = 0usize;
    bencher.bench_local(|| {
        flip ^= 1;
        draw(&mut screen, markers[flip], &style);
        out.clear();
        screen.present(&mut out);
        out.len()
    });
}

#[divan::bench]
fn full_frame_with_layout(bencher: divan::Bencher<'_, '_>) {
    let mut tree = LayoutTree::new();
    let mut screen = Screen::new(frame::WIDTH, frame::HEIGHT);
    let mut out = Vec::with_capacity(64 * 1024);
    let mut counter = 0u64;
    frame::render_frame(&mut tree, &mut screen, &mut out, counter);
    bencher.bench_local(|| {
        counter += 1;
        frame::render_frame(&mut tree, &mut screen, &mut out, counter)
    });
}

fn main() {
    divan::main();
}
