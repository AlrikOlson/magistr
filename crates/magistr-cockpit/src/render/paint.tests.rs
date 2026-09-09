use super::*;
use crate::WidgetId;
use crate::layout::{LayoutTree, Length};
use crate::render::{Attrs, Color};

fn text_at(buffer: &Buffer, row: u16) -> String {
    (0..buffer.width())
        .filter_map(|col| buffer.get(col, row))
        .filter(|cell| !cell.is_continuation())
        .map(|cell| cell.symbol.as_str())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn text_leaves_land_in_their_rects_wrapped_and_clipped() {
    let root = Node::column()
        .child(Node::text("header line").height(Length::Cells(1)))
        .child(
            Node::text("one two three four five six")
                .id(WidgetId::new(1))
                .height(Length::Cells(2)),
        )
        .child(Node::text("footer").height(Length::Cells(1)));
    let mut buffer = Buffer::new(10, 4);
    let resolved = LayoutTree::new()
        .compute(&root, 10, 4)
        .expect("layout succeeds");
    paint(&root, &resolved, &mut buffer);
    assert_eq!(text_at(&buffer, 0), "header");
    assert_eq!(text_at(&buffer, 1), "one two");
    assert_eq!(text_at(&buffer, 2), "three four");
    assert_eq!(text_at(&buffer, 3), "footer");
}

#[test]
fn a_styled_container_fills_its_rect_as_a_band() {
    let bar = Style::new().fg(Color::Rgb(5, 5, 5)).bg(Color::Rgb(8, 8, 8));
    let chip = Style::new().fg(Color::Rgb(1, 1, 1)).bg(Color::Rgb(2, 2, 2));
    let root = Node::column().child(
        Node::row()
            .style(bar.clone())
            .height(Length::Cells(1))
            .child(Node::text("aa").style(chip.clone()).shrink(0.0))
            .child(Node::text("bb").shrink(0.0)),
    );
    let mut buffer = Buffer::new(8, 2);
    let resolved = LayoutTree::new().compute(&root, 8, 2).expect("layout");
    paint(&root, &resolved, &mut buffer);
    // The bar's whole row wears its background — the tail beyond its
    // children included — so a styled row reads as a band, not floating
    // text.
    assert_eq!(buffer.get(0, 0).map(|c| &c.style), Some(&chip));
    assert_eq!(buffer.get(2, 0).map(|c| &c.style), Some(&bar));
    assert_eq!(buffer.get(7, 0).map(|c| &c.style), Some(&bar));
    // The row below stays untouched.
    assert_ne!(buffer.get(0, 1).map(|c| &c.style), Some(&bar));
}

#[test]
fn containers_paint_nothing() {
    let root = Node::column().child(Node::leaf().id(WidgetId::new(7)).grow(1.0));
    let mut buffer = Buffer::new(4, 2);
    let resolved = LayoutTree::new().compute(&root, 4, 2).expect("layout");
    paint(&root, &resolved, &mut buffer);
    assert!(buffer.cells().iter().all(|c| c.symbol.as_str() == " "));
}

#[test]
fn an_unstyled_tree_paints_in_the_default_pen_on_a_blank_buffer() {
    let root = Node::column().child(Node::text("hi").height(Length::Cells(1)));
    let mut buffer = Buffer::new(4, 2);
    let resolved = LayoutTree::new().compute(&root, 4, 2).expect("layout");
    paint(&root, &resolved, &mut buffer);
    assert!(buffer.cells().iter().all(|c| c.style == Style::default()));
    assert_eq!(text_at(&buffer, 0), "hi");
}

#[test]
fn the_root_style_fills_the_screen_and_descendants_inherit_or_override() {
    let base = Style::new().fg(Color::Rgb(1, 2, 3)).bg(Color::Rgb(9, 9, 9));
    let header = Style::new()
        .fg(Color::Rgb(7, 7, 7))
        .bg(Color::Rgb(4, 4, 4))
        .attrs(Attrs::BOLD);
    let root = Node::column()
        .style(base.clone())
        .child(
            Node::text("top")
                .style(header.clone())
                .height(Length::Cells(1)),
        )
        .child(
            Node::column()
                .grow(1.0)
                .child(Node::text("body").height(Length::Cells(1))),
        );
    let mut buffer = Buffer::new(6, 3);
    let resolved = LayoutTree::new().compute(&root, 6, 3).expect("layout");
    paint(&root, &resolved, &mut buffer);
    // Row 0: the header's own style, text and padding alike.
    assert_eq!(text_at(&buffer, 0), "top");
    assert_eq!(buffer.get(0, 0).map(|c| &c.style), Some(&header));
    assert_eq!(buffer.get(5, 0).map(|c| &c.style), Some(&base));
    // Row 1: the body inherits the base through an unstyled container.
    assert_eq!(text_at(&buffer, 1), "body");
    assert_eq!(buffer.get(0, 1).map(|c| &c.style), Some(&base));
    // Row 2: nothing was drawn there, the fill shows.
    assert_eq!(text_at(&buffer, 2), "");
    assert_eq!(buffer.get(3, 2).map(|c| &c.style), Some(&base));
    assert!(buffer.cells().iter().all(|c| c.style.bg != Color::Default));
}

#[test]
fn a_framed_container_wears_the_rounded_ring_and_content_sits_inside() {
    let root = Node::column()
        .frame(Frame::new(Style::new()))
        .child(Node::text("hi").height(Length::Cells(1)));
    let mut buffer = Buffer::new(6, 4);
    let resolved = LayoutTree::new().compute(&root, 6, 4).expect("layout");
    paint(&root, &resolved, &mut buffer);
    assert_eq!(text_at(&buffer, 0), "╭────╮");
    assert_eq!(text_at(&buffer, 1), "│hi  │");
    assert_eq!(text_at(&buffer, 2), "│    │");
    assert_eq!(text_at(&buffer, 3), "╰────╯");
}

#[test]
fn the_title_embeds_in_the_top_border_in_its_own_style_and_meta_right_aligns() {
    let border = Style::new().fg(Color::Rgb(1, 1, 1));
    let heading = Style::new().fg(Color::Rgb(2, 2, 2)).attrs(Attrs::BOLD);
    let root = Node::column().frame(
        Frame::new(border.clone())
            .title("plan")
            .meta("3/7")
            .title_style(heading.clone()),
    );
    let mut buffer = Buffer::new(20, 3);
    let resolved = LayoutTree::new().compute(&root, 20, 3).expect("layout");
    paint(&root, &resolved, &mut buffer);
    assert_eq!(text_at(&buffer, 0), "╭─ plan ───── 3/7 ─╮");
    assert_eq!(buffer.get(3, 0).map(|c| &c.style), Some(&heading));
    assert_eq!(buffer.get(0, 0).map(|c| &c.style), Some(&border));
    assert_eq!(buffer.get(13, 0).map(|c| &c.style), Some(&border));
}

#[test]
fn a_long_title_clips_and_never_touches_the_corners() {
    let root = Node::column().frame(Frame::new(Style::new()).title("a very long title"));
    let mut buffer = Buffer::new(10, 2);
    let resolved = LayoutTree::new().compute(&root, 10, 2).expect("layout");
    paint(&root, &resolved, &mut buffer);
    assert_eq!(text_at(&buffer, 0), "╭─ a ve ─╮");
    assert_eq!(text_at(&buffer, 1), "╰────────╯");
}

#[test]
fn meta_is_dropped_when_the_title_leaves_it_no_room() {
    let root = Node::column().frame(Frame::new(Style::new()).title("plan").meta("3/7"));
    let mut buffer = Buffer::new(10, 2);
    let resolved = LayoutTree::new().compute(&root, 10, 2).expect("layout");
    paint(&root, &resolved, &mut buffer);
    assert_eq!(text_at(&buffer, 0), "╭─ plan ─╮");
}

#[test]
fn a_rect_too_small_for_a_ring_paints_none() {
    let root = Node::column()
        .frame(Frame::new(Style::new()))
        .height(Length::Cells(1));
    let mut buffer = Buffer::new(4, 1);
    let resolved = LayoutTree::new().compute(&root, 4, 1).expect("layout");
    paint(&root, &resolved, &mut buffer);
    assert!(buffer.cells().iter().all(|c| c.symbol.as_str() == " "));
}
