use super::*;

#[test]
fn conversation_access_includes_the_visible_action_failure() {
    let mut model = Model::new(Size::new(122, 32))
        .with_surface(magistr_cockpit::cockpit::Surface::Native)
        .with_tab(magistr_cockpit::cockpit::StageTab::Turn);
    update(
        &mut model,
        Msg::Agent(magistr_application::sessions::AgentEvent::Text(
            "Working".into(),
        )),
    );
    update(
        &mut model,
        Msg::CommandFailed {
            cmd: magistr_cockpit::cockpit::Cmd::Cancel,
            error: "The connection closed. Try again.".into(),
        },
    );
    let frame = frame_of(&mut model);
    let tree = frame.access();
    let conversation = tree.find(Widget::Stage).unwrap();
    assert_eq!(conversation.label, "Conversation");
    let value = conversation.value.as_deref().unwrap();
    assert!(value.contains("Working"));
    assert!(value.contains("could not stop"));
    assert!(value.contains("Try again"));
    assert!(!conversation.actions.contains(&accesskit::Action::SetValue));
}

#[test]
fn access_actions_edit_focus_navigate_and_refuse_without_mutation() {
    use accesskit::{Action, ActionData, ActionRequest, NodeId, TreeId};
    let mut model = Model::new(Size::new(122, 32))
        .with_surface(magistr_cockpit::cockpit::Surface::Native)
        .with_tab(magistr_cockpit::cockpit::StageTab::Turn);
    update(
        &mut model,
        Msg::Paste("old first line\nold second line\n".into()),
    );
    let mut frame = frame_of(&mut model);
    let request = |frame: &Frame, widget: Widget, action, data| ActionRequest {
        target_tree: TreeId::ROOT,
        target_node: frame.access().find(widget).unwrap().id,
        action,
        data,
    };
    let edit = request(
        &frame,
        Widget::Composer,
        Action::SetValue,
        Some(ActionData::Value("  café\n@src/main.rs  ".into())),
    );
    for msg in frame.access_action(&model, edit).unwrap() {
        update(&mut model, msg);
    }
    assert_eq!(model.prompt().text(), "  café\n@src/main.rs  ");
    frame.build(&mut model, &clock(), 1132, 800).unwrap();
    let focus = request(&frame, Widget::TabRoadmap, Action::Focus, None);
    assert!(frame.access_action(&model, focus).unwrap().is_empty());
    assert_eq!(frame.focus_target(), Some(Widget::TabRoadmap.id()));
    let tree = frame.access();
    assert_eq!(
        tree.update(frame.focus_target()).focus,
        tree.find(Widget::TabRoadmap).unwrap().id
    );
    let before = model.clone();
    let focus_before = frame.focus_target();
    let unsupported = request(
        &frame,
        Widget::TabRoadmap,
        Action::SetValue,
        Some(ActionData::Value("bad".into())),
    );
    assert!(frame.access_action(&model, unsupported).is_err());
    assert!(
        frame
            .access_action(
                &model,
                ActionRequest {
                    target_tree: TreeId::ROOT,
                    target_node: NodeId(u64::MAX),
                    action: Action::Click,
                    data: None
                }
            )
            .is_err()
    );
    assert_eq!(model, before);
    assert_eq!(frame.focus_target(), focus_before);
    let click = request(&frame, Widget::TabRoadmap, Action::Click, None);
    for msg in frame.access_action(&model, click).unwrap() {
        update(&mut model, msg);
    }
    assert_eq!(model.tab(), magistr_cockpit::cockpit::StageTab::Roadmap);
    assert_eq!(model.prompt().text(), "  café\n@src/main.rs  ");
}

#[test]
fn published_targets_keep_ids_across_scale_but_retire_when_the_control_changes() {
    use accesskit::{Action, ActionRequest, TreeId};
    let mut model = Model::new(Size::new(122, 32))
        .with_surface(magistr_cockpit::cockpit::Surface::Native)
        .with_tab(magistr_cockpit::cockpit::StageTab::Turn);
    update(&mut model, Msg::Paste("Send this".into()));
    let mut frame = frame_of(&mut model);
    let old = frame.access().find(Widget::PromptAction).unwrap().id;
    frame.rescale(2.0).unwrap();
    frame.build(&mut model, &clock(), 2264, 1600).unwrap();
    assert_eq!(frame.access().find(Widget::PromptAction).unwrap().id, old);
    let request = ActionRequest {
        target_tree: TreeId::ROOT,
        target_node: old,
        action: Action::Click,
        data: None,
    };
    let mut commands = Vec::new();
    for msg in frame.access_action(&model, request.clone()).unwrap() {
        commands.extend(update(&mut model, msg));
    }
    assert!(
        commands.contains(&magistr_cockpit::cockpit::Cmd::Prompt("Send this".into())),
        "{commands:?}; tab={:?}",
        model.tab()
    );
    frame.build(&mut model, &clock(), 2264, 1600).unwrap();
    assert_ne!(
        frame.access().find(Widget::PromptAction).unwrap().id,
        old,
        "Send becoming Stop retires the old action"
    );
    assert!(frame.access_action(&model, request).is_err());
}

#[test]
fn accessible_palette_search_cannot_edit_or_activate_the_background() {
    use accesskit::{Action, ActionData, ActionRequest, TreeId};
    let mut model = magistr_cockpit::harness::Scenes::navigation(Size::new(122, 32))
        .with_surface(magistr_cockpit::cockpit::Surface::Native)
        .with_tab(magistr_cockpit::cockpit::StageTab::Turn);
    let mut frame = frame_of(&mut model);
    let draft = model.prompt().clone();
    let old_composer = frame.access().find(Widget::Composer).unwrap().id;
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Char('k')).with(magistr_cockpit::input::Modifiers::SUPER)),
    );
    frame.build(&mut model, &clock(), 1132, 800).unwrap();
    assert!(frame.access().find(Widget::Composer).is_none());
    assert_eq!(frame.focus_target(), Some(Widget::OverlayQuery.id()));
    let stale = ActionRequest {
        target_tree: TreeId::ROOT,
        target_node: old_composer,
        action: Action::SetValue,
        data: Some(ActionData::Value("wrong".into())),
    };
    assert!(frame.access_action(&model, stale).is_err());
    let query = frame.access().find(Widget::OverlayQuery).unwrap().id;
    let request = ActionRequest {
        target_tree: TreeId::ROOT,
        target_node: query,
        action: Action::SetValue,
        data: Some(ActionData::Value("Recorded work".into())),
    };
    for msg in frame.access_action(&model, request).unwrap() {
        update(&mut model, msg);
    }
    assert_eq!(model.palette().unwrap().query(), "Recorded work");
    assert_eq!(model.prompt(), &draft);
}

#[test]
fn accessible_resize_uses_shared_panel_limits_and_keeps_the_draft() {
    use accesskit::{Action, ActionRequest, TreeId};
    let mut model = Model::new(Size::new(122, 32))
        .with_surface(magistr_cockpit::cockpit::Surface::Native)
        .with_tab(magistr_cockpit::cockpit::StageTab::Turn);
    update(&mut model, Msg::Paste("keep".into()));
    let mut frame = frame_of(&mut model);
    let before = frame.access().find(Widget::PanelColumns).unwrap().bounds;
    let id = frame.access().find(Widget::PanelColumns).unwrap().id;
    for msg in frame
        .access_action(
            &model,
            ActionRequest {
                target_tree: TreeId::ROOT,
                target_node: id,
                action: Action::Increment,
                data: None,
            },
        )
        .unwrap()
    {
        update(&mut model, msg);
    }
    frame
        .build(&mut model, &clock(), Fixture::CAPTURE.0, Fixture::CAPTURE.1)
        .unwrap();
    assert_ne!(
        frame.access().find(Widget::PanelColumns).unwrap().bounds,
        before
    );
    assert_eq!(model.prompt().text(), "keep");
    assert!(model.active_panel().is_none());
}

#[test]
fn controls_advertise_only_actions_the_native_frame_can_execute() {
    use accesskit::Action;
    let (frame, _) = session();
    let tree = frame.access();
    let update = tree.update(None);
    let node = |widget| {
        &update
            .nodes
            .iter()
            .find(|(id, _)| *id == tree.find(widget).unwrap().id)
            .unwrap()
            .1
    };
    assert!(node(Widget::Composer).supports_action(Action::Focus));
    assert!(node(Widget::Composer).supports_action(Action::SetValue));
    assert!(node(Widget::TabRoadmap).supports_action(Action::Click));
    assert!(node(Widget::PanelColumns).supports_action(Action::Increment));
    assert!(!node(Widget::Stage).supports_action(Action::Click));
}

#[test]
fn composer_access_value_preserves_newlines_and_surrounding_spaces() {
    let mut model = Model::new(Size::new(122, 32));
    update(&mut model, Msg::Paste("  hello\nworld  ".into()));
    let frame = frame_of(&mut model);
    assert_eq!(
        frame
            .access()
            .find(Widget::Composer)
            .unwrap()
            .value
            .as_deref(),
        Some("  hello\nworld  ")
    );
}

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
    let tree = frame.access();
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

/// Published nodes retain the semantic target used by the hit registry.
#[test]
fn a_node_names_the_cockpits_widget_target() {
    let (frame, _) = session();
    let tree = frame.access();
    let composer = tree.find(Widget::Composer).expect("the composer");
    assert_eq!(
        composer.target,
        Some(Widget::Composer.id()),
        "the node resolves to the widget"
    );
}

/// Roles and names come from the cockpit's own vocabulary: the
/// composer is a text input, the divider a splitter, a crumb a link.
#[test]
fn the_roles_and_names_come_from_the_widget_ids() {
    let (frame, _) = session();
    let tree = frame.access();
    assert_eq!(
        tree.find(Widget::Composer).expect("composer").role,
        Role::MultilineTextInput
    );
    assert_eq!(
        tree.find(Widget::PanelColumns).expect("divider").role,
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
    let tree = frame.access();
    let composer = tree.find(Widget::Composer).expect("the composer");
    let value = composer.value.as_deref().expect("a value");
    assert!(value.contains("ship it"), "{value:?}");
}

/// AccessKit performs the platform's DPI conversion; publish painted pixels once.
#[test]
fn accessibility_bounds_match_painted_pixels_at_every_scale() {
    let (mut frame, mut model) = session();
    for scale in [1.0, 1.5, 2.0] {
        frame.rescale(scale).unwrap();
        frame
            .build(
                &mut model,
                &clock(),
                (1132.0 * scale) as u32,
                (800.0 * scale) as u32,
            )
            .unwrap();
        let [x, y, w, h] = frame.rect_of(crate::ComposerComposition::PILL).unwrap();
        let tree = frame.access();
        let (x0, y0, x1, y1) = tree.find(Widget::Composer).unwrap().bounds.unwrap();
        for (published, painted) in [(x0, x), (y0, y), (x1, x + w), (y1, y + h)] {
            assert!(
                (published - f64::from(painted)).abs() < 0.001,
                "scale {scale}: accessibility {published}, painted {painted}"
            );
        }
    }
}

/// The update AccessKit is handed is a *full* tree with a root and a
/// focus inside it, because the adapter's activation handler returns
/// none and the first update after that must carry everything.
#[test]
fn the_update_is_a_whole_tree_rooted_at_the_window() {
    let (frame, _) = session();
    let tree = frame.access();
    let update = tree.update(Some(Widget::Composer.id()));
    assert_eq!(update.nodes.len(), tree.nodes().len());
    let root = update.tree.as_ref().expect("a tree is declared");
    assert_eq!(root.root, AccessTree::WINDOW);
    assert_eq!(update.focus, tree.find(Widget::Composer).unwrap().id);
    // A focus the frame did not place falls back to the window.
    let update = tree.update(Some(Widget::Prompt.id()));
    assert_eq!(update.focus, AccessTree::WINDOW);
}

/// Before any frame was built there is nothing to describe, and the
/// tree says so rather than describing a screen nobody painted.
#[test]
fn an_unbuilt_frame_publishes_nothing() {
    let frame = Frame::new().expect("a grid");
    assert!(frame.access().nodes().is_empty());
}
