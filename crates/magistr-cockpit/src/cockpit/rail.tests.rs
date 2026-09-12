use magistr_engine::{NativeEvent, NativeSnapshot, RoadmapProgress};
use serde_json::json;

use super::*;
use crate::harness::fixtures;

fn frame(family: &str, kind: &str, payload: serde_json::Value) -> NativeEvent {
    fixtures::frame(family, kind, payload)
}

/// A snapshot with `done` of `total` chunks and the chunk `chunk` in
/// progress, when one is named.
fn progress(chunk: Option<&str>, done: u32, total: u32) -> NativeSnapshot {
    NativeSnapshot {
        progress: RoadmapProgress {
            total_tasks: total,
            checked_tasks: done,
            ..RoadmapProgress::default()
        },
        chunks: chunk
            .map(|id| fixtures::chunk_from(json!({"id": id, "status": "in_progress"})))
            .into_iter()
            .collect(),
        ..NativeSnapshot::default()
    }
}

#[test]
fn seeds_plan_position_and_signals_keeping_geometry() {
    let mut rail = Rail::default();
    rail.toggle();
    let seed = NativeSnapshot {
        objective: Some(fixtures::objective("goal")),
        signals: vec![
            fixtures::signal_from(json!({"id": "s1", "status": "new"})),
            fixtures::signal_from(json!({"id": "s2", "status": "researched"})),
            fixtures::signal_from(json!({"id": "s3", "status": "promoted"})),
            fixtures::signal_from(json!({"id": "s4", "status": "dismissed"})),
        ],
        ..progress(Some("r1"), 3, 8)
    };
    rail.seed(&seed);
    assert_eq!(rail.ship().objective(), Some("goal"));
    assert_eq!(rail.chunk(), Some("r1"));
    assert_eq!(rail.progress(), (3, 8));
    assert_eq!(rail.signals_pending(), 2);
    assert!(rail.is_collapsed(), "seeding leaves the geometry alone");
}

#[test]
fn ship_frames_reach_the_plan() {
    let mut rail = Rail::default();
    rail.apply(&frame(
        "ship",
        "objective_set",
        json!({"objective": {"description": "goal"}}),
    ));
    assert_eq!(rail.ship().objective(), Some("goal"));
}

#[test]
fn a_chunk_turning_in_progress_becomes_you_are_here() {
    let mut rail = Rail::default();
    rail.apply(&frame(
        "roadmap",
        "chunk_changed",
        json!({"chunk": {"id": "r1", "status": "in_progress"}}),
    ));
    assert_eq!(rail.chunk(), Some("r1"));
    rail.apply(&frame(
        "roadmap",
        "chunk_changed",
        json!({"chunk": {"id": "r1", "status": "blocked"}}),
    ));
    assert_eq!(
        rail.chunk(),
        None,
        "leaving in_progress clears the position"
    );
}

#[test]
fn a_completion_advances_done_and_clears_the_position() {
    let mut rail = Rail::default();
    rail.seed(&progress(Some("r1"), 3, 8));
    rail.apply(&frame(
        "roadmap",
        "chunk_completed",
        json!({"chunk": {"id": "r1"}}),
    ));
    assert_eq!(rail.progress(), (4, 8));
    assert_eq!(rail.chunk(), None);
}

#[test]
fn an_added_chunk_grows_the_total() {
    let mut rail = Rail::default();
    rail.apply(&frame(
        "roadmap",
        "chunk_added",
        json!({"chunk": {"id": "x"}}),
    ));
    assert_eq!(rail.progress(), (0, 1));
}

#[test]
fn signals_count_up_on_capture_and_down_on_promotion_or_dismissal() {
    let mut rail = Rail::default();
    rail.apply(&frame(
        "signal",
        "signal_captured",
        json!({"signal": {"id": "s1"}}),
    ));
    rail.apply(&frame(
        "signal",
        "signal_captured",
        json!({"signal": {"id": "s2"}}),
    ));
    assert_eq!(rail.signals_pending(), 2);
    rail.apply(&frame(
        "signal",
        "signal_changed",
        json!({"signal": {"id": "s1", "status": "promoted"}}),
    ));
    assert_eq!(rail.signals_pending(), 1);
    rail.apply(&frame(
        "signal",
        "signal_changed",
        json!({"signal": {"id": "s2", "status": "researched"}}),
    ));
    assert_eq!(
        rail.signals_pending(),
        1,
        "a non-terminal change keeps it pending"
    );
    rail.apply(&frame(
        "signal",
        "signal_dismissed",
        json!({"signal": {"id": "s2", "status": "dismissed"}}),
    ));
    assert_eq!(rail.signals_pending(), 0);
    rail.apply(&frame(
        "signal",
        "signal_dismissed",
        json!({"signal": {"id": "s3", "status": "dismissed"}}),
    ));
    assert_eq!(rail.signals_pending(), 0, "the count never goes negative");
}

#[test]
fn toggling_collapses_to_the_toggle_strip_and_back() {
    let mut rail = Rail::default();
    let expanded = rail.cols(120);
    assert_eq!(expanded, rail.width() + 1, "content plus the divider");
    rail.toggle();
    assert!(rail.is_collapsed());
    assert!(rail.cols(120) < expanded);
    rail.toggle();
    assert_eq!(rail.cols(120), expanded);
}

#[test]
fn a_narrow_screen_caps_the_rail_at_half_its_columns() {
    let rail = Rail::default();
    assert_eq!(rail.content_cols(120), rail.width());
    assert_eq!(rail.content_cols(40), 20);
    assert_eq!(rail.cols(40), 21, "the capped content plus the divider");
    assert_eq!(rail.content_cols(0), 0);
}

#[test]
fn dragging_the_divider_resizes_within_limits() {
    let mut rail = Rail::default();
    rail.drag_width(40, 120);
    assert_eq!(rail.width(), 28, "no drag without a press on the divider");
    rail.begin_drag();
    assert!(rail.is_dragging());
    rail.drag_width(40, 120);
    assert_eq!(rail.width(), 40);
    rail.drag_width(2, 120);
    assert_eq!(rail.width(), 16, "clamped to the minimum");
    rail.drag_width(100, 120);
    assert_eq!(rail.width(), 48, "clamped to the maximum");
    rail.drag_width(100, 60);
    assert_eq!(rail.width(), 30, "and to half the screen");
    rail.end_drag();
    rail.drag_width(40, 120);
    assert_eq!(rail.width(), 30, "the release ends the drag");
}

#[test]
fn a_collapsed_rail_refuses_the_drag() {
    let mut rail = Rail::default();
    rail.toggle();
    rail.begin_drag();
    assert!(!rail.is_dragging());
}

// ── The budget meter's spend ───────────────────

#[test]
fn a_metered_spend_fills_the_bound_budget() {
    let mut rail = Rail::default();
    rail.set_budget(Budget::capped(1200));
    assert_eq!(
        rail.budget().map(Budget::readout).as_deref(),
        Some("— / 12.00")
    );
    rail.note_spend(184);
    assert_eq!(
        rail.budget().map(Budget::readout).as_deref(),
        Some("1.84 / 12.00")
    );
    // The wire carries the session's cumulative cost, so a later report
    // replaces the earlier one rather than adding to it.
    rail.note_spend(361);
    assert_eq!(
        rail.budget().map(Budget::readout).as_deref(),
        Some("3.61 / 12.00")
    );
    assert_eq!(rail.spent_cents(), Some(361));
}

#[test]
fn a_new_session_settles_the_last_ones_spend_so_a_loop_meters_the_whole_run() {
    let mut rail = Rail::default();
    rail.set_budget(Budget::capped(1200));
    rail.note_spend(150);
    rail.settle_session();
    assert_eq!(rail.spent_cents(), Some(150), "settling loses nothing");
    rail.note_spend(40);
    assert_eq!(
        rail.spent_cents(),
        Some(190),
        "the second session adds to the first"
    );
    assert_eq!(
        rail.budget().map(Budget::readout).as_deref(),
        Some("1.90 / 12.00")
    );
    rail.settle_session();
    rail.settle_session();
    assert_eq!(
        rail.spent_cents(),
        Some(190),
        "settling twice is settling once"
    );
}

#[test]
fn spend_is_kept_without_a_budget_and_applied_when_one_binds() {
    let mut rail = Rail::default();
    rail.note_spend(184);
    assert_eq!(rail.budget(), None, "no cap, no meter");
    assert_eq!(rail.spent_cents(), Some(184), "the spend is still known");
    rail.set_budget(Budget::capped(1200));
    assert_eq!(
        rail.budget().map(Budget::readout).as_deref(),
        Some("1.84 / 12.00"),
        "a budget bound later wears the spend already metered"
    );
    assert_eq!(
        Rail::default().spent_cents(),
        None,
        "nothing metered, nothing claimed"
    );
}

// ── The agent block ────────────────────

#[test]
fn the_handshake_binds_the_agent_block_under_the_seeded_mode_word() {
    let mut rail = Rail::default();
    rail.set_mode("Workflow mode");
    assert_eq!(rail.agent(), None, "nothing named before the handshake");
    rail.bind_agent(None);
    assert_eq!(
        rail.agent(),
        None,
        "a handshake that named no model binds nothing"
    );
    rail.bind_agent(Some("sonnet"));
    let agent = rail.agent().expect("bound");
    assert_eq!(agent.identity(), "sonnet · Workflow mode");
    assert_eq!(agent.ledger(), "0.00 USD · 0 min · 0 requests");
}

#[test]
fn the_agents_ledger_moves_with_the_metering_message_and_the_turn_boundary() {
    let mut rail = Rail::default();
    rail.bind_agent(Some("sonnet"));
    rail.note_spend(61);
    rail.note_clock(12 * 60 + 30);
    rail.note_turn();
    rail.note_turn();
    let agent = rail.agent().expect("bound");
    assert_eq!(agent.identity(), "sonnet", "no mode word, no dangling dot");
    assert_eq!(agent.ledger(), "0.61 USD · 12 min · 2 requests");
    // The session's clock is a running total, not a delta.
    rail.note_clock(13 * 60);
    assert_eq!(
        rail.agent().expect("bound").ledger(),
        "0.61 USD · 13 min · 2 requests"
    );
}

#[test]
fn a_new_session_settles_the_clock_as_it_settles_the_spend() {
    let mut rail = Rail::default();
    rail.bind_agent(Some("sonnet"));
    rail.note_spend(100);
    rail.note_clock(10 * 60);
    rail.note_turn();
    rail.settle_session();
    rail.bind_agent(Some("haiku"));
    rail.note_spend(50);
    rail.note_clock(5 * 60);
    rail.note_turn();
    assert_eq!(
        rail.agent().expect("bound").ledger(),
        "1.50 USD · 15 min · 2 requests",
        "the run is the settled session plus the live one"
    );
    assert_eq!(rail.agent().expect("bound").identity(), "haiku");
}

#[test]
fn an_asserted_block_keeps_its_numbers_until_the_first_metering_message() {
    let mut rail = Rail::default();
    rail.set_agent(Some(
        SessionAgent::new("sonnet", "Draft mode").spent(61, 12, 2),
    ));
    assert_eq!(
        rail.agent().expect("asserted").ledger(),
        "0.61 USD · 12 min · 2 requests"
    );
    rail.note_turn();
    assert_eq!(
        rail.agent().expect("asserted").ledger(),
        "0.00 USD · 0 min · 1 request",
        "the first real figure replaces the asserted ones"
    );
}

/// The mode word is readable once seeded — the charter's cap reads it
/// after its own name — and absent before, so an unseeded cap reads
/// its bare name rather than a dangling dot.
#[test]
fn the_mode_word_is_readable_once_seeded() {
    let mut rail = Rail::default();
    assert_eq!(rail.mode(), None);
    rail.set_mode("Draft mode");
    assert_eq!(rail.mode(), Some("Draft mode"));
}

/// The window sizes its rail from its default width and moves it by
/// whole cells: zero columns dragged until the divider moves, then the
/// signed distance from the default — capped like the columns are.
#[test]
fn dragged_cols_is_the_signed_distance_from_the_default_width() {
    let mut rail = Rail::default();
    assert_eq!(
        rail.dragged_cols(120),
        0,
        "untouched, the rail is the design's"
    );
    rail.begin_drag();
    rail.drag_width(20, 120);
    assert_eq!(rail.dragged_cols(120), -8, "narrower is negative");
    rail.drag_width(40, 120);
    assert_eq!(rail.dragged_cols(120), 12, "wider is positive");
    assert_eq!(
        rail.dragged_cols(50),
        25 - 28,
        "a narrow screen caps the columns and the distance follows"
    );
}

/// The charter's offers are seeded beside the rows
/// and leave one at a time as they are accepted.
#[test]
fn offers_are_seeded_and_taken_by_index() {
    let mut rail = Rail::default();
    assert!(rail.offers().is_empty());
    rail.set_offers(vec![
        CharterOffer::new("a", "A", 5, 5),
        CharterOffer::new("b", "B", 6, 6),
    ]);
    assert_eq!(rail.take_offer(2), None);
    assert_eq!(
        rail.take_offer(0).map(|offer| offer.key().to_owned()),
        Some("a".to_owned())
    );
    assert_eq!(rail.offers().len(), 1);
    assert_eq!(rail.offers()[0].key(), "b");
}
