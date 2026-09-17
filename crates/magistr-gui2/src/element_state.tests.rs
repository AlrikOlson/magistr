use magistr_cockpit::input::{KeyEvent, KeyKind, MouseEvent};
use qs_gpu::color::Srgba;

use super::*;
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

/// Two buttons side by side on a 200x50 canvas: ids 1 and 2, both
/// focusable, the second one optionally disabled.
fn fixture(second_disabled: bool) -> (Element, ElementLayout) {
    let mut second = Element::surface()
        .width(80.0)
        .height(30.0)
        .id(ElementId(2))
        .focusable();
    if second_disabled {
        second = second.disabled();
    }
    let tree = Element::row()
        .align_start()
        .gap(20.0)
        .child(
            Element::surface()
                .width(80.0)
                .height(30.0)
                .id(ElementId(1))
                .focusable()
                .child(Element::text(label(), white(), "run")),
        )
        .child(second);
    let mut text = TextEngine::embedded();
    let layout = ElementLayout::compute(&tree, 200.0, 50.0, &mut text).expect("layout");
    (tree, layout)
}

fn mouse(kind: MouseKind, x: u32, y: u32) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(kind, 0, 0).at_px(x, y),
        target: None,
    }
}

fn tab(mods: Modifiers) -> Msg {
    Msg::Key(KeyEvent::new(KeyCode::Tab).with(mods).kind(KeyKind::Press))
}

#[test]
fn the_pointer_hovers_the_element_under_its_pixel_and_leaves_it() {
    let (tree, layout) = fixture(false);
    let mut state = ElementState::default();
    assert!(state.fold(&mouse(MouseKind::Move, 10, 10), &tree, &layout));
    assert_eq!(state.hover(), Some(ElementId(1)));
    assert!(state.fold(&mouse(MouseKind::Move, 110, 10), &tree, &layout));
    assert_eq!(state.hover(), Some(ElementId(2)));
    assert!(state.fold(&mouse(MouseKind::Move, 10, 45), &tree, &layout));
    assert_eq!(state.hover(), None, "below both buttons is nobody's");
    assert!(
        !state.fold(&mouse(MouseKind::Move, 11, 45), &tree, &layout),
        "an unchanged fold reports no repaint"
    );
}

/// Three card heads under one another: ↑/↓ walk focus between them
/// and stop at the ends; with focus on a plain button (or nowhere) the
/// arrows are not the state's to spend.
#[test]
fn the_arrows_walk_the_walkable_run_and_stop_at_its_ends() {
    let head = |n: u64| {
        Element::surface()
            .width(120.0)
            .height(20.0)
            .id(ElementId(n))
            .walkable()
    };
    let tree = Element::surface()
        .align_start()
        .child(
            Element::surface()
                .width(80.0)
                .height(20.0)
                .id(ElementId(9))
                .focusable(),
        )
        .child(head(1))
        .child(head(2))
        .child(head(3).disabled());
    let mut text = TextEngine::embedded();
    let layout = ElementLayout::compute(&tree, 200.0, 100.0, &mut text).expect("layout");
    let arrow = |code: KeyCode| Msg::Key(KeyEvent::new(code).kind(KeyKind::Press));
    let mut state = ElementState::default();
    assert!(!state.walks(&tree), "no focus, no walk");
    assert!(!state.fold(&arrow(KeyCode::Down), &tree, &layout));
    assert_eq!(state.focus(), None, "the arrows never enter the run");
    // Click the first head: focus lands on it.
    assert!(state.fold(
        &mouse(MouseKind::Down(Button::Left), 10, 30),
        &tree,
        &layout
    ));
    assert_eq!(state.focus(), Some(ElementId(1)));
    assert!(state.walks(&tree));
    assert!(state.fold(&arrow(KeyCode::Down), &tree, &layout));
    assert_eq!(state.focus(), Some(ElementId(2)));
    assert!(
        !state.fold(&arrow(KeyCode::Down), &tree, &layout),
        "the disabled head is not on the run: the end holds"
    );
    assert_eq!(state.focus(), Some(ElementId(2)));
    assert!(state.fold(&arrow(KeyCode::Up), &tree, &layout));
    assert!(
        !state.fold(&arrow(KeyCode::Up), &tree, &layout),
        "the start holds"
    );
    assert_eq!(state.focus(), Some(ElementId(1)));
    // Tab onto the plain button: the arrows are the cockpit's again.
    assert!(state.fold(
        &mouse(MouseKind::Down(Button::Left), 10, 10),
        &tree,
        &layout
    ));
    assert_eq!(state.focus(), Some(ElementId(9)));
    assert!(!state.walks(&tree));
    assert!(!state.fold(&arrow(KeyCode::Down), &tree, &layout));
    assert_eq!(state.focus(), Some(ElementId(9)));
}

#[test]
fn a_press_holds_the_element_and_the_release_lets_go() {
    let (tree, layout) = fixture(false);
    let mut state = ElementState::default();
    assert!(state.fold(
        &mouse(MouseKind::Down(Button::Left), 10, 10),
        &tree,
        &layout
    ));
    assert_eq!(state.pressed(), Some(ElementId(1)));
    assert_eq!(
        state.focus(),
        Some(ElementId(1)),
        "a click focuses what it hits"
    );
    assert!(state.fold(&mouse(MouseKind::Up(Button::Left), 10, 10), &tree, &layout));
    assert_eq!(state.pressed(), None);
    assert_eq!(
        state.focus(),
        Some(ElementId(1)),
        "focus outlives the press"
    );
}

#[test]
fn a_click_on_nothing_clears_focus() {
    let (tree, layout) = fixture(false);
    let mut state = ElementState::default();
    state.fold(
        &mouse(MouseKind::Down(Button::Left), 10, 10),
        &tree,
        &layout,
    );
    state.fold(&mouse(MouseKind::Up(Button::Left), 10, 10), &tree, &layout);
    state.fold(
        &mouse(MouseKind::Down(Button::Left), 10, 45),
        &tree,
        &layout,
    );
    assert_eq!(state.focus(), None);
}

#[test]
fn tab_walks_the_ring_and_shift_tab_walks_it_backwards() {
    let (tree, layout) = fixture(false);
    let mut state = ElementState::default();
    assert!(state.fold(&tab(Modifiers::NONE), &tree, &layout));
    assert_eq!(state.focus(), Some(ElementId(1)));
    assert!(state.fold(&tab(Modifiers::NONE), &tree, &layout));
    assert_eq!(state.focus(), Some(ElementId(2)));
    assert!(state.fold(&tab(Modifiers::NONE), &tree, &layout));
    assert_eq!(state.focus(), Some(ElementId(1)), "the ring wraps");
    assert!(state.fold(&tab(Modifiers::SHIFT), &tree, &layout));
    assert_eq!(state.focus(), Some(ElementId(2)), "and walks back");
}

#[test]
fn shift_tab_enters_the_ring_at_its_far_end() {
    let (tree, layout) = fixture(false);
    let mut state = ElementState::default();
    assert!(state.fold(&tab(Modifiers::SHIFT), &tree, &layout));
    assert_eq!(state.focus(), Some(ElementId(2)));
}

#[test]
fn a_disabled_element_answers_no_pointer_and_takes_no_focus() {
    let (tree, layout) = fixture(true);
    let mut state = ElementState::default();
    state.fold(&mouse(MouseKind::Move, 110, 10), &tree, &layout);
    assert_eq!(state.hover(), None);
    state.fold(&tab(Modifiers::NONE), &tree, &layout);
    state.fold(&tab(Modifiers::NONE), &tree, &layout);
    assert_eq!(state.focus(), Some(ElementId(1)), "tab never reaches it");
}

#[test]
fn a_drag_keeps_hover_current_and_other_buttons_are_ignored() {
    let (tree, layout) = fixture(false);
    let mut state = ElementState::default();
    state.fold(
        &mouse(MouseKind::Down(Button::Left), 10, 10),
        &tree,
        &layout,
    );
    assert!(state.fold(
        &mouse(MouseKind::Drag(Button::Left), 110, 10),
        &tree,
        &layout
    ));
    assert_eq!(state.hover(), Some(ElementId(2)));
    assert_eq!(
        state.pressed(),
        Some(ElementId(1)),
        "the press stays where it began"
    );
    assert!(
        !state.fold(
            &mouse(MouseKind::Down(Button::Right), 110, 10),
            &tree,
            &layout
        ),
        "a right press is not the substrate's verb"
    );
}
