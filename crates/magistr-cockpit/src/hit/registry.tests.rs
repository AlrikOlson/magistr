use super::*;
use crate::layout::{Inset, LayoutTree, Length, Node};

fn id(raw: u32) -> WidgetId {
    WidgetId::new(raw)
}

#[test]
fn an_empty_registry_hits_nothing() {
    let registry = HitRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    assert_eq!(registry.hit(0, 0), None);
}

#[test]
fn a_rect_answers_inside_and_not_outside() {
    let mut registry = HitRegistry::new();
    registry.register(Rect::new(2, 2, 3, 3), 0, id(1));
    assert_eq!(registry.hit(2, 2), Some(id(1)));
    assert_eq!(registry.hit(4, 4), Some(id(1)));
    assert_eq!(registry.hit(5, 4), None);
    assert_eq!(registry.hit(1, 2), None);
}

#[test]
fn empty_rects_are_ignored() {
    let mut registry = HitRegistry::new();
    registry.register(Rect::new(0, 0, 0, 5), 0, id(1));
    registry.register(Rect::new(0, 0, 5, 0), 0, id(2));
    assert!(registry.is_empty());
}

#[test]
fn a_higher_z_wins_regardless_of_registration_order() {
    let mut registry = HitRegistry::new();
    registry.register(Rect::new(0, 0, 10, 10), 5, id(1));
    registry.register(Rect::new(0, 0, 10, 10), 0, id(2));
    assert_eq!(registry.hit(3, 3), Some(id(1)));
}

#[test]
fn at_equal_z_the_last_registered_wins() {
    let mut registry = HitRegistry::new();
    registry.register(Rect::new(0, 0, 10, 10), 0, id(1));
    registry.register(Rect::new(0, 0, 10, 10), 0, id(2));
    assert_eq!(registry.hit(3, 3), Some(id(2)));
}

#[test]
fn dock_over_stage_and_overlay_over_everything() {
    let mut registry = HitRegistry::new();
    let stage = id(10);
    let dock = id(20);
    let overlay = id(30);
    // The overlay is registered first but stacks highest.
    registry.register(Rect::new(20, 5, 40, 10), 10, overlay);
    registry.register(Rect::new(0, 0, 80, 24), 0, stage);
    registry.register(Rect::new(50, 0, 30, 24), 0, dock);
    assert_eq!(registry.hit(10, 2), Some(stage));
    assert_eq!(registry.hit(55, 2), Some(dock), "dock covers the stage");
    assert_eq!(
        registry.hit(55, 8),
        Some(overlay),
        "overlay covers the dock"
    );
    assert_eq!(
        registry.hit(25, 8),
        Some(overlay),
        "overlay covers the stage"
    );
    assert_eq!(registry.hit(60, 20), Some(dock), "below the overlay");
}

#[test]
fn register_layout_puts_children_above_parents() {
    let root = Node::row()
        .id(id(0))
        .child(Node::leaf().id(id(1)).width(Length::Cells(10)))
        .child(
            Node::column()
                .id(id(2))
                .grow(1.0)
                .child(Node::leaf().id(id(3)).grow(1.0)),
        )
        .child(Node::leaf().width(Length::Cells(5)));
    let resolved = LayoutTree::new()
        .compute(&root, 40, 4)
        .expect("layout succeeds");
    let mut registry = HitRegistry::new();
    registry.register_layout(&resolved, 100);
    assert_eq!(registry.len(), 4, "only id-carrying rects are registered");
    assert_eq!(registry.hit(3, 1), Some(id(1)));
    assert_eq!(
        registry.hit(20, 1),
        Some(id(3)),
        "grandchild over child over root"
    );
    assert_eq!(
        registry.hit(37, 1),
        Some(id(0)),
        "the anonymous leaf falls through to the root"
    );
    assert_eq!(registry.hit(40, 1), None);
}

#[test]
fn clear_forgets_every_rect() {
    let mut registry = HitRegistry::new();
    registry.register(Rect::new(0, 0, 10, 10), 0, id(1));
    registry.clear();
    assert!(registry.is_empty());
    assert_eq!(registry.hit(1, 1), None);
}

#[test]
fn a_click_inside_a_modal_resolves_to_the_modal_not_to_what_it_covers() {
    // The stage nests four levels deep; the modal is a child of the root.
    // Ordering by depth alone would hand every click to the deep leaf.
    let deep = Node::column().id(id(1)).grow(1.0).child(
        Node::column().id(id(2)).grow(1.0).child(
            Node::column()
                .id(id(3))
                .grow(1.0)
                .child(Node::leaf().id(id(4)).grow(1.0)),
        ),
    );
    let modal = Node::leaf()
        .id(id(5))
        .width(Length::Cells(20))
        .height(Length::Cells(6));
    let root = Node::column()
        .id(id(0))
        .child(deep)
        .child(Node::centered_overlay(modal).id(id(6)));
    let resolved = LayoutTree::new()
        .compute(&root, 40, 12)
        .expect("layout succeeds");
    let mut registry = HitRegistry::new();
    registry.register_layout(&resolved, 0);

    assert_eq!(
        resolved.rect(id(5)),
        Some(Rect::new(10, 3, 20, 6)),
        "the modal is centred on the screen"
    );
    assert_eq!(
        registry.hit(20, 6),
        Some(id(5)),
        "the modal takes the click, over a leaf three levels deeper"
    );
    assert_eq!(
        registry.hit(1, 1),
        Some(id(6)),
        "outside the modal its full-bleed layer still swallows the click \
         — a centred overlay is a scrim, which is what makes it modal"
    );
}

#[test]
fn a_later_modal_covers_deeper_graph_label_overlays_in_the_same_layer() {
    let label = Node::leaf()
        .id(id(1))
        .overlay(Inset::none().left(Length::Cells(10)).top(Length::Cells(3)))
        .width(Length::Cells(20))
        .height(Length::Cells(6));
    let mut graph = Node::column().grow(1.0).child(label);
    for _ in 0..8 {
        graph = Node::column().grow(1.0).child(graph);
    }
    let modal = Node::centered_overlay(
        Node::leaf()
            .id(id(2))
            .width(Length::Cells(20))
            .height(Length::Cells(6)),
    );
    let root = Node::column().child(graph).child(modal);
    let resolved = LayoutTree::new().compute(&root, 40, 12).unwrap();
    let mut registry = HitRegistry::new();
    registry.register_layout(&resolved, 0);
    assert_eq!(registry.hit(20, 6), Some(id(2)));
    registry.clear();
    registry.register_layout_px(&resolved, 0);
    assert_eq!(registry.hit_at(20.5, 6.5), Some(id(2)));
}

#[test]
fn an_overlay_layer_outranks_every_in_flow_node_however_deep() {
    let mut deep = Node::leaf().id(id(9)).grow(1.0);
    for raw in (1..9).rev() {
        deep = Node::column().id(id(raw)).grow(1.0).child(deep);
    }
    let root = Node::column().id(id(0)).child(deep).child(
        Node::leaf()
            .id(id(10))
            .overlay(Inset::none().top(Length::Cells(2)).left(Length::Cells(2)))
            .width(Length::Cells(4))
            .height(Length::Cells(2)),
    );
    let resolved = LayoutTree::new()
        .compute(&root, 20, 10)
        .expect("layout succeeds");
    assert_eq!(resolved.max_depth(), 9, "the in-flow chain is nine deep");
    let mut registry = HitRegistry::new();
    registry.register_layout(&resolved, 0);
    assert_eq!(
        registry.hit(3, 3),
        Some(id(10)),
        "a depth-1 overlay beats the depth-9 leaf under it"
    );
    assert_eq!(
        registry.hit(10, 8),
        Some(id(9)),
        "beside the overlay the deep leaf answers as before"
    );
}

#[test]
fn a_reported_area_reaches_control_edges_and_prefers_the_greater_visible_overlap() {
    let mut registry = HitRegistry::new();
    registry.register_px(PxRect::new(0.0, 0.0, 400.0, 200.0), 0, id(0));
    registry.register_px(PxRect::new(97.0, 8.0, 74.0, 8.0), 0, id(1));
    registry.register_px(PxRect::new(176.0, 8.0, 44.0, 8.0), 0, id(2));
    assert_eq!(registry.hit_at(95.0, 10.0), Some(id(0)));
    assert_eq!(
        registry.hit_region(PxRect::new(90.0, 0.0, 10.0, 20.0)),
        Some(id(1))
    );
    assert_eq!(
        registry.hit_region(PxRect::new(170.0, 0.0, 10.0, 20.0)),
        Some(id(2))
    );
    assert_eq!(
        registry.hit_region(PxRect::new(300.0, 160.0, 10.0, 20.0)),
        Some(id(0))
    );
    assert_eq!(
        registry.hit_region(PxRect::new(400.0, 0.0, 10.0, 20.0)),
        None
    );
    assert_eq!(registry.hit_region(PxRect::default()), None);
}

#[test]
fn area_reports_cannot_activate_controls_covered_by_an_overlay() {
    let mut registry = HitRegistry::new();
    let bounds = PxRect::new(0.0, 0.0, 400.0, 200.0);
    registry.register_px(bounds, 0, id(0));
    registry.register_px(PxRect::new(90.0, 8.0, 100.0, 20.0), 0, id(1));
    registry.register_px(bounds, 10, id(2));
    let region = PxRect::new(90.0, 0.0, 10.0, 20.0);
    assert_eq!(registry.hit_region(region), Some(id(2)));
    registry.register_px(PxRect::new(97.0, 8.0, 74.0, 8.0), 10, id(3));
    assert_eq!(registry.hit_region(region), Some(id(3)));
}
