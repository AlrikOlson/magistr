use super::*;
use crate::cockpit::{Msg, Selected, Size, StageTab, update};
use crate::harness::fixtures;
use crate::hit::HitRegistry;
use crate::input::{Button, MouseEvent, MouseKind, Reply};
use crate::layout::{Basis, LayoutTree};
use serde_json::json;

fn model(count: usize, done: bool) -> Model {
    let mut model = Model::new(Size::new(144, 50));
    model.pixels = true;
    let chunks = (0..count)
        .map(|index| {
            fixtures::chunk_from(json!({
                "id": format!("internal-{index}"),
                "name": format!("Recovery task {}", index + 1),
                "title": "Keep every receipt safe",
                "status": if done { "done" } else { "pending" },
                "priority": index + 1,
                "description": "Save the receipt before retrying",
                "deps": if index == 0 { vec![] } else { vec![format!("internal-{}", index / 2)] },
                "acceptance": ["The saved receipt survives a restart"],
                "cross_refs": []
            }))
        })
        .collect::<Vec<_>>();
    model.roadmap.seed(&chunks, &[]);
    model.show(StageTab::Roadmap);
    model
}

#[test]
fn physical_points_labels_and_hits_share_fractional_placement_after_resize() {
    for (width, height, cell) in [(901.5, 280.75, (10, 20)), (702.25, 310.5, (13, 29))] {
        let mut model = model(3, false);
        update(
            &mut model,
            Msg::Reply(Reply::CellPixels {
                width: cell.0,
                height: cell.1,
            }),
        );
        let root = FlightRoadmapGraph::view(&model, width, height);
        let mut typography = Typography::default();
        typography.set_cell(model.cell_size());
        let placed = LayoutTree::new()
            .compute_on(
                &root,
                width,
                height,
                Basis::new(f32::from(cell.0), f32::from(cell.1)),
                &mut typography,
            )
            .unwrap();
        let rect = placed
            .placed()
            .iter()
            .find(|p| p.id == Some(Widget::RoadmapGraph.id()))
            .unwrap()
            .px;
        model.set_roadmap_graph_px(Some(rect));
        let nodes = FlightRoadmapGraph::nodes(&model, rect.width, rect.height);
        let mut hits = HitRegistry::new();
        hits.register_layout_px(&placed, 0);
        for (index, point) in nodes.iter().enumerate() {
            let x = f64::from(rect.x + point.x);
            let y = f64::from(rect.y + point.y);
            assert_eq!(hits.hit_at(x, y), Some(Widget::sky_star(index)));
            assert!(model.roadmap.click_at(point.x, point.y, model.now()));
            assert_eq!(
                model.roadmap.selected().unwrap().id,
                format!("internal-{index}")
            );
        }
        let point = &nodes[1];
        let event = MouseEvent::new(MouseKind::Down(Button::Left), 0, 0)
            .at_px((rect.x + point.x) as u32, (rect.y + point.y) as u32);
        let target = hits.hit_at(
            f64::from(event.px.unwrap().0),
            f64::from(event.px.unwrap().1),
        );
        update(&mut model, Msg::Mouse { event, target });
        assert_eq!(
            model.selected(),
            Some(&Selected::Chunk("internal-1".into()))
        );
        assert_eq!(model.shown_tab(), StageTab::Roadmap);
    }
}

#[test]
fn full_graph_keeps_all_sixty_six_items_and_focuses_old_completed_work() {
    let mut model = model(66, true);
    model.set_roadmap_graph_px(Some(PxRect::new(20.5, 110.25, 900.5, 208.5)));
    assert_eq!(model.roadmap.dag_nodes().len(), 66);
    assert!(
        model
            .roadmap
            .dag_nodes()
            .iter()
            .all(|node| node.x >= 0.0 && node.x <= 900.5 && node.y >= 0.0 && node.y <= 208.5),
        "100% frames the whole plan"
    );
    for index in [0, 32, 65] {
        let id = format!("internal-{index}");
        model.roadmap.select(&id, 0.0, false);
        let point = &model.roadmap.dag_nodes()[index];
        assert!(point.x >= 0.0 && point.x < 900.5);
        assert!(point.y >= 0.0 && point.y < 208.5);
        assert!(point.selected);
    }
    let nodes = model.roadmap.dag_nodes();
    for path in model.roadmap.dag_paths_at(f32::INFINITY) {
        assert_eq!(path.points[0], (nodes[path.from].x, nodes[path.from].y));
        assert_eq!(
            *path.points.last().unwrap(),
            (nodes[path.to].x, nodes[path.to].y)
        );
    }
}

#[test]
fn dense_overview_names_never_cover_another_work_items_point() {
    let mut model = model(66, false);
    let width = 900.0;
    let height = 280.0;
    let root = FlightRoadmapGraph::view(&model, width, height);
    let mut typography = Typography::default();
    let placed = LayoutTree::new()
        .compute_on(
            &root,
            width,
            height,
            Basis::new(10.0, 20.0),
            &mut typography,
        )
        .unwrap();
    let graph = placed.px(Widget::RoadmapGraph.id()).unwrap();
    model.set_roadmap_graph_px(Some(graph));
    let mut hits = HitRegistry::new();
    hits.register_layout_px(&placed, 0);
    for (index, point) in FlightRoadmapGraph::nodes(&model, graph.width, graph.height)
        .iter()
        .enumerate()
    {
        assert_eq!(
            hits.hit_at(f64::from(graph.x + point.x), f64::from(graph.y + point.y)),
            Some(Widget::sky_star(index)),
            "point {index}"
        );
    }
}

#[test]
fn panned_nodes_never_capture_header_list_or_composer_clicks() {
    let mut model = model(66, false);
    let clock = crate::cockpit::FrameClock::new(0, std::time::Duration::ZERO);
    let mut typography = Typography::default();
    let mut tree = LayoutTree::new();
    let first = tree
        .compute_on(
            &crate::cockpit::view(&model, &clock),
            1440.0,
            1000.0,
            Basis::new(10.0, 20.0),
            &mut typography,
        )
        .unwrap();
    let graph = first.px(Widget::RoadmapGraph.id()).unwrap();
    model.set_roadmap_graph_px(Some(graph));
    model
        .roadmap
        .zoom_at(4.0, graph.width / 2.0, graph.height / 2.0);
    model.roadmap.pan_by(-100.0, -30.0);
    let placed = tree
        .compute_on(
            &crate::cockpit::view(&model, &clock),
            1440.0,
            1000.0,
            Basis::new(10.0, 20.0),
            &mut typography,
        )
        .unwrap();
    let graph = placed.px(Widget::RoadmapGraph.id()).unwrap();
    for target in placed
        .placed()
        .iter()
        .filter(|p| p.id.and_then(Widget::sky_star_index).is_some())
    {
        assert!(target.px.x >= graph.x && target.px.y >= graph.y);
        assert!(target.px.x + target.px.width <= graph.x + graph.width + 0.01);
        assert!(target.px.y + target.px.height <= graph.y + graph.height + 0.01);
    }
    let mut hits = HitRegistry::new();
    hits.register_layout_px(&placed, 0);
    for x in (0..1440).step_by(4) {
        for y in [graph.y - 1.0, graph.y + graph.height + 1.0, 930.0] {
            assert!(
                hits.hit_at(f64::from(x), f64::from(y))
                    .and_then(Widget::sky_star_index)
                    .is_none()
            );
        }
    }
}

#[test]
fn reset_view_works_through_its_drawn_button_with_an_unsent_draft() {
    let mut model = model(66, false);
    model.prompt.insert("Please check the recovery order.");
    model.set_roadmap_graph_px(Some(PxRect::new(1.0, 35.0, 898.0, 208.0)));
    model.roadmap.zoom_at(3.0, 400.0, 100.0);
    model.roadmap.pan_by(-40.0, -10.0);
    let selected = model.selected().cloned();
    let root = FlightRoadmapGraph::view(&model, 900.0, 280.0);
    let mut typography = Typography::default();
    let placed = LayoutTree::new()
        .compute_on(&root, 900.0, 280.0, Basis::new(10.0, 20.0), &mut typography)
        .unwrap();
    let reset = placed.px(Widget::RoadmapReset.id()).unwrap();
    let x = reset.x + reset.width / 2.0;
    let y = reset.y + reset.height / 2.0;
    let mut hits = HitRegistry::new();
    hits.register_layout_px(&placed, 0);
    let target = hits.hit_at(f64::from(x), f64::from(y));
    assert_eq!(target, Some(Widget::RoadmapReset.id()));
    let commands = update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0).at_px(x as u32, y as u32),
            target,
        },
    );
    assert!(
        commands.iter().all(|command| matches!(
            command,
            crate::cockpit::Cmd::Animate(_) | crate::cockpit::Cmd::Still
        )),
        "reset only requests camera repaint: {commands:?}"
    );
    assert_eq!(model.roadmap.zoom(), 1.0);
    assert_eq!(model.roadmap.pan(), (0.0, 0.0));
    assert_eq!(model.prompt.text(), "Please check the recovery order.");
    assert_eq!(model.selected(), selected.as_ref());
}

#[test]
fn physical_wheel_and_drag_preserve_exact_camera_motion() {
    let mut model = model(66, false);
    update(
        &mut model,
        Msg::Reply(Reply::CellPixels {
            width: 13,
            height: 29,
        }),
    );
    model.set_roadmap_graph_px(Some(PxRect::new(20.5, 110.25, 700.5, 200.5)));
    let event = |kind, x, y| MouseEvent::new(kind, 0, 0).at_px(x, y);
    update(
        &mut model,
        Msg::Mouse {
            event: event(MouseKind::ScrollUp, 350, 180),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert!(model.roadmap.zoom() > 1.0);
    let before = model.roadmap.pan();
    update(
        &mut model,
        Msg::Mouse {
            event: event(MouseKind::Down(Button::Left), 350, 180),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    update(
        &mut model,
        Msg::Mouse {
            event: event(MouseKind::Drag(Button::Left), 327, 180),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert!((model.roadmap.pan().0 - before.0 - 23.0).abs() < 0.01);
    update(
        &mut model,
        Msg::Mouse {
            event: event(MouseKind::Up(Button::Left), 327, 180),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert!(model.roadmap_drag.is_none());
    let pan = model.roadmap.pan();
    model.set_roadmap_graph_px(Some(PxRect::new(20.5, 110.25, 700.5, 200.5)));
    assert_eq!(
        model.roadmap.pan(),
        pan,
        "unchanged frames never reset the camera"
    );
}
