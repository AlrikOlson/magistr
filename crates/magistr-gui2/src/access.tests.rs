use super::*;

use accesskit::Role;
use magistr_cockpit::cockpit::{FrameClock, Model, Msg, Size, Widget, update};
use magistr_cockpit::input::{KeyCode, KeyEvent};

use crate::fixture::Fixture;
use crate::frame::Frame;

fn clock() -> FrameClock {
    FrameClock::new(0, std::time::Duration::ZERO)
}

/// The tree of a painted frame: the mock session through the whole
/// build, at the captures' own size.
fn frame_of(model: &mut Model) -> Frame {
    let mut frame = Frame::new().expect("a grid");
    let (width, height) = Fixture::CAPTURE;
    let _ = frame
        .build(model, &clock(), width, height)
        .expect("the frame builds");
    frame
}

fn session() -> (Frame, Model) {
    let mut model = Fixture::mock("05", Size::new(122, 32)).expect("view 05");
    let frame = frame_of(&mut model);
    (frame, model)
}

/// Every published node is reachable from the window by child edges —
/// an orphan would be a widget a screen reader can never arrive at.
#[test]
fn every_node_is_reachable_from_the_window() {
    let (frame, _) = session();
    let tree = frame.access(1.0);
    let nodes = tree.nodes();
    assert!(nodes.len() > 3, "the session publishes its widgets");
    let mut reached = vec![nodes[0].id];
    let mut frontier = vec![&nodes[0]];
    while let Some(node) = frontier.pop() {
        for child in &node.children {
            reached.push(*child);
            frontier.extend(nodes.iter().filter(|n| n.id == *child));
        }
    }
    for node in nodes {
        assert!(
            reached.contains(&node.id),
            "{:?} ({}) is unreachable",
            node.id,
            node.label
        );
    }
}

/// A node's id is the cockpit's widget id plus one — the same identity
/// the hit registry answers with, so the two cannot drift.
#[test]
fn a_nodes_id_is_the_cockpits_widget_id() {
    let (frame, _) = session();
    let tree = frame.access(1.0);
    let composer = tree.find(Widget::Composer).expect("the composer");
    assert_eq!(
        composer.id,
        AccessTree::node_id(Widget::Composer.id()),
        "the node is the widget"
    );
}

/// Roles and names come from the cockpit's own vocabulary: the
/// composer is a text input, the divider a splitter, a crumb a link.
#[test]
fn the_roles_and_names_come_from_the_widget_ids() {
    let (frame, _) = session();
    let tree = frame.access(1.0);
    assert_eq!(
        tree.find(Widget::Composer).expect("composer").role,
        Role::TextInput
    );
    assert_eq!(
        tree.find(Widget::RailDivider).expect("divider").role,
        Role::Splitter
    );
    let crumb = tree.find(Widget::CrumbProjects).expect("the crumb");
    assert_eq!(crumb.role, Role::Link);
    assert_eq!(crumb.label, "projects");
}

/// The composer carries what it holds as a value — the window's prompt
/// is readable, not just named.
#[test]
fn the_composer_carries_its_text_as_a_value() {
    let mut model = Fixture::mock("05", Size::new(122, 32)).expect("view 05");
    for ch in "ship it".chars() {
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(ch))));
    }
    let frame = frame_of(&mut model);
    let tree = frame.access(1.0);
    let composer = tree.find(Widget::Composer).expect("the composer");
    let value = composer.value.as_deref().expect("a value");
    assert!(value.contains("ship it"), "{value:?}");
}

/// Bounds are the element's own placed box in logical points — so a 2x
/// window reports half the physical pixels and a screen reader's
/// highlight lands on the widget rather than on a quarter of it.
#[test]
fn bounds_are_the_placed_box_in_logical_points() {
    let (frame, _) = session();
    let at_one = frame.access(1.0);
    let at_two = frame.access(2.0);
    let pill = at_one.find(Widget::Composer).expect("the composer");
    let (x0, y0, x1, y1) = pill.bounds.expect("bounded");
    assert!(x1 > x0 && y1 > y0, "a real box");
    let scaled = at_two.find(Widget::Composer).expect("the composer");
    let (sx0, sy0, sx1, sy1) = scaled.bounds.expect("bounded");
    for (one, two) in [(x0, sx0), (y0, sy0), (x1, sx1), (y1, sy1)] {
        assert!((two - one / 2.0).abs() < 1e-9, "{one} vs {two}");
    }
}

/// The update AccessKit is handed is a *full* tree with a root and a
/// focus inside it, because the adapter's activation handler returns
/// none and the first update after that must carry everything.
#[test]
fn the_update_is_a_whole_tree_rooted_at_the_window() {
    let (frame, _) = session();
    let tree = frame.access(1.0);
    let update = tree.update(Some(Widget::Composer));
    assert_eq!(update.nodes.len(), tree.nodes().len());
    let root = update.tree.as_ref().expect("a tree is declared");
    assert_eq!(root.root, AccessTree::WINDOW);
    assert_eq!(update.focus, AccessTree::node_id(Widget::Composer.id()));
    // A focus the frame did not place falls back to the window.
    let update = tree.update(Some(Widget::Prompt));
    assert_eq!(update.focus, AccessTree::WINDOW);
}

/// Before any frame was built there is nothing to describe, and the
/// tree says so rather than describing a screen nobody painted.
#[test]
fn an_unbuilt_frame_publishes_nothing() {
    let frame = Frame::new().expect("a grid");
    assert!(frame.access(1.0).nodes().is_empty());
}
