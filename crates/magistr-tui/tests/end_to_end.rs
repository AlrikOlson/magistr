//! The renderer end to end: a widget tree goes through
//! layout, painting, the damage diff and the encoder, and the bytes that
//! come out are replayed through a VT emulator (`vt100`) whose screen must
//! then hold, cell for cell, what the painter put in the buffer.
//!
//! Every other test in this crate checks one layer against its neighbour;
//! this one checks the only thing that matters to a user — what a terminal
//! shows after reading our output — against what we meant to draw.

use magistr_cockpit::WidgetId;
use magistr_cockpit::hit::HitRegistry;
use magistr_cockpit::layout::{LayoutTree, Node, Rect};
use magistr_cockpit::render::{Buffer, Color, width};
use magistr_tui::render::Screen;

#[path = "support/frame.rs"]
mod frame;

const BSU: &[u8] = b"\x1b[?2026h";
const ESU: &[u8] = b"\x1b[?2026l";
const CLEAR: &[u8] = b"\x1b[2J";

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|w| *w == needle)
        .count()
}

/// A terminal of `width × height` cells that has seen nothing yet.
fn terminal(width: u16, height: u16) -> vt100::Parser {
    vt100::Parser::new(height, width, 0)
}

fn color(color: Color) -> vt100::Color {
    match color {
        Color::Default => vt100::Color::Default,
        Color::Indexed(n) => vt100::Color::Idx(n),
        Color::Rgb(r, g, b) => vt100::Color::Rgb(r, g, b),
    }
}

/// The expected buffer for `frame` at `width × height`, painted by the same
/// painter that fills the presented screen.
fn expected(tree: &mut LayoutTree, frame: u64, width: u16, height: u16) -> Buffer {
    let mut buffer = Buffer::new(width, height);
    frame::paint_frame(tree, &frame::cockpit(frame), &mut buffer);
    buffer
}

/// Every cell the terminal shows equals the cell the buffer holds: the
/// grapheme (a never-written terminal cell counts as a blank), whether it
/// is a wide head or the continuation of one, and the foreground colour.
fn assert_terminal_shows(term: &vt100::Parser, buffer: &Buffer) {
    let screen = term.screen();
    assert_eq!(screen.size(), (buffer.height(), buffer.width()));
    let mut mismatches = Vec::new();
    for row in 0..buffer.height() {
        for col in 0..buffer.width() {
            let want = buffer.get(col, row).expect("inside the buffer");
            let got = screen.cell(row, col).expect("inside the terminal");
            if want.is_continuation() {
                if !got.is_wide_continuation() {
                    mismatches.push(format!(
                        "({col},{row}): expected a wide continuation, terminal shows {:?}",
                        got.contents()
                    ));
                }
                continue;
            }
            let shown = if got.contents().is_empty() {
                " "
            } else {
                got.contents()
            };
            if shown != want.symbol.as_str() {
                mismatches.push(format!(
                    "({col},{row}): buffer {:?}, terminal {shown:?}",
                    want.symbol
                ));
            }
            if (want.width() == 2) != got.is_wide() {
                mismatches.push(format!(
                    "({col},{row}): buffer width {}, terminal wide = {}",
                    want.width(),
                    got.is_wide()
                ));
            }
            if got.fgcolor() != color(want.style.fg) {
                mismatches.push(format!(
                    "({col},{row}): buffer fg {:?}, terminal fg {:?}",
                    want.style.fg,
                    got.fgcolor()
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} cells differ:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

/// The text of `buffer` inside `rect`'s first row.
fn row_text(buffer: &Buffer, rect: Rect) -> String {
    (rect.x..rect.right())
        .filter_map(|col| buffer.get(col, rect.y))
        .filter(|cell| !cell.is_continuation())
        .map(|cell| cell.symbol.as_str())
        .collect()
}

fn cells_that_differ(a: &Buffer, b: &Buffer) -> usize {
    a.cells()
        .iter()
        .zip(b.cells())
        .filter(|(x, y)| x != y)
        .count()
}

#[test]
fn a_cockpit_frame_replays_cell_for_cell() {
    let mut tree = LayoutTree::new();
    let mut screen = Screen::new(frame::WIDTH, frame::HEIGHT);
    let mut out = Vec::new();
    frame::render_frame(&mut tree, &mut screen, &mut out, 0);
    assert_eq!(count(&out, BSU), 1);
    assert_eq!(count(&out, ESU), 1);
    assert_eq!(count(&out, CLEAR), 1, "the first frame clears the display");

    let mut term = terminal(frame::WIDTH, frame::HEIGHT);
    term.process(&out);
    let want = expected(&mut tree, 0, frame::WIDTH, frame::HEIGHT);
    assert_terminal_shows(&term, &want);
    assert!(
        term.screen().contents().contains("magistr — frame 0"),
        "the header text reached the terminal"
    );
}

#[test]
fn consecutive_frames_replay_cumulatively_with_one_bracket_each() {
    let mut tree = LayoutTree::new();
    let mut screen = Screen::new(frame::WIDTH, frame::HEIGHT);
    let mut out = Vec::new();
    let mut term = terminal(frame::WIDTH, frame::HEIGHT);
    for frame in 0..12 {
        frame::render_frame(&mut tree, &mut screen, &mut out, frame);
        assert_eq!(count(&out, BSU), 1, "frame {frame}");
        assert_eq!(count(&out, ESU), 1, "frame {frame}");
        assert_eq!(
            count(&out, CLEAR),
            usize::from(frame == 0),
            "only the first frame clears"
        );
        term.process(&out);
        let want = expected(&mut tree, frame, frame::WIDTH, frame::HEIGHT);
        assert_terminal_shows(&term, &want);
    }
}

#[test]
fn a_counter_tick_costs_a_few_dozen_bytes() {
    let mut tree = LayoutTree::new();
    let mut screen = Screen::new(frame::WIDTH, frame::HEIGHT);
    let mut out = Vec::new();
    frame::render_frame(&mut tree, &mut screen, &mut out, 8);
    let bytes = frame::render_frame(&mut tree, &mut screen, &mut out, 9);

    let before = expected(&mut tree, 8, frame::WIDTH, frame::HEIGHT);
    let after = expected(&mut tree, 9, frame::WIDTH, frame::HEIGHT);
    let changed = cells_that_differ(&before, &after);
    assert_eq!(changed, 2, "the header counter and the first stream line");
    assert!(
        bytes < 64,
        "{changed} changed cells cost {bytes} bytes: {:?}",
        String::from_utf8_lossy(&out)
    );
}

#[test]
fn an_unchanged_cockpit_frame_is_bracket_only() {
    let mut tree = LayoutTree::new();
    let mut screen = Screen::new(frame::WIDTH, frame::HEIGHT);
    let mut out = Vec::new();
    frame::render_frame(&mut tree, &mut screen, &mut out, 3);
    frame::render_frame(&mut tree, &mut screen, &mut out, 3);
    assert_eq!(out, [BSU, ESU].concat());
}

#[test]
fn resizing_replays_at_every_size() {
    let sizes = [(80, 24), (180, 50), (80, 24)];
    let mut tree = LayoutTree::new();
    let mut screen = Screen::new(sizes[0].0, sizes[0].1);
    let mut term = terminal(sizes[0].0, sizes[0].1);
    let mut out = Vec::new();
    for (frame, &(width, height)) in sizes.iter().enumerate() {
        if frame > 0 {
            screen.resize(width, height);
            term.screen_mut().set_size(height, width);
        }
        let frame = frame as u64;
        frame::render_frame(&mut tree, &mut screen, &mut out, frame);
        assert_eq!(count(&out, CLEAR), 1, "every resize clears once");
        assert_eq!(count(&out, BSU), 1);
        assert_eq!(count(&out, ESU), 1);
        term.process(&out);
        let want = expected(&mut tree, frame, width, height);
        assert_terminal_shows(&term, &want);
    }
}

#[test]
fn the_mouse_resolves_through_the_layout_that_painted() {
    let mut tree = LayoutTree::new();
    let root = frame::cockpit(0);
    let mut buffer = Buffer::new(frame::WIDTH, frame::HEIGHT);
    let resolved = frame::paint_frame(&mut tree, &root, &mut buffer);
    let mut hits = HitRegistry::new();
    hits.register_layout(&resolved, 0);

    let id = WidgetId::new;
    let rect = |n: u32| resolved.rect(id(n)).expect("placed");

    // Header and footer span the width.
    assert_eq!(hits.hit(0, 0), Some(id(0)));
    assert_eq!(hits.hit(frame::WIDTH - 1, frame::HEIGHT - 1), Some(id(4)));

    // A rail row: the click lands on the row, and the row shows its text.
    let rail_row = rect(103);
    assert_eq!(hits.hit(rail_row.x, rail_row.y), Some(id(103)));
    assert!(row_text(&buffer, rail_row).starts_with("chunk-3"));

    // The stage's padding belongs to the stage itself, its rows to the rows.
    let stage = rect(2);
    assert_eq!(hits.hit(stage.x, stage.y), Some(id(2)));
    let stream_row = rect(205);
    assert_eq!(
        hits.hit(stream_row.right() - 1, stream_row.y),
        Some(id(205))
    );
    assert!(row_text(&buffer, stream_row).starts_with("05 The agent"));

    // A dock row.
    let dock = rect(3);
    let dock_row = rect(305);
    assert_eq!(hits.hit(dock_row.x, dock_row.y), Some(id(305)));

    // An overlay straddling stage and dock takes every click under it and
    // nothing beside or below it.
    let overlay = Rect::new(dock.x - 10, stream_row.y, 30, 1);
    hits.register(overlay, 100, id(999));
    assert_eq!(hits.hit(dock.x - 1, stream_row.y), Some(id(999)));
    assert_eq!(hits.hit(dock.x + 5, stream_row.y), Some(id(999)));
    assert_eq!(hits.hit(dock.x - 11, stream_row.y), Some(id(205)));
    assert_eq!(hits.hit(dock.x + 5, overlay.bottom()), Some(id(3)));
    assert_eq!(hits.hit(dock_row.x, dock_row.y), Some(id(305)));

    // And the terminal shows the row the click resolved to.
    let mut screen = Screen::new(frame::WIDTH, frame::HEIGHT);
    let mut out = Vec::new();
    frame::render_frame(&mut tree, &mut screen, &mut out, 0);
    let mut term = terminal(frame::WIDTH, frame::HEIGHT);
    term.process(&out);
    let shown: String = (stream_row.x..stream_row.x + 12)
        .filter_map(|col| term.screen().cell(stream_row.y, col))
        .map(vt100::Cell::contents)
        .collect();
    assert_eq!(shown, "05 The agent");
}

#[test]
fn cjk_and_combining_text_flows_through_every_layer() {
    const CJK: &str = "漢字テスト";
    const COMBINING: &str = "cafe\u{301} noe\u{308}l";
    let id = WidgetId::new;
    let root = Node::column()
        .child(
            Node::row()
                .child(Node::text(CJK).id(id(1)))
                .child(Node::text("|").id(id(2))),
        )
        .child(
            Node::row()
                .child(Node::text(COMBINING).id(id(3)))
                .child(Node::text("|").id(id(4))),
        );

    let mut tree = LayoutTree::new();
    let mut buffer = Buffer::new(40, 2);
    let resolved = frame::paint_frame(&mut tree, &root, &mut buffer);
    let rect = |n: u32| resolved.rect(id(n)).expect("placed");

    // Layout width == measured width == where the next widget starts.
    assert_eq!(width(CJK), 10);
    assert_eq!(width(COMBINING), 9);
    for (text, node, next) in [(CJK, 1, 2), (COMBINING, 3, 4)] {
        assert_eq!(rect(node).width, width(text), "{text:?}");
        assert_eq!(rect(next).x, width(text), "{text:?}");
    }

    // The buffer holds heads and continuations where the layout said.
    assert_eq!(buffer.get(0, 0).map(|c| c.symbol.as_str()), Some("漢"));
    assert!(buffer.get(1, 0).is_some_and(|c| c.is_continuation()));
    assert_eq!(buffer.get(10, 0).map(|c| c.symbol.as_str()), Some("|"));
    assert_eq!(
        buffer.get(3, 1).map(|c| c.symbol.as_str()),
        Some("e\u{301}")
    );
    assert_eq!(buffer.get(9, 1).map(|c| c.symbol.as_str()), Some("|"));

    // And the terminal agrees, cell for cell.
    let mut screen = Screen::new(40, 2);
    frame::paint_frame(&mut tree, &root, screen.buffer());
    let mut out = Vec::new();
    screen.present(&mut out);
    let mut term = terminal(40, 2);
    term.process(&out);
    assert_terminal_shows(&term, &buffer);
}

/// A ZWJ emoji sequence is one grapheme two columns wide to every layer of
/// this crate — and to no two terminals alike. The `vt100` oracle advances
/// per code point and shows the family as four wide cells, and real
/// terminals split the same way (Kitty, WezTerm and iTerm2 join; others do
/// not). So the encoder does not trust its cursor after the family: the
/// bar that follows is written after an explicit cursor move, and lands in
/// the column the buffer holds it in even on a terminal that drew the
/// family eight columns wide.
#[test]
fn a_zwj_sequence_is_one_grapheme_two_columns_wide_to_every_layer() {
    const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
    let id = WidgetId::new;
    let root = Node::row()
        .child(Node::text(FAMILY).id(id(5)))
        .child(Node::text("|").id(id(6)));

    let mut tree = LayoutTree::new();
    let mut screen = Screen::new(40, 1);
    let resolved = frame::paint_frame(&mut tree, &root, screen.buffer());
    assert_eq!(width(FAMILY), 2);
    assert_eq!(resolved.rect(id(5)).map(|r| r.width), Some(2));
    assert_eq!(resolved.rect(id(6)).map(|r| r.x), Some(2));
    let buffer = screen.buffer();
    assert_eq!(buffer.get(0, 0).map(|c| c.symbol.as_str()), Some(FAMILY));
    assert!(buffer.get(1, 0).is_some_and(|c| c.is_continuation()));
    assert_eq!(buffer.get(2, 0).map(|c| c.symbol.as_str()), Some("|"));

    let mut out = Vec::new();
    screen.present(&mut out);
    let text = String::from_utf8(out.clone()).expect("utf-8 frame");
    assert!(
        text.contains(&format!("{FAMILY}\x1b[1;3H|")),
        "the bar re-anchors the cursor after the family: {text:?}"
    );

    // vt100 draws the family wider than we measure it; the bar is still
    // where we put it.
    let mut term = terminal(40, 1);
    term.process(&out);
    assert_eq!(term.screen().cell(0, 2).map(|c| c.contents()), Some("|"));
}
