use super::*;
use crate::cockpit::{Msg, Selected, Size, Surface, update};
use crate::harness::fixtures;
use crate::hit::HitRegistry;
use crate::input::{Button, MouseEvent, MouseKind};
use crate::layout::{Basis, LayoutTree};
use magistr_workflow_contracts::NativeSnapshot;
use serde_json::json;

fn model(wide: bool) -> Model {
    let mut chunks = vec![
        fixtures::chunk_from(json!({"id":"foundation","name":"Storage","status":"done"})),
        fixtures::chunk_from(
            json!({"id":"limits","name":"Retry limits","status":"in_progress","deps":["foundation"]}),
        ),
        fixtures::chunk_from(
            json!({"id":"timing","name":"Retry timing","status":"pending","deps":["limits"]}),
        ),
    ];
    if wide {
        chunks.extend((0..40).map(|n| fixtures::chunk_from(json!({
            "id":format!("case-{n}"),"name":format!("Restart case {n}"),"status":"pending","deps":["limits"]
        }))));
    }
    Model::new(Size::new(144, 50))
        .with_surface(Surface::Terminal)
        .with_snapshot(&NativeSnapshot {
            chunks,
            ..NativeSnapshot::default()
        })
}

#[test]
fn every_drawn_point_resolves_to_its_work_after_resize_and_font_changes() {
    for (width, cell_width, cell_height) in [(440.0, 10, 20), (340.0, 12, 28), (220.0, 8, 16)] {
        let mut model = model(true);
        update(
            &mut model,
            Msg::CellSize(Some(crate::pixel::CellSize {
                width: cell_width,
                height: cell_height,
            })),
        );
        let graph_height = FlightRoadmap::graph_height(&model, 900.0);
        let mut typeface = Typography::default();
        typeface.set_cell(model.cell_size());
        let resolved = LayoutTree::new()
            .compute_on(
                &FlightRoadmap::view(&model, width, 900.0),
                width,
                900.0,
                Basis::new(f32::from(cell_width), f32::from(cell_height)),
                &mut typeface,
            )
            .expect("pixel layout");
        let graph = resolved
            .placed()
            .iter()
            .find(|placed| placed.id == Some(Widget::RailGraph.id()))
            .expect("graph")
            .px;
        let mut hits = HitRegistry::new();
        hits.register_layout_px(&resolved, 0);
        for (index, node) in FlightRoadmap::nodes(&model, graph.width, graph_height) {
            let (x, y) = (graph.x + node.x, graph.y + node.y);
            let target = hits.hit_at(f64::from(x), f64::from(y));
            assert_eq!(
                target,
                Some(Widget::sky_star(index)),
                "point {index} at {width}px / {cell_height}px"
            );
            let id = model
                .roadmap()
                .graph_chunk(index)
                .expect("drawn work")
                .id
                .clone();
            update(
                &mut model,
                Msg::Mouse {
                    event: MouseEvent::new(
                        MouseKind::Down(Button::Left),
                        (x / f32::from(cell_width)) as u16,
                        (y / f32::from(cell_height)) as u16,
                    )
                    .at_px(x.round() as u32, y.round() as u32),
                    target,
                },
            );
            assert_eq!(model.selected(), Some(&Selected::Chunk(id)));
        }
    }
}

#[test]
fn a_crowded_neighborhood_retains_the_active_item_and_real_edges() {
    let model = model(true);
    let (nodes, edges) = model.roadmap().mini_pixels(400.0, 60.0);
    assert!(nodes.len() < 10);
    assert!(nodes.iter().any(|(_, node)| node.here));
    for (from, to) in edges {
        let a = model.roadmap().graph_chunk(nodes[from].0).expect("source");
        let b = model.roadmap().graph_chunk(nodes[to].0).expect("target");
        assert!(b.deps.contains(&a.id));
    }
    for (a, (_, first)) in nodes.iter().enumerate() {
        for (_, second) in &nodes[a + 1..] {
            assert!((first.x - second.x).abs() > 12.0 || (first.y - second.y).abs() > 12.0);
        }
    }
}

#[test]
fn empty_and_single_item_maps_stay_inside_their_pixel_bounds() {
    let empty = Model::new(Size::new(100, 30));
    assert!(FlightRoadmap::nodes(&empty, 300.0, 60.0).is_empty());
    let one = Model::new(Size::new(100, 30)).with_snapshot(&NativeSnapshot {
        chunks: vec![fixtures::chunk_from(
            json!({"id":"start","name":"First task","status":"pending"}),
        )],
        ..NativeSnapshot::default()
    });
    let nodes = FlightRoadmap::nodes(&one, 301.5, 59.5);
    assert_eq!(nodes.len(), 1);
    assert_eq!((nodes[0].1.x, nodes[0].1.y), (150.75, 29.75));
}

fn panel_layout(model: &Model, width: f32, height: f32) -> crate::layout::Resolved {
    let cell = model.cell_size();
    let mut typeface = Typography::default();
    typeface.set_cell(cell);
    LayoutTree::new()
        .compute_on(
            &FlightRoadmap::view(model, width, height),
            width,
            height,
            Basis::new(f32::from(cell.width), f32::from(cell.height)),
            &mut typeface,
        )
        .expect("panel layout")
}

#[test]
fn small_panels_prioritize_complete_work_cards_and_scroll_without_moving_the_conversation() {
    for (width, height, cw, ch) in [(380.0, 284.0, 10, 20), (540.0, 480.0, 16, 33)] {
        let mut model = crate::harness::Scenes::roadmap_overview(Size::new(144, 50))
            .with_surface(Surface::Terminal);
        update(
            &mut model,
            Msg::Mode(crate::capability::Mode::Graphics {
                pointer_shapes: true,
            }),
        );
        update(
            &mut model,
            Msg::CellSize(Some(crate::pixel::CellSize {
                width: cw,
                height: ch,
            })),
        );
        update(&mut model, Msg::Paste("Keep this draft".into()));
        let top = model.top_row();
        let resolved = panel_layout(&model, width, height);
        assert!(
            !resolved
                .placed()
                .iter()
                .any(|p| p.id == Some(Widget::RailGraph.id())),
            "cards take priority"
        );
        let (index, chunk) = model.roadmap().overview_items()[0];
        let id = chunk.id.clone();
        let card = resolved
            .placed()
            .iter()
            .find(|p| p.id == Some(Widget::sky_star(index)))
            .unwrap()
            .px;
        assert!(
            card.y + card.height < height,
            "the first card is fully visible"
        );
        let mut hits = HitRegistry::new();
        hits.register_layout_px(&resolved, 0);
        let target = hits.hit_at(f64::from(card.x + 5.0), f64::from(card.y + 5.0));
        assert_eq!(target, Some(Widget::sky_star(index)));
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target,
            },
        );
        assert_eq!(model.selected(), Some(&Selected::Chunk(id)));
        assert_eq!(model.tab(), crate::cockpit::StageTab::Turn);
        assert_eq!(model.pane(), crate::cockpit::Pane::Inspector);
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::ScrollDown, 0, 0),
                target,
            },
        );
        assert_eq!(model.roadmap_overview_top(), 1);
        for _ in 0..50 {
            update(
                &mut model,
                Msg::Mouse {
                    event: MouseEvent::new(MouseKind::ScrollDown, 0, 0),
                    target: Some(Widget::RoadmapOverview.id()),
                },
            );
        }
        let items = model.roadmap().overview_items();
        assert_eq!(model.roadmap_overview_top(), items.len() - 1);
        let last = Widget::sky_star(items.last().unwrap().0);
        let resolved = panel_layout(&model, width, height);
        let card = resolved
            .placed()
            .iter()
            .find(|p| p.id == Some(last))
            .unwrap()
            .px;
        assert!(card.y + card.height < height, "the last item is reachable");
        assert_eq!(model.top_row(), top);
        assert_eq!(model.prompt().text(), "Keep this draft");
        update(
            &mut model,
            Msg::Engine(crate::engine::EngineEvent::Seed(NativeSnapshot {
                chunks: vec![fixtures::chunk_from(
                    json!({"id":"remaining","name":"Remaining work","status":"pending"}),
                )],
                ..NativeSnapshot::default()
            })),
        );
        assert_eq!(
            model.roadmap_overview_top(),
            0,
            "a shorter snapshot clamps the local scroll"
        );
    }
}

#[test]
fn the_panel_loop_button_opens_track_selection_without_starting_work() {
    let mut model = crate::harness::Scenes::roadmap_overview(Size::new(144, 50))
        .with_surface(Surface::Terminal);
    let resolved = panel_layout(&model, 380.0, 284.0);
    let button = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(Widget::LaunchLoop.id()))
        .unwrap()
        .px;
    let mut hits = HitRegistry::new();
    hits.register_layout_px(&resolved, 0);
    let target = hits.hit_at(
        f64::from(button.x + button.width * 0.5),
        f64::from(button.y + button.height * 0.5),
    );
    assert_eq!(target, Some(Widget::LaunchLoop.id()));
    assert!(
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target
            }
        )
        .is_empty()
    );
    assert_eq!(model.palette().unwrap().title(), "Choose a track");
    assert_eq!(model.launch(), None);
}

#[test]
fn a_stopped_loop_restores_track_selection_on_the_panel() {
    use crate::cockpit::LoopEvent;
    let mut model = crate::harness::Scenes::roadmap_overview(Size::new(144, 50))
        .with_surface(Surface::Terminal);
    update(
        &mut model,
        Msg::Mode(crate::capability::Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 1,
            max: 4,
        }),
    );
    let resolved = panel_layout(&model, 380.0, 284.0);
    assert!(
        !resolved
            .placed()
            .iter()
            .any(|p| p.id == Some(Widget::LaunchLoop.id()))
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Failed("The selected work is blocked.".into())),
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Ended("Work stopped.".into())),
    );
    let resolved = panel_layout(&model, 380.0, 284.0);
    let button = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(Widget::LaunchLoop.id()))
        .expect("start restored")
        .px;
    let mut hits = HitRegistry::new();
    hits.register_layout_px(&resolved, 0);
    let target = hits.hit_at(
        f64::from(button.x + button.width * 0.5),
        f64::from(button.y + button.height * 0.5),
    );
    assert_eq!(target, Some(Widget::LaunchLoop.id()));
    assert!(
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target,
            }
        )
        .is_empty()
    );
    assert_eq!(model.palette().unwrap().title(), "Choose a track");
    assert_eq!(model.launch(), None);
}
