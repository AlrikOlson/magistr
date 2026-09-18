//! Native roadmap states use the real renderer in both themes and display scales.
use magistr_cockpit::cockpit::{
    FrameClock, Model, Msg, RequestPhase, Size, StageTab, Surface, Widget, update,
};
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};
use magistr_gui2::{Frame, comp_roadmap::RoadmapComposition as R, fixture::Fixture};
fn scene(name: &str, size: Size) -> Model {
    match name {
        "empty" => Model::new(size)
            .with_surface(Surface::Native)
            .with_tab(StageTab::Roadmap),
        "waiting" => Fixture::roadmap_request(size, RequestPhase::Waiting),
        "queued" => Fixture::roadmap_request(size, RequestPhase::Queued),
        "running" => Fixture::roadmap_request(size, RequestPhase::Working),
        "failed" => Fixture::roadmap_request(size, RequestPhase::Failed),
        "stopped" => Fixture::roadmap_request(size, RequestPhase::Stopped),
        "complete" => Fixture::roadmap_request(size, RequestPhase::Complete),
        _ => Fixture::roadmap(size),
    }
}
#[test]
fn roadmap_state_matrix_has_stable_frames_and_usable_controls() {
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    for (theme, mode) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        for (layout, w, h, scale) in [
            ("wide", 1100, 760, 1.0),
            ("compact", 620, 640, 1.0),
            ("scaled", 1650, 1140, 1.5),
        ] {
            for name in [
                "empty", "plan", "waiting", "queued", "running", "failed", "stopped", "complete",
            ] {
                let mut frame = Frame::new().unwrap();
                frame.rescale(scale).unwrap();
                let mut model = scene(name, frame.metrics().grid(w, h))
                    .with_theme(Theme::forced(mode).with_motion(MotionMode::Reduced));
                let clock = FrameClock::new(0, std::time::Duration::ZERO);
                let image = frame.still(&mut model, &clock, w, h).unwrap();
                let access = frame.access();
                let ids: Vec<_> = access.nodes().iter().map(|n| n.id).collect();
                assert_eq!(
                    ids.len(),
                    ids.iter().collect::<std::collections::HashSet<_>>().len()
                );
                for widget in [
                    Widget::TabTurn,
                    Widget::RoadmapRequests,
                    Widget::RoadmapDetails,
                    Widget::Composer,
                ] {
                    let node = access.find(widget).expect("visible primary control");
                    let (x, y, right, bottom) = node.bounds.unwrap();
                    assert!(
                        x >= 0.0 && y >= 0.0 && right <= w as f64 + 1.0 && bottom <= h as f64 + 1.0,
                        "{name} {layout} {widget:?}: {node:?}"
                    );
                }
                assert!(frame.rect_of(R::BODY).is_some());
                let before = model.clone();
                let again = frame.still(&mut model, &clock, w, h).unwrap();
                assert_eq!(model, before, "layout is not a workflow event");
                assert_eq!(image.data(), again.data(), "reduced motion is settled");
                if layout != "scaled" {
                    image
                        .save_png(output.join(format!("roadmap-{name}-{layout}-{theme}.png")))
                        .unwrap();
                }
            }
        }
    }
}
#[test]
fn reviewed_roadmap_goldens() {
    let mut mismatches = Vec::new();
    let golden = magistr_cockpit::golden::Golden::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/roadmap"),
    );
    for (theme, mode) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        for (name, layout, w, h) in [
            ("plan", "wide", 1100, 760),
            ("complete", "wide", 1100, 760),
            ("failed", "compact", 620, 640),
        ] {
            let mut frame = Frame::new().unwrap();
            let mut model = scene(name, frame.metrics().grid(w, h))
                .with_theme(Theme::forced(mode).with_motion(MotionMode::Reduced));
            let image = frame
                .still(
                    &mut model,
                    &FrameClock::new(0, std::time::Duration::ZERO),
                    w,
                    h,
                )
                .unwrap();
            if let Err(error) = golden.check(&format!("{name}-{layout}-{theme}"), &image) {
                mismatches.push(error.to_string());
            }
        }
    }
    assert!(mismatches.is_empty(), "Review candidates: {mismatches:#?}");
}

#[test]
fn selected_blocker_and_proposal_are_visible_in_item_details() {
    for (id, text) in [
        ("delivery-blocked", "needs an access key"),
        ("delivery-later", "clear summary"),
    ] {
        let mut model = Fixture::roadmap(Size::new(120, 40));
        let at = model
            .roadmap()
            .rows()
            .iter()
            .position(|chunk| chunk.id == id)
            .unwrap();
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target: Some(Widget::roadmap_row(at)),
            },
        );
        let mut frame = Frame::new().unwrap();
        frame
            .still(
                &mut model,
                &FrameClock::new(0, std::time::Duration::ZERO),
                1100,
                900,
            )
            .unwrap();
        let says = frame
            .says()
            .into_iter()
            .map(|(s, _)| s)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(says.contains(text), "{says}");
        assert!(frame.access().find(Widget::RoadmapStart).is_none());
        if id == "delivery-blocked" {
            assert_eq!(
                model.roadmap().star_state(id),
                Some(magistr_cockpit::cockpit::StarState::Blocked)
            );
        }
    }
}
