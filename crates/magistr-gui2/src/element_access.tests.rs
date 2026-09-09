use accesskit::{NodeId, Role};
use qs_gpu::color::Srgba;

use super::*;
use crate::element_layout::ElementLayout;
use crate::text::TextEngine;
use crate::voice::Voice;

fn white() -> Srgba {
    Srgba::new(1.0, 1.0, 1.0, 1.0)
}

fn label() -> Voice {
    Voice {
        family: "sans",
        px: 12.0,
        weight: 550,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    }
}

fn published() -> (Vec<AccessNode>, ElementLayout) {
    let tree = Element::surface()
        .label("cockpit")
        .role(Role::Window)
        .child(
            Element::row()
                .width(120.0)
                .height(30.0)
                .role(Role::Button)
                .label("Run")
                .child(Element::text(label(), white(), "Run")),
        )
        .child(Element::text(label(), white(), "quiet"));
    let mut text = TextEngine::embedded();
    let layout = ElementLayout::compute(&tree, 200.0, 100.0, &mut text).expect("layout");
    (access_of(&tree, &layout), layout)
}

#[test]
fn every_element_becomes_a_node_with_its_role_and_children() {
    let (nodes, _) = published();
    assert_eq!(nodes.len(), 4);
    assert_eq!(nodes[0].role, Role::Window);
    assert_eq!(nodes[0].children, vec![NodeId(1), NodeId(3)]);
    assert_eq!(nodes[1].role, Role::Button);
    assert_eq!(nodes[1].children, vec![NodeId(2)]);
    assert!(nodes[2].children.is_empty());
}

#[test]
fn a_text_run_is_its_own_label_and_a_declared_label_wins() {
    let (nodes, _) = published();
    assert_eq!(nodes[0].label, "cockpit");
    assert_eq!(nodes[1].label, "Run");
    assert_eq!(nodes[3].label, "quiet", "a run speaks its content");
}

#[test]
fn bounds_are_the_placed_pixel_boxes() {
    let (nodes, layout) = published();
    let [x, y, w, h] = layout.rect(1);
    assert_eq!(
        nodes[1].bounds,
        Some((
            f64::from(x),
            f64::from(y),
            f64::from(x + w),
            f64::from(y + h)
        ))
    );
}
