use super::*;
use crate::cockpit::{Msg, Selected, Size, Surface, update};
use crate::harness::fixtures;
use crate::hit::HitRegistry;
use crate::input::{Button, MouseEvent, MouseKind};
use crate::layout::{Basis, LayoutTree};
use magistr_engine::NativeSnapshot;
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
        let graph_height = FlightRoadmap::graph_height(&model, 148.0);
        let mut typeface = Typography::default();
        typeface.set_cell(model.cell_size());
        let resolved = LayoutTree::new()
            .compute_on(
                &FlightRoadmap::view(&model, width, 148.0),
                width,
                148.0,
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
