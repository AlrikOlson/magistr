use super::*;
use crate::cockpit::{Msg, Size, Surface, update};
use crate::harness::fixtures;
use crate::input::{Button, MouseEvent, MouseKind};
use magistr_engine::NativeSnapshot;
use serde_json::json;

fn model(steps: Vec<serde_json::Value>) -> Model {
    Model::new(Size::new(144, 50))
        .with_surface(Surface::Terminal)
        .with_snapshot(&NativeSnapshot {
            steps: fixtures::steps_from(steps),
            ..NativeSnapshot::default()
        })
}

fn select(model: &mut Model, index: usize) {
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(Widget::trace_row(index)),
        },
    );
}

#[test]
fn empty_and_single_graphs_are_finite_and_keep_global_identity() {
    let empty = model(vec![]);
    assert!(FlightRecorder::step_indices(&empty, 900.0).is_empty());
    assert!(
        FlightRecorder::graph(&empty, 900.0)
            .points(900.0, 80.0)
            .is_empty()
    );
    let one = model(vec![json!({"step_number":812})]);
    assert_eq!(FlightRecorder::step_indices(&one, 900.0), [0]);
    assert_eq!(
        FlightRecorder::graph(&one, 900.0).points(900.0, 80.0),
        [(450.0, 40.0)]
    );
}

#[test]
fn every_sparse_decision_remains_reachable_in_a_large_trace() {
    let mut model = model((0..1000).map(|i| json!({"step_number":100+i*19})).collect());
    for index in [0, 1, 499, 998, 999] {
        select(&mut model, index);
        let indices = FlightRecorder::step_indices(&model, 350.0);
        assert!(indices.contains(&index));
        assert!(indices.len() < 10);
        let graph = FlightRecorder::graph(&model, 350.0);
        assert_eq!(graph.points(350.0, 80.0).len(), indices.len());
        for (x, y) in graph.points(350.0, 80.0) {
            assert!((0.0..350.0).contains(&x) && (0.0..80.0).contains(&y));
        }
    }
}

#[test]
fn many_interleaved_branches_never_collapse_into_one_lane() {
    let mut model = model(
        (0..50)
            .map(|i| {
                json!({
                    "step_number":i*3+10,
                    "branch_id":format!("b{}",i%8),
                    "branch_from":1
                })
            })
            .collect(),
    );
    select(&mut model, 25);
    let indices = FlightRecorder::step_indices(&model, 900.0);
    assert!(indices.contains(&25));
    assert!(visible_branches(&model, &indices).len() <= 3);
    let graph = FlightRecorder::graph(&model, 900.0);
    for (a, &index_a) in indices.iter().enumerate() {
        for (b, &index_b) in indices.iter().enumerate() {
            if model.trace().steps()[index_a].branch_id != model.trace().steps()[index_b].branch_id
            {
                assert_ne!(graph.lanes()[a], graph.lanes()[b]);
            }
        }
    }
}

#[test]
fn short_question_cards_drop_whole_lines_instead_of_clipping_text() {
    let model = model(vec![json!({
        "step_number": 7, "branch_id": "missing-name",
        "branch_name": "Should retries share a limit?",
        "outcome": "Keep separate limits."
    })]);
    let style = FlightStyle::for_model(&model);
    let questions = model.trace().investigations();
    let line = f32::from(model.cell_size().height);
    let text = |height| {
        fn visit(node: &Node, labels: &mut Vec<String>) {
            if let Some(text) = node.text_metrics() {
                labels.push(text.text().to_owned());
            }
            for child in node.child_nodes() {
                visit(child, labels);
            }
        }
        let card = question_card(&model, &style, &questions[0], Some(7), 400.0, height);
        let mut labels = Vec::new();
        visit(&card, &mut labels);
        labels.join("\n")
    };
    assert!(text(line * 3.0 + 36.0).contains("Keep separate limits."));
    let compact = text(line * 3.0 + 24.0);
    assert!(!compact.contains("Keep separate limits."));
    assert!(compact.contains("Still exploring"));
    let minimum = text(line + 24.0);
    assert!(minimum.contains("Should retries share a limit?"));
    assert!(!minimum.contains("Still exploring"));
}

#[test]
fn append_keeps_the_selected_global_decision_in_the_graph() {
    let mut model = model((0..30).map(|i| json!({"step_number":100+i*13})).collect());
    select(&mut model, 11);
    let selected = model.selected().cloned();
    update(
        &mut model,
        Msg::Engine(fixtures::event(
            "think",
            "step_appended",
            json!({
                "step":{"step_number":9999,"purpose":"A later conclusion"}
            }),
        )),
    );
    assert_eq!(model.selected(), selected.as_ref());
    assert!(FlightRecorder::step_indices(&model, 400.0).contains(&11));
}
