use magistr_engine::{NativeEvent, NativeSnapshot};
use serde_json::json;

use super::*;
use crate::cockpit::trace::TRACE_GRAPH_ROWS;
use crate::harness::fixtures;

fn frame(family: &str, kind: &str, payload: serde_json::Value) -> NativeEvent {
    fixtures::frame(family, kind, payload)
}

fn step(number: u32) -> NativeEvent {
    frame(
        "think",
        "step_appended",
        json!({"step": {"step_number": number, "purpose": "analysis"}}),
    )
}

fn branch_step(number: u32, branch: &str, from: u32) -> NativeEvent {
    frame(
        "think",
        "step_appended",
        json!({"step": {
            "step_number": number,
            "branch_id": branch,
            "branch_name": format!("{branch} name"),
            "branch_from": from,
        }}),
    )
}

/// A branch's status is readable by id: `active` until the engine says
/// otherwise, the frame's word after — the datum the window's
/// `branches · 1 · kept` row reads without parsing a formatted row.
#[test]
fn a_branch_status_is_active_until_the_frame_says_otherwise() {
    let mut trace = Trace::default();
    trace.apply(&step(1));
    trace.apply(&branch_step(2, "alt", 1));
    assert_eq!(trace.branch_status("alt"), "active");
    assert_eq!(trace.branch_status("never"), "active");
    trace.apply(&frame(
        "think",
        "branch_status_changed",
        json!({"branch_id": "alt", "status": "merged"}),
    ));
    assert_eq!(trace.branch_status("alt"), "merged");
}

#[test]
fn steps_land_in_arrival_order_and_a_renumbered_step_replaces() {
    let mut trace = Trace::default();
    trace.apply(&step(1));
    trace.apply(&step(2));
    trace.apply(&frame(
        "think",
        "step_appended",
        json!({"step": {"step_number": 1, "purpose": "correction"}}),
    ));
    assert_eq!(trace.steps().len(), 2);
    assert_eq!(trace.steps()[0].purpose, "correction");
    assert_eq!(trace.steps()[1].number, 2);
}

#[test]
fn a_revising_step_marks_its_target_and_so_does_step_revised() {
    let mut trace = Trace::default();
    trace.apply(&step(1));
    trace.apply(&frame(
        "think",
        "step_appended",
        json!({"step": {"step_number": 2, "revises_step": 1}}),
    ));
    assert_eq!(trace.step(1).expect("step 1").revised_by, Some(2));
    trace.apply(&step(3));
    trace.apply(&frame(
        "think",
        "step_revised",
        json!({"revised_step": 3, "by_step": 9}),
    ));
    assert_eq!(trace.step(3).expect("step 3").revised_by, Some(9));
}

#[test]
fn toggle_expands_one_step_at_a_time_and_expand_step_navigates() {
    let mut trace = Trace::default();
    trace.apply(&step(1));
    trace.apply(&step(2));
    trace.toggle(0);
    assert!(trace.is_expanded(1));
    trace.toggle(1);
    assert!(trace.is_expanded(2), "the newer expansion replaces");
    assert!(!trace.is_expanded(1));
    trace.toggle(1);
    assert!(!trace.is_expanded(2), "a second click folds");
    assert!(trace.expand_step(1), "a link expands its target");
    assert!(trace.is_expanded(1));
    assert!(!trace.expand_step(99), "an unknown target is refused");
}

#[test]
fn pin_changed_frames_and_port_answers_flip_the_flag() {
    let mut trace = Trace::default();
    trace.apply(&step(4));
    trace.apply(&frame(
        "think",
        "pin_changed",
        json!({"step_number": 4, "pinned": true}),
    ));
    assert!(trace.step(4).expect("step").pinned);
    trace.set_pinned(4, false);
    assert!(!trace.step(4).expect("step").pinned);
}

#[test]
fn lanes_links_and_branch_rows_describe_the_fork() {
    let mut trace = Trace::default();
    trace.apply(&step(1));
    trace.apply(&step(2));
    trace.apply(&branch_step(3, "b-alt", 2));
    trace.apply(&branch_step(4, "b-alt", 2));
    trace.apply(&step(5));
    assert_eq!(trace.lanes(), vec![(1, 0), (2, 0), (3, 1), (4, 1), (5, 0)]);
    assert_eq!(trace.links(), vec![(1, 2)], "fork edge from step 2's index");
    assert!(trace.has_branches());
    trace.apply(&frame(
        "think",
        "branch_status_changed",
        json!({"branch_id": "b-alt", "status": "merged"}),
    ));
    assert_eq!(
        trace.branch_rows(),
        vec![
            "main · 3 steps".to_owned(),
            "└─ b-alt name (merged) from 2 · 2 steps".to_owned(),
        ]
    );
}

#[test]
fn ship_frames_join_the_timeline_and_cleared_empties_each_half() {
    let mut trace = Trace::default();
    trace.apply(&frame(
        "ship",
        "action_recorded",
        json!({"task_id": "t", "action": {"type": "code", "description": "did"}}),
    ));
    trace.apply(&frame(
        "ship",
        "check_recorded",
        json!({"check": {"name": "test", "passed": true, "exit_code": 0}}),
    ));
    assert_eq!(trace.moments().len(), 2);
    trace.apply(&step(1));
    trace.apply(&frame("ship", "cleared", json!({})));
    assert!(trace.moments().is_empty());
    assert_eq!(trace.steps().len(), 1, "the think half is untouched");
    trace.apply(&frame("think", "cleared", json!({})));
    assert!(trace.steps().is_empty());
    assert!(trace.is_empty());
}

#[test]
fn the_checkpoint_viewer_opens_and_closes() {
    let mut trace = Trace::default();
    assert!(!trace.close_viewer(), "nothing to close");
    trace.open_viewer(vec!["open hypotheses: 1".to_owned()]);
    assert_eq!(
        trace.viewer().expect("open"),
        ["open hypotheses: 1".to_owned()]
    );
    assert!(!trace.is_empty(), "a report counts as content");
    assert!(trace.close_viewer());
    assert_eq!(trace.viewer(), None);
}

#[test]
fn foreign_events_are_ignored() {
    let mut trace = Trace::default();
    trace.apply(&frame(
        "roadmap",
        "chunk_changed",
        json!({"chunk": {"id": "a"}}),
    ));
    trace.apply(&frame(
        "think",
        "estimate_revised",
        json!({"old": 3, "new": 9}),
    ));
    trace.apply(&frame("ship", "task_started", json!({"task_id": "t1"})));
    assert!(trace.is_empty());
}

#[test]
fn a_seed_replaces_both_halves_and_keeps_what_only_the_feed_knows() {
    let mut trace = Trace::default();
    // A feed-only run: two steps, a branch status, one moment, one open.
    trace.apply(&step(1));
    trace.apply(&branch_step(2, "alt", 1));
    trace.apply(&frame(
        "think",
        "branch_status_changed",
        json!({"branch_id": "alt", "status": "abandoned"}),
    ));
    trace.apply(&frame(
        "ship",
        "action_recorded",
        json!({"task_id": "t1", "action": {"type": "code", "description": "first"}}),
    ));
    trace.toggle(0);
    assert!(trace.is_expanded(1));

    // The store is the superset: the same two steps plus a third, and
    // the same moment — a re-seed must not double it.
    trace.seed(&NativeSnapshot {
        steps: vec![
            fixtures::step_from(json!({"step_number": 1, "purpose": "analysis"})),
            fixtures::step_from(json!({
                "step_number": 2,
                "branch_id": "alt",
                "branch_name": "alt name",
                "branch_from": 1,
            })),
            fixtures::step_from(json!({"step_number": 3, "purpose": "analysis"})),
        ],
        tasks: vec![fixtures::task_from(json!({
            "id": "t1",
            "actions": [{"type": "code", "description": "first"}],
        }))],
        ..NativeSnapshot::default()
    });

    assert_eq!(
        trace.steps().iter().map(|s| s.number).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        trace.moments().len(),
        1,
        "the moment is replaced, not appended"
    );
    assert!(
        trace.is_expanded(1),
        "the expansion is the feed's, not the store's"
    );
    assert!(
        trace
            .branch_rows()
            .iter()
            .any(|row| row.contains("abandoned")),
        "the branch status survives a re-seed: {:?}",
        trace.branch_rows()
    );
}

#[test]
fn the_body_counts_every_row_the_tab_paints() {
    let mut trace = Trace::default();
    assert_eq!(trace.pinned_rows(false), 1, "the heading");
    assert_eq!(trace.body_rows(80), 1, "the empty note");

    trace.apply(&step(1));
    trace.apply(&step(2));
    assert_eq!((trace.pinned_rows(false), trace.body_rows(80)), (1, 2));
    assert_eq!(
        trace.pinned_rows(true),
        1 + TRACE_GRAPH_ROWS,
        "with pixels the graph block sits under the heading"
    );

    // Selection and disclosure leave the compact list unchanged.
    trace.toggle(0);
    let card = trace.card_rows(80, false).len();
    assert!(
        card >= 5,
        "blank, title, meta, buttons, blank at least: {card}"
    );
    assert_eq!(trace.body_rows(80), 2);
    // Every field whole is never fewer rows than the brief cut.
    trace.toggle_detail();
    assert!(trace.body_rows(80) == 2);
    // A narrower stage wraps to more rows, never fewer.
    assert!(trace.body_rows(40) >= trace.body_rows(80));
    trace.toggle_detail();

    // A branch pins the lane list above the scroll on a plain terminal.
    trace.apply(&branch_step(3, "alt", 1));
    assert_eq!(trace.pinned_rows(false), 1 + trace.branch_rows().len());

    // The recorded work adds its heading and one row per moment.
    let before = trace.body_rows(80);
    trace.apply(&frame(
        "ship",
        "check_recorded",
        json!({"task_id": "t1", "check": {"name": "lint", "passed": true}}),
    ));
    assert_eq!(trace.body_rows(80), before + 2);

    // The checkpoint pane takes the body whole.
    trace.open_viewer(vec!["{".to_owned(), "}".to_owned()]);
    assert_eq!((trace.pinned_rows(false), trace.body_rows(80)), (3, 2));
}

#[test]
fn the_selection_walks_the_feed_and_the_focus_lights_relations() {
    let mut trace = Trace::default();
    assert!(!trace.select_step(true), "nothing to select");
    trace.apply(&step(1));
    trace.apply(&step(2));
    trace.apply(&branch_step(3, "alt", 1));
    assert_eq!(trace.selected_index(), None);
    assert!(trace.select_step(false), "up from nothing takes the newest");
    assert_eq!(trace.selected_index(), Some(2));
    assert!(trace.select_step(false));
    assert_eq!(trace.selected_index(), Some(1));
    assert!(trace.select_step(true));
    assert_eq!(trace.selected_index(), Some(2));
    assert!(!trace.select_step(true), "the newest is the end");
    trace.deselect();
    assert!(
        trace.select_step(true),
        "down from nothing takes the newest too"
    );
    assert_eq!(trace.selected(), trace.step(3));
    let focus = trace.focus().expect("a selection has a focus");
    assert_eq!(focus.selected, 2);
    assert!(focus.is_lit(0), "the fork point is an ancestor");
    assert!(!focus.is_lit(1), "step 2 is unrelated to the branch");
    assert!(trace.deselect());
    assert!(!trace.deselect());
    assert_eq!(trace.focus(), None);
}

#[test]
fn a_merged_branch_rejoins_the_main_line_and_the_heading_counts() {
    let mut trace = Trace::default();
    trace.apply(&step(1));
    trace.apply(&branch_step(2, "alt", 1));
    trace.apply(&branch_step(3, "alt", 1));
    trace.apply(&step(4));
    assert_eq!(trace.merges(), vec![], "an active branch has not rejoined");
    trace.apply(&frame(
        "think",
        "branch_status_changed",
        json!({"branch_id": "alt", "status": "merged"}),
    ));
    assert_eq!(
        trace.merges(),
        vec![(2, 3)],
        "its last step links to the next main step"
    );
    assert_eq!(trace.heading_line(), "4 steps · 1 branch");
    trace.set_pinned(4, true);
    assert_eq!(trace.heading_line(), "4 steps · 1 branch · 1 pinned");
    assert_eq!(trace.pins(), vec![false, false, false, true]);
    assert_eq!(Trace::default().heading_line(), "0 steps");
}
