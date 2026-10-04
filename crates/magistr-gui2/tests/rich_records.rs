//! Real native reading, targeting and source anchoring in both themes.
use magistr_cockpit::cockpit::{
    FrameClock, Model, Msg, Pane, Size, StageTab, Surface, Widget, update,
};
use magistr_cockpit::harness::{Scenes, fixtures};
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};
use magistr_gui2::{Frame, comp_workspace::WorkspaceComposition as W};
use std::time::Duration;

fn press(model: &mut Model, target: magistr_cockpit::widget_id::WidgetId) {
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(target),
        },
    );
}
#[test]
fn rich_record_native_frames_have_copyable_text_and_stable_layout() {
    let mut errors = Vec::new();
    let golden = magistr_cockpit::golden::Golden::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/records"),
    );
    for (theme, mode) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        for (name, w, h) in [("wide", 1100, 900), ("narrow", 620, 740)] {
            let mut frame = Frame::new().unwrap();
            let mut model = Scenes::rich_record(frame.metrics().grid(w, h))
                .with_surface(Surface::Native)
                .with_theme(Theme::forced(mode).with_motion(MotionMode::Reduced));
            press(&mut model, Widget::RoadmapDetails.id());
            let clock = FrameClock::new(0, Duration::ZERO);
            let image = frame.still(&mut model, &clock, w, h).unwrap();
            let stable = model.clone();
            let again = frame.still(&mut model, &clock, w, h).unwrap();
            assert_eq!(image.data(), again.data());
            assert_eq!(model, stable);
            let [x, y, width, height] = frame.rect_of(W::id(Widget::detail_copy())).unwrap();
            assert_eq!(
                frame
                    .hits()
                    .hit_at((x + width / 2.0) as f64, (y + height / 2.0) as f64),
                Some(Widget::detail_copy())
            );
            let commands = update(
                &mut model,
                Msg::Mouse {
                    event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                    target: Some(Widget::detail_copy()),
                },
            );
            assert!(commands.iter().any(|c| matches!(c, magistr_cockpit::cockpit::Cmd::Clipboard(s) if s.contains("    let receipt = saved_result;") && s.contains("2. Preserve reading position"))));
            assert_eq!(model.prompt().text(), "Keep this draft");
            if let Err(error) = golden.check(&format!("record-{name}-{theme}"), &image) {
                errors.push(error.to_string());
            }
        }
    }
    assert!(errors.is_empty(), "{errors:#?}");
}
#[test]
fn native_resize_keeps_the_source_within_a_wrapped_paragraph() {
    use magistr_cockpit::engine::EngineEvent;
    let mut frame = Frame::new().unwrap();
    let mut model = Scenes::rich_record(Size::new(120, 40)).with_surface(Surface::Native);
    let changed = fixtures::frame(
        "roadmap",
        "chunk_changed",
        serde_json::json!({"chunk":{
            "id":"reading","title":"Save delivery results","status":"in_progress","description":(0..250).map(|n|format!("word{n} ")).collect::<String>()
        }}),
    );
    update(&mut model, Msg::Engine(EngineEvent::local(changed)));
    press(&mut model, Widget::RoadmapDetails.id());
    let clock = FrameClock::new(0, Duration::ZERO);
    frame.still(&mut model, &clock, 1100, 760).unwrap();
    let header = frame
        .rect_of(W::id(Widget::InspectorTitle.id()))
        .expect("placed record title");
    press(&mut model, Widget::InspectorFields.id());
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::PageDown)));
    frame.still(&mut model, &clock, 1100, 760).unwrap();
    assert!(model.inspector_offset() > 0);
    assert_eq!(
        frame.rect_of(W::id(Widget::InspectorTitle.id())),
        Some(header),
        "reading scroll keeps the header fixed"
    );
    assert!(
        frame
            .says()
            .iter()
            .any(|(s, _)| s == "Save delivery results"),
        "the title remains visible while reading"
    );
    let before = frame
        .says()
        .into_iter()
        .map(|(s, _)| s)
        .filter(|s| s.starts_with("word"))
        .collect::<Vec<_>>();
    frame.still(&mut model, &clock, 800, 760).unwrap();
    let after = frame
        .says()
        .into_iter()
        .map(|(s, _)| s)
        .filter(|s| s.starts_with("word"))
        .collect::<Vec<_>>();
    assert!(
        before
            .iter()
            .take(10)
            .any(|word| after.iter().take(25).any(|other| other == word)),
        "{before:?}\n{after:?}"
    );
    let stable = model.inspector_offset();
    frame.still(&mut model, &clock, 800, 760).unwrap();
    assert_eq!(model.inspector_offset(), stable);
    assert_eq!(model.pane(), Pane::Inspector);
    assert_eq!(model.tab(), StageTab::Roadmap);
}

#[test]
fn selected_document_input_and_paint_stay_within_the_native_bound() {
    use std::time::Instant;
    let mut frame = Frame::new().unwrap();
    let mut model = Scenes::rich_record(Size::new(120, 40)).with_surface(Surface::Native);
    press(&mut model, Widget::RoadmapDetails.id());
    press(&mut model, Widget::InspectorFields.id());
    let clock = FrameClock::new(0, Duration::ZERO);
    for _ in 0..4 {
        frame.build(&mut model, &clock, 1100, 900).unwrap();
    }
    let mut samples = Vec::new();
    for at in 0..60 {
        let start = Instant::now();
        update(
            &mut model,
            Msg::Key(KeyEvent::new(if at % 2 == 0 {
                KeyCode::Down
            } else {
                KeyCode::Up
            })),
        );
        frame.build(&mut model, &clock, 1100, 900).unwrap();
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    let worst = *samples.last().unwrap();
    let bound = Duration::from_millis(if cfg!(debug_assertions) { 60 } else { 20 });
    println!(
        "selected document input+paint: median {:?}, worst {worst:?}, bound {bound:?}",
        samples[samples.len() / 2]
    );
    assert!(worst <= bound, "{worst:?} exceeds {bound:?}");
    assert_eq!(model.pane(), Pane::Inspector);
    assert_eq!(model.prompt().text(), "Keep this draft");
}
