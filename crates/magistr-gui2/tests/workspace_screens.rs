//! Native frame coverage for empty, running, tool, permission, stale and completed states.
use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::{FrameClock, Model, Msg, Size, StageTab, Surface, Widget, update};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::harness::{Scenes, fixtures};
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};
use magistr_gui2::{
    Frame, comp_composer::ComposerComposition, comp_workspace::WorkspaceComposition as W,
};
use magistr_workflow_contracts::FeedStatus;
use serde_json::json;

fn press(model: &mut Model, target: magistr_cockpit::widget_id::WidgetId) {
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(target),
        },
    );
}
fn scene(name: &str, size: Size) -> Model {
    let mut model = match name {
        "empty" => Model::new(size).with_tab(StageTab::Turn),
        "stream" => Scenes::streaming(size),
        "tools" => Scenes::permission_turn(size),
        "permission" => Scenes::permission(size),
        _ => Scenes::navigation(size),
    }
    .with_surface(Surface::Native);
    if matches!(name, "stale" | "complete") {
        update(
            &mut model,
            Msg::Engine(EngineEvent::local(fixtures::frame(
                "ship",
                "check_recorded",
                json!({"task_id":"verify-retry","check":{"name":"Recovery after restart","passed":true,
            "verified":true,"timestamp":"2026-09-17T12:00:00Z"}}),
            ))),
        );
        press(&mut model, Widget::session_row(1));
        press(&mut model, Widget::TabTurn.id());
        if name == "stale" {
            update(
                &mut model,
                Msg::Engine(EngineEvent::Feed(FeedStatus::Disconnected)),
            );
        } else {
            update(
                &mut model,
                Msg::Agent(AgentEvent::TurnEnded(
                    magistr_application::sessions::TurnEnd::EndTurn,
                )),
            );
        }
    }
    if name == "stream" {
        update(
            &mut model,
            Msg::Engine(EngineEvent::local(fixtures::frame(
                "think",
                "step_appended",
                json!({"step":{"step_number":17,"purpose":"Keep delivery limits separate",
                "outcome":"Save each customer's remaining attempts.",
                "rationale":"One busy customer should not delay another.",
                "next_action":"Check recovery after restart."}}),
            ))),
        );
        press(&mut model, Widget::ReasoningToggle.id());
        press(&mut model, Widget::LatestDecision.id());
    }
    Scenes::settle(&mut model);
    model
}

#[test]
fn workspace_states_render_in_both_themes_compact_wide_and_scaled() {
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    for (theme, mode) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        for (layout, w, h, scale) in [
            ("wide", 1100, 760, 1.0),
            ("compact", 620, 640, 1.0),
            ("scaled", 1650, 1140, 1.5),
        ] {
            for name in [
                "empty",
                "stream",
                "tools",
                "permission",
                "stale",
                "complete",
            ] {
                let mut frame = Frame::new().unwrap();
                frame.rescale(scale).unwrap();
                let mut model = scene(name, frame.metrics().grid(w, h))
                    .with_theme(Theme::forced(mode).with_motion(MotionMode::Reduced));
                let clock = FrameClock::new(0, std::time::Duration::ZERO);
                let image = frame.still(&mut model, &clock, w, h).unwrap();
                let composer = frame.rect_of(ComposerComposition::PILL).unwrap();
                assert!(composer[1] + composer[3] <= h as f32);
                assert!(composer[2] > w as f32 * 0.85);
                assert!(frame.rect_of(W::REASONING).is_some());
                assert!(frame.rect_of(W::NEARBY).is_some() == (layout != "compact"));
                let ids: Vec<_> = frame.access(scale).nodes().iter().map(|n| n.id).collect();
                let unique: std::collections::HashSet<_> = ids.iter().collect();
                assert_eq!(
                    ids.len(),
                    unique.len(),
                    "duplicate accessible identities in {name}"
                );
                let stable = model.clone();
                let again = frame.still(&mut model, &clock, w, h).unwrap();
                assert_eq!(image.data(), again.data());
                assert_eq!(model, stable);
                if layout != "scaled" && matches!(name, "stream" | "stale") {
                    magistr_cockpit::golden::Golden::new(
                        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("tests/golden/workspace"),
                    )
                    .assert_matches(&format!("{name}-{layout}-{theme}"), &image);
                }
                if layout != "scaled" {
                    image
                        .save_png(output.join(format!("workspace-{name}-{layout}-{theme}.png")))
                        .unwrap();
                }
            }
        }
    }
}
