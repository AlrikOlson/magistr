use super::*;
use crate::cockpit::Widget;
use crate::layout::{Basis, Length};
use crate::render::Attrs;
use crate::theme::Pen;
use magistr_theme::catppuccin_mocha;

fn pen() -> Pen {
    Pen::of(&catppuccin_mocha())
}

/// A card: a bare container carrying the id, a styled label inside it,
/// and one child that only inherits.
fn card(id: crate::widget_id::WidgetId, pen: &Pen) -> Node {
    Node::column()
        .id(id)
        .child(Node::text("label").style(pen.stage.clone()))
        .child(Node::text("inherited"))
}

#[test]
fn nothing_hovered_changes_nothing() {
    let pen = pen();
    let mut root = card(Widget::stage_row(0), &pen);
    let before = root.clone();
    assert!(!answer(&mut root, None, None, &pen));
    assert_eq!(root, before);
}

#[test]
fn an_inert_widget_changes_nothing() {
    let pen = pen();
    let mut root = card(Widget::Stage.id(), &pen);
    let before = root.clone();
    assert!(!answer(&mut root, Some(Widget::Stage.id()), None, &pen));
    assert_eq!(root, before);
}

/// A hovered row is raised as a band: the container it hangs on gets the
/// hover ground even though it carried no style at all, and the label
/// that did carry one is raised with it. The child that merely inherits
/// is left bare, so the map never turns a plain container into a band.
#[test]
fn a_hovered_row_raises_the_container_and_every_style_under_it() {
    let pen = pen();
    let id = Widget::stage_row(2);
    let mut root = Node::column().style(pen.base.clone()).child(card(id, &pen));
    assert!(answer(&mut root, Some(id), None, &pen));
    let row = &root.child_nodes()[0];
    assert_eq!(
        row.cell_style().map(|s| s.bg),
        Some(pen.hover_ground),
        "the bare container answers on the ground it inherited"
    );
    assert_eq!(
        row.child_nodes()[0].cell_style().map(|s| s.bg),
        Some(pen.hover_ground),
        "the label is raised with it"
    );
    assert!(
        row.child_nodes()[1].cell_style().is_none(),
        "an inheriting child stays bare"
    );
}

/// A hovered button lifts: the label's colour moves and it goes bold.
#[test]
fn a_hovered_button_lifts_its_label() {
    let pen = pen();
    let id = Widget::Accept.id();
    let label = Node::text("[Accept]").style(pen.allow.clone()).id(id);
    let mut root = Node::row().style(pen.base.clone()).child(label);
    assert!(answer(&mut root, Some(id), None, &pen));
    let style = root.child_nodes()[0]
        .cell_style()
        .expect("the button keeps a style");
    assert_eq!(style.bg, pen.allow.bg, "a lift never moves the ground");
    assert_ne!(style.fg, pen.allow.fg, "the lift is visible");
    assert!(style.attrs.contains(Attrs::BOLD));
}

/// A hovered link underlines on top of the lift.
#[test]
fn a_hovered_link_underlines() {
    let pen = pen();
    let id = Widget::trace_link(9);
    let mut root = Node::row()
        .style(pen.base.clone())
        .child(Node::text("[revises 8]").style(pen.link.clone()).id(id));
    assert!(answer(&mut root, Some(id), None, &pen));
    let style = root.child_nodes()[0].cell_style().expect("styled");
    assert!(style.attrs.contains(Attrs::UNDERLINE));
    assert!(style.attrs.contains(Attrs::BOLD));
}

/// An id nothing on screen carries is simply not found — the pass never
/// panics on a stale hover from the frame before.
#[test]
fn an_id_no_node_carries_is_not_found() {
    let pen = pen();
    let mut root = card(Widget::stage_row(0), &pen);
    let before = root.clone();
    assert!(!answer(&mut root, Some(Widget::stage_row(9)), None, &pen));
    assert_eq!(root, before);
}

/// Hover is paint, never layout: the constraints the flex engine reads
/// come out of the pass exactly as they went in.
#[test]
fn the_pass_touches_no_constraint() {
    let pen = pen();
    let id = Widget::launch_session(0);
    let build = || {
        Node::column().style(pen.base.clone()).child(
            Node::row()
                .id(id)
                .height(Length::Cells(1))
                .child(Node::text("session").style(pen.stage.clone())),
        )
    };
    let plain = build();
    let mut hovered = build();
    assert!(answer(&mut hovered, Some(id), None, &pen));
    let strip = |node: &Node| format!("{:?}", node.flex_style(Basis::CELLS));
    assert_eq!(
        strip(&hovered.child_nodes()[0]),
        strip(&plain.child_nodes()[0])
    );
}

#[test]
fn a_held_widget_is_its_hovered_self_filled_in() {
    // a press is the same widget held, not a fourth role — so
    // it composes onto whichever treatment the pointer already gave it,
    // and it swaps the pair the pen already cleared the floor with
    // rather than inventing a third colour.
    let pen = pen();
    let id = Widget::TabHome.id();
    let build = || {
        Node::row().child(
            Node::text("Home")
                .id(id)
                .style(pen.rail.clone())
                .height(Length::Cells(1)),
        )
    };
    let (mut hovered, mut held) = (build(), build());
    assert!(answer(&mut hovered, Some(id), None, &pen));
    assert!(answer(&mut held, Some(id), Some(id), &pen));
    assert_ne!(
        format!("{hovered:?}"),
        format!("{held:?}"),
        "a held tab has to look different from one merely under the pointer"
    );

    // A button the pointer has dragged off is not being pressed.
    let mut elsewhere = build();
    assert!(answer(
        &mut elsewhere,
        Some(id),
        Some(Widget::TabTurn.id()),
        &pen
    ));
    assert_eq!(format!("{elsewhere:?}"), format!("{hovered:?}"));
}

#[test]
fn structure_stays_silent_under_the_button_as_it_is_under_the_pointer() {
    // Tinting a panel because a click landed in its padding would be
    // noise, so an inert widget answers neither.
    let pen = pen();
    let id = Widget::Stage.id();
    let mut root = Node::row().child(
        Node::text("stage")
            .id(id)
            .style(pen.stage.clone())
            .height(Length::Cells(1)),
    );
    assert!(!answer(&mut root, Some(id), Some(id), &pen));
}
