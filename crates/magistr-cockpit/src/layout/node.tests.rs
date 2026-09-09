use taffy::prelude::{auto, percent};

use super::*;

#[test]
fn constructors_pick_the_flex_direction() {
    assert_eq!(
        Node::row().flex_style(Basis::CELLS).flex_direction,
        FlexDirection::Row
    );
    assert_eq!(
        Node::column().flex_style(Basis::CELLS).flex_direction,
        FlexDirection::Column
    );
    assert_eq!(Node::leaf().flex_style(Basis::CELLS).display, Display::Flex);
    assert_eq!(
        Node::text("x").flex_style(Basis::CELLS).flex_direction,
        FlexDirection::Row
    );
}

#[test]
fn defaults_match_css_flex_items() {
    let style = Node::leaf().flex_style(Basis::CELLS);
    assert_eq!(style.flex_grow, 0.0);
    assert_eq!(style.flex_shrink, 1.0);
    assert_eq!(style.flex_wrap, FlexWrap::NoWrap);
    assert_eq!(
        style.size,
        Size {
            width: auto(),
            height: auto()
        }
    );
    assert_eq!(style.padding.left, length(0.0));
    assert_eq!(style.gap.width, length(0.0));
}

#[test]
fn constraints_map_onto_the_style() {
    let style = Node::row()
        .width(Length::Cells(24))
        .height(Length::Percent(0.5))
        .min_width(Length::Cells(10))
        .max_width(Length::Cells(40))
        .min_height(Length::Cells(1))
        .max_height(Length::Percent(0.9))
        .grow(2.0)
        .shrink(0.0)
        .wrap()
        .gap(1)
        .padding(2)
        .flex_style(Basis::CELLS);
    assert_eq!(style.size.width, length(24.0));
    assert_eq!(style.size.height, percent(0.5));
    assert_eq!(style.min_size.width, length(10.0));
    assert_eq!(style.max_size.width, length(40.0));
    assert_eq!(style.min_size.height, length(1.0));
    assert_eq!(style.max_size.height, percent(0.9));
    assert_eq!(style.flex_grow, 2.0);
    assert_eq!(style.flex_shrink, 0.0);
    assert_eq!(style.flex_wrap, FlexWrap::Wrap);
    assert_eq!(style.gap.width, length(1.0));
    assert_eq!(style.gap.height, length(1.0));
    assert_eq!(style.padding.top, length(2.0));
    assert_eq!(style.padding.right, length(2.0));
}

#[test]
fn ids_children_and_text_are_readable() {
    let id = WidgetId::new(9);
    let node = Node::column()
        .id(id)
        .child(Node::text("hello world"))
        .children([Node::leaf(), Node::leaf()]);
    assert_eq!(node.widget_id(), Some(id));
    assert_eq!(node.child_nodes().len(), 3);
    assert!(node.text_metrics().is_none());
    assert_eq!(
        node.child_nodes()[0]
            .text_metrics()
            .map(TextMetrics::max_content_width),
        Some(11)
    );
    assert!(Node::leaf().widget_id().is_none());
}

#[test]
fn a_viewport_scrolls_vertically_and_shrinks_below_its_content() {
    let style = Node::column().viewport().flex_style(Basis::CELLS);
    assert_eq!(style.overflow.y, Overflow::Scroll);
    assert_eq!(style.overflow.x, Overflow::Visible);
    assert_eq!(style.min_size.height, length(0.0));
    assert_eq!(style.scrollbar_width, 0.0);

    let plain = Node::column().flex_style(Basis::CELLS);
    assert_eq!(plain.overflow.y, Overflow::Visible);
    assert_eq!(plain.min_size.height, auto());
}

#[test]
fn an_explicit_basis_replaces_the_content_derived_one() {
    assert_eq!(Node::leaf().flex_style(Basis::CELLS).flex_basis, auto());
    assert_eq!(
        Node::leaf()
            .basis(Length::Cells(0))
            .flex_style(Basis::CELLS)
            .flex_basis,
        length(0.0),
        "CSS `flex: 1 1 0` — ask for nothing, take only a share"
    );
    assert_eq!(
        Node::leaf()
            .basis(Length::Percent(0.5))
            .flex_style(Basis::CELLS)
            .flex_basis,
        percent(0.5)
    );
}

#[test]
fn align_and_justify_map_onto_the_style() {
    let style = Node::row()
        .align_items(Align::Center)
        .justify_content(Justify::End)
        .flex_style(Basis::CELLS);
    assert_eq!(style.align_items, Some(Align::Center.taffy()));
    assert_eq!(style.justify_content, Some(Justify::End.taffy()));

    let plain = Node::row().flex_style(Basis::CELLS);
    assert_eq!(
        plain.align_items,
        Some(Align::Stretch.taffy()),
        "the CSS defaults, stated rather than left to the engine"
    );
    assert_eq!(plain.justify_content, Some(Justify::Start.taffy()));
}

#[test]
fn an_overlay_is_absolute_and_an_in_flow_node_is_not() {
    let plain = Node::leaf();
    assert!(!plain.is_overlay());
    let style = plain.flex_style(Basis::CELLS);
    assert_eq!(style.position, Position::Relative);
    assert_eq!(style.inset, TaffyRect::auto());

    let overlay = Node::leaf().overlay(Inset::none().top(Length::Cells(1)).right(Length::Cells(2)));
    assert!(overlay.is_overlay());
    let style = overlay.flex_style(Basis::CELLS);
    assert_eq!(style.position, Position::Absolute);
    assert_eq!(style.inset.top, length(1.0));
    assert_eq!(style.inset.right, length(2.0));
    assert_eq!(style.inset.left, auto(), "an unset edge is unconstrained");
    assert_eq!(style.inset.bottom, auto());
}

#[test]
fn a_centered_overlay_is_a_full_bleed_layer_centring_its_child() {
    let modal = Node::leaf().width(Length::Cells(10));
    let node = Node::centered_overlay(modal.clone());
    assert!(node.is_overlay());
    assert_eq!(node.child_nodes(), &[modal], "the child is untouched");
    let style = node.flex_style(Basis::CELLS);
    assert_eq!(style.flex_direction, FlexDirection::Column);
    assert_eq!(style.position, Position::Absolute);
    assert_eq!(style.inset, TaffyRect::zero());
    assert_eq!(style.justify_content, Some(Justify::Center.taffy()));
    assert_eq!(style.align_items, Some(Align::Center.taffy()));
}
