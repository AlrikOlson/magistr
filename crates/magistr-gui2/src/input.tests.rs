use super::*;

#[test]
fn native_text_payload_keeps_combined_scalars_and_ignores_composition_enter() {
    let input = Input::default();
    let result = input.text_key(
        &Key::Character("e".into()),
        Some("e\u{301}"),
        KeyKind::Press,
    );
    assert!(matches!(result, Some(Msg::Paste(text)) if text == "e\u{301}"));
}

#[test]
fn composed_unicode_and_paste_commit_once_without_submitting() {
    use magistr_cockpit::cockpit::{StageTab, update};
    use winit::event::Ime;
    let mut input = Input::default();
    let hits = HitRegistry::default();
    let mut model =
        magistr_cockpit::cockpit::Model::new(Size::new(80, 24)).with_tab(StageTab::Turn);
    for event in [Ime::Enabled, Ime::Preedit("ni".into(), Some((2, 2)))] {
        for msg in fold(&mut input, WindowEvent::Ime(event), &hits) {
            assert!(update(&mut model, msg).is_empty());
        }
    }
    assert!(
        input
            .keyed(&Key::Named(NamedKey::Enter), KeyKind::Press)
            .is_none()
    );
    assert!(model.prompt().is_empty());
    for event in [
        Ime::Preedit(String::new(), None),
        Ime::Commit("你好".into()),
    ] {
        for msg in fold(&mut input, WindowEvent::Ime(event), &hits) {
            assert!(update(&mut model, msg).is_empty());
        }
    }
    let msg = input
        .keyed(&Key::Character("e\u{301} 👩‍💻".into()), KeyKind::Press)
        .unwrap();
    assert!(update(&mut model, msg).is_empty());
    assert_eq!(model.prompt().text(), "你好e\u{301} 👩‍💻");
}

#[test]
fn modified_return_keeps_a_multiline_draft() {
    use magistr_cockpit::cockpit::{StageTab, update};
    let mut model =
        magistr_cockpit::cockpit::Model::new(Size::new(80, 24)).with_tab(StageTab::Turn);
    for mods in [Modifiers::SHIFT, Modifiers::ALT] {
        let input = Input {
            mods,
            ..Input::default()
        };
        assert!(
            update(
                &mut model,
                input
                    .keyed(&Key::Named(NamedKey::Enter), KeyKind::Press)
                    .unwrap()
            )
            .is_empty()
        );
    }
    assert_eq!(model.prompt().text(), "\n\n");
}

#[test]
fn navigation_journey_through_native_keys_preserves_the_work_and_draft() {
    use magistr_cockpit::cockpit::{Selected, StageTab, Surface, update};
    use magistr_cockpit::harness::Scenes;
    let mut model = Scenes::navigation(Size::new(120, 40)).with_surface(Surface::Native);
    let prompt = model.prompt().clone();
    let mut input = Input::default();
    for (query, tab) in [
        ("Retry delivery", StageTab::Roadmap),
        ("Roadmap", StageTab::Roadmap),
        ("Recorded work", StageTab::Session),
        ("Conversation", StageTab::Turn),
    ] {
        input.mods = Modifiers::SUPER;
        update(
            &mut model,
            input
                .keyed(&Key::Character("k".into()), KeyKind::Press)
                .unwrap(),
        );
        assert!(model.palette().is_some());
        input.mods = Modifiers::NONE;
        for c in query.chars() {
            update(
                &mut model,
                input
                    .keyed(&Key::Character(c.to_string().into()), KeyKind::Press)
                    .unwrap(),
            );
        }
        update(
            &mut model,
            input
                .keyed(&Key::Named(NamedKey::Enter), KeyKind::Press)
                .unwrap(),
        );
        assert!(model.palette().is_none());
        assert_eq!(model.tab(), tab);
        assert_eq!(model.shown_tab(), tab);
        assert_eq!(
            model.selected(),
            Some(&Selected::Chunk("retry-work".into()))
        );
        assert_eq!(model.prompt(), &prompt);
    }
    input.mods = Modifiers::CTRL;
    update(
        &mut model,
        input
            .keyed(&Key::Named(NamedKey::Tab), KeyKind::Press)
            .unwrap(),
    );
    assert_eq!(model.tab(), StageTab::Session);
    assert_eq!(model.prompt(), &prompt);
}

use magistr_cockpit::cockpit::{FrameClock, Widget};
use winit::dpi::PhysicalPosition;
use winit::event::DeviceId;

use crate::fixture::Fixture;
use crate::text::TextEngine;

fn metrics() -> CellMetrics {
    TextEngine::embedded()
        .metrics(CellMetrics::DEFAULT_PX)
        .expect("a grid")
}

fn device() -> DeviceId {
    DeviceId::dummy()
}

/// The frame the session's own build registered — its registry is the
/// element layout's placed boxes under the cockpit's widget ids.
fn cockpit() -> crate::Frame {
    let mut model = Fixture::mock("05", Size::new(80, 24)).expect("view 05");
    let mut frame = crate::Frame::new().expect("a grid");
    let cell = frame.metrics();
    let (w, h) = (
        (f32::from(model.size().cols) * cell.width()) as u32,
        (f32::from(model.size().rows) * cell.height()) as u32,
    );
    let _ = frame
        .build(
            &mut model,
            &FrameClock::new(0, std::time::Duration::ZERO),
            w,
            h,
        )
        .expect("builds");
    frame
}

fn fold(input: &mut Input, event: WindowEvent, hits: &HitRegistry) -> Vec<Msg> {
    let mut out = Vec::new();
    input.fold(&event, &metrics(), Size::new(80, 24), hits, &mut out);
    out
}

fn moved(x: f64, y: f64) -> WindowEvent {
    WindowEvent::CursorMoved {
        device_id: device(),
        position: PhysicalPosition::new(x, y),
    }
}

fn clicked(state: ElementState) -> WindowEvent {
    WindowEvent::MouseInput {
        device_id: device(),
        state,
        button: MouseButton::Left,
    }
}

/// The pointer's pixel lands on the widget layout put there — the whole
/// of "resolved through the hit registry", with the terminal's own
/// registry doing the resolving.
#[test]
fn a_click_carries_the_widget_the_terminal_would_have_carried() {
    let frame = cockpit();
    let cell = metrics();
    let mut input = Input::default();
    // The middle of the projects crumb, in the pixels the element
    // layout actually put it at (the window has no tab bar — the
    // altitude path is the navigation).
    let (tree, layout) = frame.placed().expect("a frame was built");
    let mut crumb = None;
    tree.each(&mut |index, element| {
        if element.id == Some(crate::comp_header::HeaderComposition::PROJECTS) {
            crumb = Some(layout.rect(index));
        }
    });
    let [cx, cy, cw, ch] = crumb.expect("the crumb is on screen");
    let x = f64::from(cx + cw / 2.0);
    let y = f64::from(cy + ch / 2.0);

    fold(&mut input, moved(x, y), frame.hits());
    let msgs = fold(&mut input, clicked(ElementState::Pressed), frame.hits());
    let Some(Msg::Mouse { event, target }) = msgs.first() else {
        panic!("no mouse message: {msgs:?}");
    };
    assert_eq!(event.kind, MouseKind::Down(Button::Left));
    assert_eq!(
        (event.col, event.row),
        (
            (x / f64::from(cell.width())) as u16,
            (y / f64::from(cell.height())) as u16
        )
    );
    assert_eq!(*target, Some(Widget::CrumbProjects.id()));
}

/// The pixel the pointer is at travels with the cell it resolved to,
/// so a drag on a window whose rows are not all one cell tall can be
/// exact where the uniform division is only near.
#[test]
fn a_report_carries_the_pointers_pixel_beside_its_cell() {
    let frame = cockpit();
    let hits = frame.hits();
    let cell = metrics();
    let mut input = Input::default();
    let x = f64::from(cell.width()) * 4.0 + 2.0;
    let y = f64::from(cell.height()) * 3.0 + 7.4;
    let msgs = fold(&mut input, moved(x, y), hits);
    let Some(Msg::Mouse { event, .. }) = msgs.first() else {
        panic!("no mouse message");
    };
    assert_eq!((event.col, event.row), (4, 3), "the uniform cell");
    assert_eq!(
        event.px,
        Some((x.round() as u32, y.round() as u32)),
        "and the pixel, rounded"
    );
}

/// winit reports a drag as a bare `CursorMoved`; a terminal reports
/// `Drag(button)`. Remembering the held button is what keeps the rail
/// divider and the tech-tree pan working — without it every drag would
/// arrive as a hover.
#[test]
fn a_move_with_a_button_held_is_a_drag_and_without_one_is_a_hover() {
    let frame = cockpit();
    let hits = frame.hits();
    let mut input = Input::default();
    let kind = |msgs: &[Msg]| match msgs.first() {
        Some(Msg::Mouse { event, .. }) => event.kind,
        other => panic!("no mouse message: {other:?}"),
    };
    assert_eq!(
        kind(&fold(&mut input, moved(40.0, 40.0), hits)),
        MouseKind::Move
    );
    fold(&mut input, clicked(ElementState::Pressed), hits);
    assert_eq!(
        kind(&fold(&mut input, moved(60.0, 40.0), hits)),
        MouseKind::Drag(Button::Left)
    );
    fold(&mut input, clicked(ElementState::Released), hits);
    assert_eq!(
        kind(&fold(&mut input, moved(80.0, 40.0), hits)),
        MouseKind::Move
    );
}

/// A pointer that leaves the window releases the button it was holding:
/// otherwise the next hover over the surface would arrive as a drag of a
/// button nobody is pressing.
#[test]
fn a_pointer_that_leaves_the_window_stops_dragging() {
    let frame = cockpit();
    let hits = frame.hits();
    let mut input = Input::default();
    fold(&mut input, clicked(ElementState::Pressed), hits);
    fold(
        &mut input,
        WindowEvent::CursorLeft {
            device_id: device(),
        },
        hits,
    );
    let msgs = fold(&mut input, moved(10.0, 10.0), hits);
    let Some(Msg::Mouse { event, .. }) = msgs.first() else {
        panic!("no mouse message");
    };
    assert_eq!(event.kind, MouseKind::Move);
}

/// A line delta is notches outright, and positive `y` is a scroll *up*
/// — winit measures away from the user.
#[test]
fn a_line_delta_is_notches_and_positive_is_upward() {
    let frame = cockpit();
    let hits = frame.hits();
    let mut input = Input::default();
    let msgs = fold(
        &mut input,
        WindowEvent::MouseWheel {
            device_id: device(),
            delta: MouseScrollDelta::LineDelta(0.0, 3.0),
            phase: winit::event::TouchPhase::Moved,
        },
        hits,
    );
    assert_eq!(msgs.len(), 3, "three notches, three reports");
    for msg in &msgs {
        let Msg::Mouse { event, .. } = msg else {
            panic!("not a mouse message");
        };
        assert_eq!(event.kind, MouseKind::ScrollUp);
    }
}

/// A trackpad's continuous delta accumulates: a gesture smaller than a
/// notch produces nothing and does not lose the pixels it moved, which
/// is what stops a slow drag from scrolling by nothing for ever.
#[test]
fn a_pixel_delta_accumulates_into_whole_notches() {
    let frame = cockpit();
    let hits = frame.hits();
    let mut input = Input::default();
    let notch = f64::from(metrics().height());
    let wheel = |dy: f64| WindowEvent::MouseWheel {
        device_id: device(),
        delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, dy)),
        phase: winit::event::TouchPhase::Moved,
    };
    assert!(fold(&mut input, wheel(notch * 0.4), hits).is_empty());
    assert!(fold(&mut input, wheel(notch * 0.4), hits).is_empty());
    // 1.2 notches accumulated: one report, and the remainder is kept.
    assert_eq!(fold(&mut input, wheel(notch * 0.4), hits).len(), 1);
}

/// The *logical* key, so a binding is the letter a layout produces
/// rather than the scancode under it. A key the cockpit has no code for
/// reaches `update` as nothing at all, the way a terminal drops a
/// sequence it cannot decode.
#[test]
fn a_letter_arrives_as_the_letter_and_an_unknown_key_as_nothing() {
    let input = Input::default();
    let msg = input
        .keyed(&Key::Character("n".into()), KeyKind::Press)
        .expect("a letter has a code");
    let Msg::Key(key) = msg else {
        panic!("not a key message");
    };
    assert_eq!(key.code, KeyCode::Char('n'));
    assert_eq!(key.kind, KeyKind::Press);

    assert!(
        input
            .keyed(&Key::Named(NamedKey::BrowserBack), KeyKind::Press)
            .is_none(),
        "a key with no cockpit code should reach update as nothing"
    );
}

/// Space is a named key in winit and a character to the cockpit — the
/// prompt's most-typed key, and the one a naive `Key::Character` match
/// silently drops.
#[test]
fn space_is_a_named_key_in_winit_and_a_character_to_the_cockpit() {
    let input = Input::default();
    let msg = input
        .keyed(&Key::Named(NamedKey::Space), KeyKind::Press)
        .expect("space has a code");
    let Msg::Key(key) = msg else {
        panic!("not a key message");
    };
    assert_eq!(key.code, KeyCode::Char(' '));
}

/// Modifiers arrive as their own winit event and are then carried on
/// every report, the way a terminal encodes them into each one.
#[test]
fn modifiers_are_remembered_and_carried_on_every_report() {
    let frame = cockpit();
    let hits = frame.hits();
    let mut input = Input::default();
    let mut state = ModifiersState::empty();
    state.set(ModifiersState::CONTROL, true);
    fold(
        &mut input,
        WindowEvent::ModifiersChanged(state.into()),
        hits,
    );
    let msgs = fold(&mut input, moved(10.0, 10.0), hits);
    let Some(Msg::Mouse { event, .. }) = msgs.first() else {
        panic!("no mouse message");
    };
    assert!(event.mods.contains(Modifiers::CTRL));
    let Some(Msg::Key(key)) = input.keyed(&Key::Character("c".into()), KeyKind::Press) else {
        panic!("not a key message");
    };
    assert!(key.mods.contains(Modifiers::CTRL));
}
