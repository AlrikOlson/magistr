use super::*;

#[test]
fn events_received_before_graphics_stay_in_records_without_conversation_rows() {
    let mut model = Model::new(crate::cockpit::Size::new(144, 50));
    model.turn.push_event("Decision 12 saved");
    model.turn.push_text(Voice::User, "Check the restart.\n");
    model.turn.push_event("Task started");
    model
        .turn
        .push_text(Voice::Agent, "The receipt is saved.\n");
    model.pixels = true;
    model.sync_flight_wrap();
    let rows = model.turn.conversation().rows();
    assert_eq!(rows[0].height, 0);
    assert_eq!(rows[2].height, 0);
    assert_eq!(rows[1].source, 1);
    assert_eq!(model.top_row(), 1);
    assert!(matches!(model.turn.rows()[2].kind, RowKind::Event));
    assert_eq!(rows[3].author, Some(Voice::Agent));
}

use crate::agent::{AgentEvent, ToolStatus, ToolVerb};
use crate::cockpit::{Msg, Size, update};
use crate::input::{Button, MouseEvent, MouseKind, Reply};
use crate::layout::{Basis, LayoutTree};
use crate::pixel::{CellSize, Typography};

fn model() -> Model {
    let mut model = Model::new(Size::new(144, 50)).with_tab(crate::cockpit::StageTab::Turn);
    model.pixels = true;
    model.sync_flight_wrap();
    model
}

#[test]
fn source_reveal_uses_measured_rows_and_preserves_the_draft() {
    let mut model = model();
    for i in 0..40 {
        model
            .turn
            .push_text(Voice::Agent, &format!("Saved result {i}.\n"));
    }
    model.prompt.insert("Keep this draft");
    model.sync_flight_wrap();
    let source = model.turn.lines()[8].row_start();
    assert!(model.reveal_conversation(source));
    assert_eq!(
        model.conversation_top() as usize,
        model.conversation_row_offset(source)
    );
    assert_eq!(model.top_row(), source);
    assert_eq!(model.prompt.text(), "Keep this draft");
    let before = model.conversation_top();
    assert!(!model.reveal_conversation(usize::MAX));
    assert_eq!(model.conversation_top(), before);
}

#[test]
fn speaker_groups_and_cards_keep_actual_source_targets() {
    let mut model = model();
    model.turn.push_text(Voice::User, "Keep retries safe.\n");
    model
        .turn
        .push_text(Voice::Agent, "I will save the retry schedule.\n");
    model.turn.push_edit(
        "change-one",
        "src/retry.rs",
        ToolStatus::Completed,
        "-old\n+new",
    );
    model.sync_flight_wrap();
    let geometry = model.turn.conversation();
    assert_eq!(
        geometry
            .rows()
            .iter()
            .filter_map(|r| r.author)
            .collect::<Vec<_>>(),
        [Voice::User, Voice::Agent]
    );
    let source = model.turn.lines().last().unwrap().row_start();
    let row = geometry.rows()[source];
    assert!(row.height > 60);
    let root = crate::cockpit::view(
        &model,
        &crate::cockpit::FrameClock::new(0, std::time::Duration::ZERO),
    );
    let mut face = Typography::default();
    let placed = LayoutTree::new()
        .compute_on(&root, 1440.0, 1000.0, Basis::new(10.0, 20.0), &mut face)
        .unwrap();
    let id = Widget::stage_row(source - model.top_row());
    let rect = placed
        .placed()
        .iter()
        .find(|p| p.id == Some(id))
        .unwrap()
        .px;
    let mut hits = crate::hit::HitRegistry::new();
    hits.register_layout_px(&placed, 0);
    let target = hits.hit_at(f64::from(rect.x + 20.0), f64::from(rect.y + 20.0));
    assert_eq!(target, Some(id));
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target,
        },
    );
    assert_eq!(model.selected(), Some(&Selected::Tool("change-one".into())));
    assert!(matches!(
        model.turn.lines().last().unwrap().kind(),
        LineKind::Edit { expanded: true, .. }
    ));
    let button_row = model
        .turn
        .rows()
        .iter()
        .position(|r| r.kind == RowKind::EditButtons)
        .unwrap();
    let index = button_row - model.top_row();
    let root = crate::cockpit::view(
        &model,
        &crate::cockpit::FrameClock::new(0, std::time::Duration::ZERO),
    );
    let placed = LayoutTree::new()
        .compute_on(&root, 1440.0, 1000.0, Basis::new(10.0, 20.0), &mut face)
        .unwrap();
    hits.clear();
    hits.register_layout_px(&placed, 0);
    for (id, expected) in [
        (
            Widget::edit_open(index),
            crate::cockpit::WorkbenchCmd::Open {
                path: "src/retry.rs".into(),
            },
        ),
        (
            Widget::edit_undo(index),
            crate::cockpit::WorkbenchCmd::Undo {
                id: "change-one".into(),
            },
        ),
    ] {
        let rect = placed
            .placed()
            .iter()
            .find(|p| p.id == Some(id))
            .unwrap()
            .px;
        let target = hits.hit_at(
            f64::from(rect.x + rect.width / 2.0),
            f64::from(rect.y + rect.height / 2.0),
        );
        assert_eq!(target, Some(id));
        let commands = update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target,
            },
        );
        assert_eq!(commands, vec![crate::cockpit::Cmd::Workbench(expected)]);
    }
}

#[test]
fn pinned_pixel_anchor_survives_streaming_resize_and_card_growth() {
    let mut model = model();
    for i in 0..40 {
        model.turn.push_text(
            Voice::Agent,
            &format!(
                "Message {i}: {}\n",
                "Keep the source anchor beside its words. ".repeat(3)
            ),
        );
    }
    model.sync_flight_wrap();
    let position = model.conversation_row_offset(20) + 7;
    let (total, height) = model.conversation_window();
    model.viewport.home(total, height, 0.0, false);
    model.viewport.hold(position, total, height);
    let anchor = model.stage_anchor().unwrap();
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("New tail content arrives.\n".into())),
    );
    assert_eq!(model.stage_anchor(), Some(anchor));
    update(
        &mut model,
        Msg::Reply(Reply::CellPixels {
            width: 13,
            height: 29,
        }),
    );
    assert_eq!(model.stage_anchor(), Some(anchor));
    assert_eq!(model.cell_size(), CellSize::new(13, 29));
    assert!(!model.viewport.is_following());
}

#[test]
fn pixel_wrap_preserves_blank_paragraphs_and_measures_digit_leading_prose() {
    use crate::turn::Turn;
    let mut turn = Turn::new(8);
    turn.push_text(
        Voice::Agent,
        "12 attempts are enough to finish this request.\n\nNext paragraph.\n",
    );
    turn.set_pixel_wrap(Some(180), CellSize::DEFAULT);
    assert!(turn.rows().iter().any(|r| r.text.is_empty()));
    let joined = turn
        .rows()
        .iter()
        .filter(|r| !r.text.is_empty())
        .map(|r| r.text.trim())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(joined.contains("12 attempts are enough"));
    assert!(turn.rows()[0].text.len() > 8);
}

#[test]
fn proportional_lists_keep_their_marker_and_measured_continuation_indent() {
    use crate::layout::TextRole;
    let mut turn = crate::turn::Turn::new(8);
    turn.push_text(
        Voice::Agent,
        "12. Keep the original receipt and save each retry before sending it.\n",
    );
    turn.set_pixel_wrap(Some(160), CellSize::DEFAULT);
    let face = Typography::default();
    let rows = turn.rows();
    assert!(rows.len() > 1);
    assert_eq!(rows[0].marker.as_deref(), Some("12. "));
    let indent = face.advance("12. ", TextRole::Prose).ceil() as u16;
    assert!(
        rows.iter()
            .all(|row| row.kind == RowKind::Bullet && row.indent_px == indent)
    );
    assert!(rows[1..].iter().all(|row| row.marker.is_none()));
    assert!(
        rows.iter().all(
            |row| face.advance(&row.text, TextRole::Prose) + f32::from(row.indent_px) <= 160.01
        )
    );
    let anchor = turn.anchor_at(1).unwrap();
    turn.set_pixel_wrap(Some(210), CellSize::DEFAULT);
    assert!(turn.row_of(anchor) < turn.len());
    turn.set_pixel_wrap(None, CellSize::DEFAULT);
    assert!(
        turn.rows()
            .iter()
            .all(|row| row.indent_px == 0 && row.marker.is_none())
    );
    assert!(turn.rows()[0].text.starts_with("12. "));
}

#[test]
fn terminal_output_arriving_after_completion_cannot_be_killed_as_running_work() {
    for late_output in [true, false] {
        let mut model = model();
        let output = AgentEvent::Terminal {
            terminal_id: "checks".into(),
            bytes: b"3 checks passed\r\n".to_vec(),
        };
        if !late_output {
            update(&mut model, Msg::Agent(output.clone()));
        }
        update(
            &mut model,
            Msg::Agent(AgentEvent::ToolCall {
                id: "run".into(),
                title: "Recovery checks".into(),
                verb: ToolVerb::Run,
                status: ToolStatus::Completed,
                diff: None,
                images: Vec::new(),
                record: Box::new(crate::agent::ToolRecord {
                    terminal: Some("checks".into()),
                    ..crate::agent::ToolRecord::default()
                }),
            }),
        );
        if late_output {
            update(&mut model, Msg::Agent(output));
        }
        assert!(model.term("checks").unwrap().is_finished());
        assert!(!model.term("checks").unwrap().is_killed());
        assert!(model.kill_target().is_none());
    }
}

#[test]
fn settled_following_starts_with_a_complete_source_row_at_large_metrics() {
    let mut model = model();
    update(
        &mut model,
        Msg::Reply(Reply::CellPixels {
            width: 12,
            height: 28,
        }),
    );
    for index in 0..30 {
        model
            .turn
            .push_text(Voice::Agent, &format!("Saved result {index}.\n"));
    }
    model.sync_flight_wrap();
    assert!(model.viewport.is_following());
    assert_eq!(
        model.conversation_top() as usize,
        model.conversation_row_offset(model.top_row())
    );
    assert!(model.visible_rows().last().unwrap().text.contains("29"));
}

#[test]
fn fences_and_lists_keep_bounded_aligned_rows_under_pixel_wrapping() {
    use crate::turn::Turn;
    let mut turn = Turn::new(40);
    turn.push_text(
        Voice::Agent,
        "# Result\n\n```rust\nlet retry = 3;\n```\n12. Keep the saved receipt\n",
    );
    turn.set_pixel_wrap(Some(350), CellSize::DEFAULT);
    assert_eq!(
        turn.rows()
            .iter()
            .filter(|r| r.kind == RowKind::Fence)
            .count(),
        2
    );
    assert!(
        turn.rows()
            .iter()
            .filter(|r| r.kind == RowKind::Fence)
            .all(|r| crate::render::width(&r.text) == 40)
    );
    assert!(
        turn.rows()
            .iter()
            .any(|r| r.kind == RowKind::Code && r.text == "let retry = 3;")
    );
    assert!(turn.rows().iter().any(|r| r.kind == RowKind::Bullet));
    assert!(
        turn.rows()
            .iter()
            .any(|r| r.kind == RowKind::Heading && r.text == "Result")
    );
}

#[test]
fn compact_harbor_conversation_keeps_the_question_visible() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../apps/website/fixtures/harbor.json"
    ))
    .unwrap();
    let data = &data["turn"];
    let mut model = Model::new(Size::new(128, 40));
    model.pixels = true;
    model.turn.push_text(
        Voice::User,
        &format!("{}\n", data["prompt"].as_str().unwrap()),
    );
    model.turn.push_text(
        Voice::Agent,
        &format!("{}\n", data["response"].as_str().unwrap()),
    );
    model.turn.push_edit(
        "harbor-edit",
        data["edit_path"].as_str().unwrap(),
        ToolStatus::Completed,
        "-renew_in_memory();\n+receipt_and_schedule.commit()?;",
    );
    model
        .turn
        .set_tool_title("harbor-edit", "Save the retry schedule");
    model.turn.push_tool(
        "harbor-test",
        "3 recovery checks passed",
        ToolVerb::Run,
        ToolStatus::Completed,
    );
    model.turn.push_text(
        Voice::Agent,
        &format!("{}\n", data["followup"].as_str().unwrap()),
    );
    model.sync_flight_wrap();
    assert!(model.flight_layout().conversation_height < 380.0);
    assert_eq!(
        model.top_row(),
        0,
        "the initial conversation includes its complete question"
    );
    assert_eq!(model.conversation_top(), 0.0);
    assert!(
        model
            .visible_rows()
            .iter()
            .any(|r| r.text.contains("worker restarts"))
    );
    assert!(
        model
            .visible_rows()
            .last()
            .unwrap()
            .text
            .contains("final receipt")
    );
}

#[test]
fn card_growth_preserves_following_and_latest_text_remains_visible() {
    let mut model = model();
    model.turn.push_tool(
        "check",
        "Run restart recovery checks",
        ToolVerb::Run,
        ToolStatus::InProgress,
    );
    for _ in 0..12 {
        model
            .turn
            .push_text(Voice::Agent, "Waiting for the current checks.\n");
    }
    model.sync_flight_wrap();
    assert!(model.viewport.is_following());
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("The latest result is ready.\n".into())),
    );
    assert!(
        model
            .visible_rows()
            .last()
            .unwrap()
            .text
            .contains("latest result")
    );
    let last = model.turn.conversation().rows().last().unwrap();
    assert!(
        last.top + last.height <= model.conversation_top() as usize + model.conversation_height()
    );
}
