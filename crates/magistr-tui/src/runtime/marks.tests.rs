use super::*;
use crate::graphics::{Passthrough, Protocol};
use crate::pixel::{Compression, IdSpace};
use magistr_cockpit::layout::{Frame, LayoutTree, Length};
use magistr_cockpit::pixel::Icon;
use magistr_cockpit::render::Style;

fn screen(graphics: bool) -> Screen {
    let mut screen = Screen::new(20, 4);
    if graphics {
        screen.enable_graphics(
            Protocol::Kitty(Passthrough::Direct),
            Compression::None,
            IdSpace::FIRST,
        );
    }
    screen
}

fn fallback() -> Fallback {
    Fallback {
        ground: (26, 27, 38),
        ink: (205, 214, 244),
    }
}

fn icon(col: u16) -> Mark {
    Mark::Icon {
        icon: Icon::Done,
        col,
        cols: 1,
        ink: Color::Rgb(158, 206, 106),
    }
}

fn tree(rows: &[Mark]) -> Node {
    let mut root = Node::column().frame(Frame::new(Style::new()).drawn(true));
    for mark in rows {
        root = root.child(
            Node::text("  x")
                .mark(mark.clone())
                .height(Length::Cells(1)),
        );
    }
    root
}

fn layout(root: &Node) -> Resolved {
    LayoutTree::new().compute(root, 20, 4).expect("layout")
}

/// The wire a frame writes, so a test can count what was transmitted.
fn present(screen: &mut Screen) -> Vec<u8> {
    let mut out = Vec::new();
    screen.present(&mut out);
    out
}

fn transmits(wire: &[u8]) -> usize {
    wire.windows(4).filter(|w| w == b"a=t,").count()
}

#[test]
fn an_unchanged_frame_transmits_nothing_and_a_changed_mark_transmits_once() {
    let mut marks = Marks::default();
    let mut screen = screen(true);
    let root = tree(&[icon(0), icon(1)]);
    let resolved = layout(&root);
    marks.show(&root, &resolved, CellSize::DEFAULT, fallback(), &mut screen);
    let first = present(&mut screen);
    // The panel ring and two icons: three surfaces went out.
    assert_eq!(transmits(&first), 3, "{}", String::from_utf8_lossy(&first));
    marks.show(&root, &resolved, CellSize::DEFAULT, fallback(), &mut screen);
    assert_eq!(
        transmits(&present(&mut screen)),
        0,
        "a still frame costs nothing"
    );
    // The second icon becomes a different glyph: one surface is new.
    let root = tree(&[
        icon(0),
        Mark::Icon {
            icon: Icon::Failed,
            col: 1,
            cols: 1,
            ink: Color::Rgb(247, 118, 142),
        },
    ]);
    let resolved = layout(&root);
    marks.show(&root, &resolved, CellSize::DEFAULT, fallback(), &mut screen);
    assert_eq!(transmits(&present(&mut screen)), 1);
}

#[test]
fn a_mark_that_left_frees_its_slot_for_the_next_newcomer() {
    let mut marks = Marks::default();
    let mut screen = screen(true);
    let root = tree(&[icon(0), icon(1)]);
    marks.show(
        &root,
        &layout(&root),
        CellSize::DEFAULT,
        fallback(),
        &mut screen,
    );
    present(&mut screen);
    assert_eq!(marks.minted, 3);
    let root = tree(&[icon(0)]);
    marks.show(
        &root,
        &layout(&root),
        CellSize::DEFAULT,
        fallback(),
        &mut screen,
    );
    present(&mut screen);
    assert_eq!(marks.free.len(), 1, "the icon that left gave its slot back");
    let root = tree(&[icon(0), icon(2)]);
    marks.show(
        &root,
        &layout(&root),
        CellSize::DEFAULT,
        fallback(),
        &mut screen,
    );
    assert_eq!(marks.minted, 3, "the newcomer took the freed slot");
    assert!(marks.free.is_empty());
}

#[test]
fn without_graphics_nothing_is_shown_or_kept() {
    let mut marks = Marks::default();
    let mut screen = screen(false);
    let root = tree(&[icon(0)]);
    marks.show(
        &root,
        &layout(&root),
        CellSize::DEFAULT,
        fallback(),
        &mut screen,
    );
    assert!(marks.shown.is_empty());
    assert_eq!(marks.minted, 0);
}

#[test]
fn an_icon_covers_its_cell_on_the_first_row_and_one_past_the_edge_covers_nothing() {
    let rect = Rect::new(4, 2, 6, 3);
    assert_eq!(covered(&icon(1), rect), Some(Rect::new(5, 2, 1, 1)));
    assert_eq!(
        covered(
            &Mark::Icon {
                icon: Icon::Pin,
                col: 5,
                cols: 2,
                ink: Color::Default
            },
            rect
        ),
        Some(Rect::new(9, 2, 1, 1)),
        "a two-cell icon at the last cell is cut to it"
    );
    assert_eq!(covered(&icon(6), rect), None);
    assert_eq!(
        covered(
            &Mark::Rule {
                ink: Color::Default
            },
            rect
        ),
        Some(rect)
    );
}

#[test]
fn every_kind_of_mark_paints_a_surface_of_its_rect() {
    let rect = Rect::new(0, 0, 8, 1);
    let cell = CellSize::DEFAULT;
    let kinds = [
        icon(0),
        Mark::Chip {
            fill: Color::Default,
            ground: Color::Default,
        },
        Mark::Segment {
            fill: Some(Color::Rgb(1, 2, 3)),
            track: Color::Default,
            ground: Color::Default,
            first: true,
            last: false,
        },
        Mark::Bar {
            fill: 500,
            line: Color::Default,
            ground: Color::Default,
        },
        Mark::Rule {
            ink: Color::Default,
        },
        Mark::Panel {
            border: Color::Default,
            title: Some((2, 4)),
        },
    ];
    for mark in kinds {
        // A panel ring needs two rows to have a top and a bottom edge.
        let rect = match mark {
            Mark::Panel { .. } => Rect::new(0, 0, 8, 2),
            _ => rect,
        };
        let surface = paint(&mark, rect, cell, fallback());
        assert_eq!(surface.rect(), rect, "{mark:?}");
        assert!(surface.is_dirty(), "{mark:?} is ready to transmit");
        assert!(
            surface.pixels().chunks(4).any(|px| px[3] > 0),
            "{mark:?} drew something"
        );
    }
}

#[test]
fn a_mark_that_changed_over_the_same_cells_repaints_into_its_slot() {
    let mut marks = Marks::default();
    let mut screen = screen(true);
    let root = tree(&[icon(0)]);
    marks.show(
        &root,
        &layout(&root),
        CellSize::DEFAULT,
        fallback(),
        &mut screen,
    );
    let first = present(&mut screen);
    assert_eq!(transmits(&first), 2, "the panel and the icon");
    let opened = tree(&[Mark::Icon {
        icon: Icon::FoldOpen,
        col: 0,
        cols: 1,
        ink: Color::Rgb(1, 2, 3),
    }]);
    marks.show(
        &opened,
        &layout(&opened),
        CellSize::DEFAULT,
        fallback(),
        &mut screen,
    );
    let wire = present(&mut screen);
    assert_eq!(transmits(&wire), 1, "the changed glyph goes out once");
    assert_eq!(marks.minted, 2, "into the slot the old glyph held");
    assert!(marks.free.is_empty(), "nothing vacated");
    assert_eq!(
        wire.windows(6).filter(|w| w == b"\x1b_Ga=d").count(),
        1,
        "the superseded image's placement is deleted, no slot's"
    );
}
