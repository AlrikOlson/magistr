use super::*;
use crate::capability::Mode;
use crate::cockpit::{Msg, Size, update};
use crate::theme::{Icons, Theme};

fn pen() -> Pen {
    Pen::for_theme(&Theme::default())
}

fn plain() -> Model {
    Model::new(Size::new(80, 24))
}

fn pixels() -> Model {
    let mut m = plain();
    update(
        &mut m,
        Msg::Mode(Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    m
}

#[test]
fn a_plain_terminal_gets_the_glyphs_and_no_marks() {
    let pen = pen();
    let node = Marked::new(&plain(), &pen)
        .icon(Icon::Done)
        .text(" check ")
        .icon(Icon::Pinned)
        .node();
    let text = node.text_metrics().expect("text").text().to_owned();
    assert_eq!(
        text,
        format!("{} check {}", Icons::UNICODE.done, Icons::UNICODE.pinned)
    );
    assert!(node.marks().is_empty());
}

#[test]
fn a_graphics_terminal_places_icons_in_separate_slots_beside_proportional_text() {
    let pen = pen();
    let node = Marked::new(&pixels(), &pen)
        .icon(Icon::Done)
        .text(" check ")
        .icon(Icon::Pinned)
        .node();
    let children = node.child_nodes();
    assert_eq!(children.len(), 3);
    assert_eq!(children[1].text_metrics().expect("label").text(), " check ");
    for (child, icon) in [(&children[0], Icon::Done), (&children[2], Icon::Pinned)] {
        assert!(
            matches!(child.marks(), [Mark::Icon { icon: actual, col: 0, cols: 1, .. }] if *actual == icon)
        );
    }
}

#[test]
fn a_segment_is_one_rigid_row_in_its_style() {
    let pen = pen();
    let node = Marked::new(&plain(), &pen)
        .text("x")
        .segment(pen.note.clone());
    assert_eq!(node.cell_style(), Some(&pen.note));
}
