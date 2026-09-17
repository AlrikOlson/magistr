use super::*;

#[test]
fn indeterminate_motion_is_continuous_at_turnarounds_and_cycle_boundaries() {
    assert_eq!(Motion::indeterminate_position(0.0), 0);
    assert_eq!(Motion::indeterminate_position(16.0), 1000);
    assert_eq!(Motion::indeterminate_position(32.0), 0);
    for step in 0..128 {
        let now = step as f32 / 4.0;
        let a = Motion::indeterminate_position(now);
        let b = Motion::indeterminate_position(now + 0.25);
        assert!(a.abs_diff(b) <= 25, "no wraparound jump at {now}");
        assert!(a <= 1000 && b <= 1000);
    }
    assert_ne!(
        Motion::indeterminate_position(8.0),
        Motion::indeterminate_position(8.5)
    );
}

use crate::cockpit::StageTab;

const fn key(tab: StageTab, dock: usize, rail: (u32, u32), roadmap: (usize, usize)) -> MotionKey {
    MotionKey {
        tab,
        dock,
        rail,
        roadmap,
        rows: 0,
        gates: 0,
        tasks: (0, 0),
        trace: 0,
        activity: 0,
        hover: None,
        rail_open: None,
    }
}

/// The still key, with only the counts an arrival or a finish moves.
const fn items(rows: usize, gates: usize, tasks: (usize, usize)) -> MotionKey {
    MotionKey {
        rows,
        gates,
        tasks,
        ..key(StageTab::Roadmap, 0, (0, 0), (0, 0))
    }
}

#[test]
fn a_fresh_cockpit_settles_the_first_progress_it_sees() {
    let mut motion = Motion::default();
    assert!(!motion.is_settling(0.0));
    let empty = key(StageTab::Roadmap, 0, (0, 0), (0, 0));
    motion.observe(empty, empty, 0.0, true);
    assert!(!motion.is_settling(0.0), "0/0 is not a target");
    let seeded = key(StageTab::Roadmap, 0, (3, 4), (134, 183));
    motion.observe(empty, seeded, 0.0, true);
    assert_eq!(motion.rail_fill(0.0), 0.75, "the first fill lands at once");
    assert!((motion.roadmap_fill(0.0) - 134.0 / 183.0).abs() < 1e-6);
    assert!(!motion.is_settling(0.0));
}

#[test]
fn progress_then_eases_to_its_next_value() {
    let mut motion = Motion::default();
    let a = key(StageTab::Roadmap, 0, (2, 4), (0, 0));
    let b = key(StageTab::Roadmap, 0, (3, 4), (0, 0));
    motion.observe(a, a, 0.0, true);
    motion.observe(a, b, 10.0, true);
    assert!(motion.is_settling(10.0));
    assert_eq!(motion.rail_fill(10.0), 0.5);
    let mid = motion.rail_fill(12.0);
    assert!(mid > 0.5 && mid < 0.75, "midway: {mid}");
    assert_eq!(motion.rail_fill((10 + Eased::STEPS) as f32), 0.75);
    assert!(!motion.is_settling((10 + Eased::STEPS) as f32));
}

#[test]
fn a_tab_switch_and_a_dock_arrival_transition_briefly() {
    let mut motion = Motion::default();
    let before = key(StageTab::Roadmap, 0, (0, 0), (0, 0));
    motion.observe(before, key(StageTab::Session, 0, (0, 0), (0, 0)), 5.0, true);
    assert_eq!(motion.tab_transition(5.0), Some(0.0));
    assert_eq!(motion.tab_transition(7.0), Some(0.5));
    assert_eq!(
        motion.tab_transition((5 + Motion::TRANSITION_STEPS) as f32),
        None
    );
    assert!(motion.is_settling(6.0));
    assert!(!motion.is_settling((5 + Motion::TRANSITION_STEPS) as f32));
    assert_eq!(motion.dock_transition(5.0), None, "the dock did not change");

    motion.observe(
        key(StageTab::Session, 0, (0, 0), (0, 0)),
        key(StageTab::Session, 1, (0, 0), (0, 0)),
        20.0,
        true,
    );
    assert_eq!(motion.dock_transition(20.0), Some(0.0));
    assert!(motion.dock_transition(21.0).is_some());
    assert_eq!(motion.dock_transition(24.0), None);
    // A card leaving is not an arrival.
    motion.observe(
        key(StageTab::Session, 1, (0, 0), (0, 0)),
        key(StageTab::Session, 0, (0, 0), (0, 0)),
        30.0,
        true,
    );
    assert_eq!(motion.dock_transition(30.0), None);
}

#[test]
fn reduced_motion_lands_fills_and_starts_no_transition() {
    let mut motion = Motion::default();
    let a = key(StageTab::Roadmap, 0, (2, 4), (0, 0));
    motion.observe(a, a, 0.0, false);
    motion.observe(a, key(StageTab::Session, 1, (3, 4), (0, 0)), 1.0, false);
    assert_eq!(motion.rail_fill(1.0), 0.75, "landed, no glide");
    assert_eq!(motion.tab_transition(1.0), None);
    assert_eq!(motion.dock_transition(1.0), None);
    assert!(!motion.is_settling(1.0));
}

#[test]
fn rows_that_arrive_fade_and_the_rows_that_were_there_do_not() {
    let mut motion = Motion::default();
    motion.observe(items(4, 0, (0, 0)), items(9, 0, (0, 0)), 5.0, true);
    // Only the rows past the ones that were already there.
    assert_eq!(motion.row_arrival(3, 5.0), None, "row 3 was already there");
    assert_eq!(motion.row_arrival(4, 5.0), Some(0.0), "the dim end");
    assert_eq!(motion.row_arrival(8, 5.0), Some(0.0));
    assert_eq!(
        motion.row_arrival(4, (5 + Motion::ARRIVAL_STEPS - 1) as f32),
        Some((Motion::ARRIVAL_STEPS - 1) as f32 / Motion::ARRIVAL_STEPS as f32)
    );
    assert_eq!(
        motion.row_arrival(4, (5 + Motion::ARRIVAL_STEPS) as f32),
        None,
        "settled: the row is its own pen again"
    );
    assert!(motion.is_settling(6.0));
    assert!(!motion.is_settling((5 + Motion::ARRIVAL_STEPS) as f32));
    // Rows leaving — a rewrap that narrows, a cleared turn — is not an
    // arrival.
    motion.observe(items(9, 0, (0, 0)), items(2, 0, (0, 0)), 30.0, true);
    assert_eq!(motion.row_arrival(1, 30.0), None);
    // A later arrival replaces the earlier one rather than piling up.
    motion.observe(items(2, 0, (0, 0)), items(3, 0, (0, 0)), 40.0, true);
    assert_eq!(motion.row_arrival(2, 40.0), Some(0.0));
    assert_eq!(motion.row_arrival(1, 40.0), None);
}

#[test]
fn a_gate_a_task_and_a_chunk_each_pop_once_when_they_finish() {
    let mut motion = Motion::default();
    // Two gates settle at once: both pop, the ones before them do not.
    motion.observe(items(0, 1, (0, 0)), items(0, 3, (0, 0)), 5.0, true);
    assert_eq!(motion.gate_pop(0, 5.0), None, "gate 0 had already settled");
    assert_eq!(motion.gate_pop(1, 5.0), Some(0.0));
    assert_eq!(motion.gate_pop(2, 7.0), Some(0.5));
    assert_eq!(
        motion.gate_pop(1, (5 + Motion::TRANSITION_STEPS) as f32),
        None
    );

    // A plan task completing pops the row it sits on, not its neighbours.
    motion.observe(items(0, 3, (0, 0)), items(0, 3, (1, 2)), 20.0, true);
    assert_eq!(motion.task_pop(2, 20.0), Some(0.0));
    assert_eq!(motion.task_pop(1, 20.0), None);
    assert_eq!(
        motion.task_pop(2, (20 + Motion::TRANSITION_STEPS) as f32),
        None
    );

    // A roadmap chunk completing pops the counter.
    let before = MotionKey {
        rail: (0, 4),
        ..items(0, 3, (1, 2))
    };
    let after = MotionKey {
        hover: None,
        rail_open: None,
        rail: (1, 4),
        ..before
    };
    assert_eq!(motion.chunk_pop(30.0), None);
    motion.observe(before, after, 30.0, true);
    assert_eq!(motion.chunk_pop(30.0), Some(0.0));
    assert_eq!(
        motion.chunk_pop((30 + Motion::TRANSITION_STEPS) as f32),
        None
    );
    assert!(motion.is_settling(31.0));
    // A roadmap arriving whole is a seed, not a hundred completions.
    let mut fresh = Motion::default();
    let empty = items(0, 0, (0, 0));
    fresh.observe(
        empty,
        MotionKey {
            rail: (134, 183),
            ..empty
        },
        0.0,
        true,
    );
    assert_eq!(fresh.chunk_pop(0.0), None, "the first roadmap is a seed");
}

#[test]
fn reduced_motion_starts_no_arrival_and_no_pop() {
    let mut motion = Motion::default();
    motion.observe(items(0, 0, (0, 0)), items(5, 2, (1, 1)), 1.0, false);
    assert_eq!(motion.row_arrival(3, 1.0), None);
    assert_eq!(motion.gate_pop(0, 1.0), None);
    assert_eq!(motion.task_pop(1, 1.0), None);
    assert_eq!(motion.chunk_pop(1.0), None);
    assert!(!motion.is_settling(1.0));
}

/// Every transient the motion layer holds is over inside the longest
/// one — the acceptance's "stops ticking within 8 steps".
#[test]
fn every_transient_stops_ticking_inside_eight_steps() {
    let mut motion = Motion::default();
    let before = key(StageTab::Roadmap, 0, (0, 4), (0, 4));
    motion.observe(before, before, 0.0, true);
    let after = MotionKey {
        tab: StageTab::Session,
        dock: 1,
        rail: (4, 4),
        roadmap: (4, 0),
        rows: 12,
        gates: 3,
        tasks: (2, 5),
        trace: 9,
        activity: 5,
        hover: None,
        rail_open: None,
    };
    motion.observe(before, after, 0.0, true);
    assert!(motion.is_settling(1.0), "something is in flight");
    assert!(
        !motion.is_settling(8.0),
        "every transient is over within 8 steps"
    );
}

/// Several folds landing between two ticks — a burst of agent deltas,
/// two gates finishing at once — arrive as one move rather than only the
/// last fold's share of it.
#[test]
fn folds_that_share_a_tick_share_one_arrival() {
    let mut motion = Motion::default();
    motion.observe(items(2, 0, (0, 0)), items(5, 0, (0, 0)), 9.0, true);
    motion.observe(items(5, 1, (0, 0)), items(8, 2, (0, 0)), 9.0, true);
    assert_eq!(
        motion.row_arrival(2, 9.0),
        Some(0.0),
        "the first fold's rows"
    );
    assert_eq!(motion.row_arrival(7, 9.0), Some(0.0), "the second fold's");
    assert_eq!(motion.row_arrival(1, 9.0), None, "what was already there");
    // Two gates settling in two folds on one tick both pop.
    motion.observe(items(8, 2, (0, 0)), items(8, 3, (0, 0)), 9.0, true);
    assert_eq!(motion.gate_pop(1, 9.0), Some(0.0));
    assert_eq!(motion.gate_pop(2, 9.0), Some(0.0));
    assert_eq!(motion.gate_pop(0, 9.0), None);
    // A later tick starts fresh.
    motion.observe(items(8, 3, (0, 0)), items(9, 3, (0, 0)), 10.0, true);
    assert_eq!(motion.row_arrival(2, 10.0), None);
    assert_eq!(motion.row_arrival(8, 10.0), Some(0.0));
}

/// a trace arriving whole sweeps the graph
/// in from the left; a step landing on a trace that was already there
/// draws in only its own end of it. Either way the sweep ends at exactly
/// `None`, so a settled Trace tab is the graph the tick never touched.
#[test]
fn the_trace_sweep_starts_where_the_graph_already_reached_and_ends_whole() {
    let trace = |steps: usize| MotionKey {
        trace: steps,
        ..key(StageTab::Session, 0, (0, 0), (0, 0))
    };

    // A trace arriving whole: the sweep starts at nothing.
    let mut seeded = Motion::default();
    seeded.observe(trace(0), trace(40), 0.0, true);
    assert_eq!(seeded.trace_sweep(40, 0.0), Some(0.0));
    assert!(seeded.is_settling(0.0));
    let mid = seeded.trace_sweep(40, 3.0).expect("still sweeping");
    assert!((0.4..0.6).contains(&mid), "half way in: {mid}");
    assert_eq!(
        seeded.trace_sweep(40, Motion::SWEEP_STEPS as f32),
        None,
        "the sweep is over and the graph draws whole"
    );
    assert!(!seeded.is_settling(Motion::SWEEP_STEPS as f32));

    // One step landing on a long trace: only its own end draws in.
    let mut grown = Motion::default();
    grown.observe(trace(19), trace(20), 0.0, true);
    let from = grown.trace_sweep(20, 0.0).expect("sweeping");
    assert!(from > 0.9, "only the last step is hidden: {from}");
    assert_eq!(grown.trace_sweep(20, Motion::SWEEP_STEPS as f32), None);

    // A trace that did not grow never sweeps, and neither does a graph
    // with no steps to divide by.
    let mut still = Motion::default();
    still.observe(trace(7), trace(7), 0.0, true);
    assert_eq!(still.trace_sweep(7, 0.0), None);
    assert_eq!(seeded.trace_sweep(0, 0.0), None);
}

/// the header's activity monitor draws itself
/// in on the same curve the trace graph does — from the share of the
/// slot that was already drawn, ending at exactly `None` so a settled
/// header is the one the tick never touched.
#[test]
fn the_activity_sweep_starts_where_the_ribbon_already_reached_and_ends_whole() {
    let spark = |samples: usize| MotionKey {
        activity: samples,
        ..key(StageTab::Roadmap, 0, (0, 0), (0, 0))
    };

    // The monitor arriving with a burst of busy frames behind it: the
    // line draws in from nothing.
    let mut arrived = Motion::default();
    arrived.observe(spark(0), spark(8), 0.0, true);
    assert_eq!(arrived.spark_sweep(8, 0.0), Some(0.0));
    assert!(arrived.is_settling(0.0));
    let mid = arrived.spark_sweep(8, 3.0).expect("still sweeping");
    assert!((0.4..0.6).contains(&mid), "half way in: {mid}");
    assert_eq!(
        arrived.spark_sweep(8, Motion::SWEEP_STEPS as f32),
        None,
        "the sweep is over and the line draws whole"
    );
    assert!(!arrived.is_settling(Motion::SWEEP_STEPS as f32));

    // One sample landing on a ribbon that was already drawn: only its
    // own column draws in.
    let mut grown = Motion::default();
    grown.observe(spark(7), spark(8), 0.0, true);
    let from = grown.spark_sweep(8, 0.0).expect("sweeping");
    assert!(from > 0.8, "only the newest column is hidden: {from}");
    assert_eq!(grown.spark_sweep(8, Motion::SWEEP_STEPS as f32), None);

    // A ribbon that did not grow never sweeps, and neither does a slot
    // with no width to divide by.
    let mut still = Motion::default();
    still.observe(spark(4), spark(4), 0.0, true);
    assert_eq!(still.spark_sweep(8, 0.0), None);
    assert_eq!(arrived.spark_sweep(0, 0.0), None);
}

/// The saturation guard, and the reason the cap lives in
/// `Model::motion_key` rather than here: once the ribbon fills the slot
/// its visible count stops growing, so a streaming session — which lands
/// a busy frame at a time, for minutes — takes no stamp at all. Without
/// it every busy frame would re-arm `is_settling` for another
/// `SWEEP_STEPS`, holding the tick past the news for a sweep that starts
/// at 1.0 and therefore draws nothing.
#[test]
fn a_saturated_activity_ribbon_stops_sweeping_and_holds_no_tick() {
    let spark = |samples: usize| MotionKey {
        activity: samples,
        ..key(StageTab::Roadmap, 0, (0, 0), (0, 0))
    };
    let mut motion = Motion::default();
    // The count the view reports is capped at the slot width, so a
    // hundred more busy frames are the same key as the eighth.
    motion.observe(spark(8), spark(8), 0.0, true);
    assert_eq!(motion.spark_sweep(8, 0.0), None);
    assert!(!motion.is_settling(0.0));
}

/// Under reduced motion the monitor never sweeps: `observe` takes no
/// transient stamp at all, so the line is whole on the first frame.
#[test]
fn reduced_motion_never_sweeps_the_activity_monitor() {
    let spark = |samples: usize| MotionKey {
        activity: samples,
        ..key(StageTab::Roadmap, 0, (0, 0), (0, 0))
    };
    let mut motion = Motion::default();
    motion.observe(spark(0), spark(8), 0.0, false);
    assert_eq!(motion.spark_sweep(8, 0.0), None);
    assert!(!motion.is_settling(0.0));
}

/// Under reduced motion the trace never sweeps: `observe` takes no
/// transient stamp at all, so the graph is whole on the first frame.
#[test]
fn reduced_motion_never_sweeps_the_trace() {
    let trace = |steps: usize| MotionKey {
        trace: steps,
        ..key(StageTab::Session, 0, (0, 0), (0, 0))
    };
    let mut motion = Motion::default();
    motion.observe(trace(0), trace(40), 0.0, false);
    assert_eq!(motion.trace_sweep(40, 0.0), None);
    assert!(!motion.is_settling(0.0));
}

// ---------------------------------------------------------------------
// the active chunk's breath.
// ---------------------------------------------------------------------

#[test]
fn the_glow_breathes_on_a_period_that_starts_and_ends_where_it_was() {
    assert_eq!(Motion::glow_phase(0.0), 0.0, "the foot of the wave");
    assert_eq!(
        Motion::glow_phase(Motion::GLOW_STEPS as f32),
        0.0,
        "and again a period later"
    );
    let crest = Motion::glow_phase((Motion::GLOW_STEPS / 2) as f32);
    assert!((crest - 0.5).abs() < f32::EPSILON, "the crest is halfway");
    // Strictly rising through the first half, so nothing stalls.
    for step in 1..Motion::GLOW_STEPS / 2 {
        assert!(Motion::glow_phase(step as f32) > Motion::glow_phase((step - 1) as f32));
    }
}

#[test]
fn the_glow_never_asks_for_a_tick_of_its_own() {
    // The idle guarantee: a motion layer with
    // nothing in it is settled at every step of the glow's period, so a
    // Roadmap tab with an in-progress chunk and nothing else moving
    // never starts the runtime's interval.
    let motion = Motion::default();
    for step in 0..Motion::GLOW_STEPS * 2 {
        assert!(!motion.is_settling(step as f32), "step {step}");
    }
}

#[test]
fn a_transient_is_read_between_two_ticks_and_ends_at_the_same_step_it_always_did() {
    // The transients are sampled on the same continuous clock the fills
    // are, so a tab pulse on a 120 Hz window has as many positions as the
    // display can show rather than four.
    let mut motion = Motion::default();
    let home = key(StageTab::Roadmap, 0, (0, 0), (0, 0));
    let trace = key(StageTab::Session, 0, (0, 0), (0, 0));
    motion.observe(home, trace, 0.0, true);
    let quarter = motion.tab_transition(0.5).expect("still running");
    assert!(
        quarter > 0.0 && quarter < 1.0 / Motion::TRANSITION_STEPS as f32,
        "half a tick in: {quarter}"
    );
    // The *duration* is unchanged: a transition still ends exactly
    // TRANSITION_STEPS after its stamp, so nothing was re-timed by
    // accident.
    assert!(
        motion
            .tab_transition(Motion::TRANSITION_STEPS as f32 - 0.001)
            .is_some()
    );
    assert!(
        motion
            .tab_transition(Motion::TRANSITION_STEPS as f32)
            .is_none()
    );
}

#[test]
fn folds_at_two_phases_of_one_tick_still_share_a_single_arrival() {
    // On a window two folds land at two moments inside one tick. They are
    // the burst `merge` exists for, so the stamp keeps the earlier index —
    // otherwise the rows already fading would fall out of the range the
    // stamp names and snap to settled mid-fade.
    let mut motion = Motion::default();
    motion.observe(items(0, 0, (0, 0)), items(2, 0, (0, 0)), 3.1, true);
    motion.observe(items(2, 0, (0, 0)), items(4, 0, (0, 0)), 3.6, true);
    assert!(
        motion.row_arrival(0, 3.6).is_some(),
        "the first fold's rows are still arriving"
    );
    assert!(motion.row_arrival(3, 3.6).is_some());
}

/// The rail's chrome fades in when it collapses or expands on a window,
/// and a terminal — whose key never
/// carries the rail's state — takes no stamp and no tick.
#[test]
fn the_rail_fades_its_chrome_in_on_a_window_and_never_on_a_terminal() {
    let open = MotionKey {
        rail_open: Some(true),
        ..key(StageTab::Turn, 0, (0, 0), (0, 0))
    };
    let shut = MotionKey {
        rail_open: Some(false),
        ..open
    };
    let mut motion = Motion::default();
    motion.observe(open, shut, 2.0, true);
    assert_eq!(
        motion.rail_fade(2.0),
        Some(0.0),
        "the fade starts at the toggle"
    );
    assert!(motion.is_settling(2.0));
    assert!(
        motion
            .rail_fade(2.0 + Motion::CHROME_FADE_STEPS as f32)
            .is_none()
    );
    assert!(!motion.is_settling(2.0 + Motion::CHROME_FADE_STEPS as f32));
    // Expanding again is a fade too.
    motion.observe(shut, open, 9.0, true);
    assert_eq!(motion.rail_fade(9.0), Some(0.0));
    // The same rail, resting: nothing.
    motion.observe(open, open, 20.0, true);
    assert!(motion.rail_fade(20.0).is_none());

    // A terminal's key is `None` on both sides of every fold.
    let terminal = key(StageTab::Turn, 0, (0, 0), (0, 0));
    let mut still = Motion::default();
    still.observe(terminal, terminal, 0.0, true);
    assert!(still.rail_fade(0.0).is_none());
    assert!(!still.is_settling(0.0));

    // Reduced motion never stamps.
    let mut reduced = Motion::default();
    reduced.observe(open, shut, 2.0, false);
    assert!(reduced.rail_fade(2.0).is_none());
}

// ---------------------------------------------------------------------
// every transient answers to one of the sixteen
// names, and settling is derived from the answer.
// ---------------------------------------------------------------------

/// Each stamp names itself by the design's word, and `is_settling` is
/// exactly "something is named": a layer with nothing in flight names
/// nothing, one fold that moves everything names the five words this
/// layer owns, and each word leaves as its transient ends.
#[test]
fn every_transient_carries_one_of_the_sixteen_names() {
    use MotionName::{Arrive, ChromeFade, Ignite, Sweep, ToastCounter};
    let mut motion = Motion::default();
    assert_eq!(
        motion.in_flight(0.0).count(),
        0,
        "nothing named, nothing settling"
    );
    let before = MotionKey {
        rail_open: Some(true),
        ..key(StageTab::Roadmap, 0, (0, 4), (0, 4))
    };
    motion.observe(before, before, 0.0, true);
    let after = MotionKey {
        tab: StageTab::Session,
        dock: 1,
        rail: (4, 4),
        roadmap: (4, 0),
        rows: 12,
        gates: 3,
        tasks: (2, 5),
        trace: 9,
        activity: 5,
        hover: None,
        rail_open: Some(false),
    };
    motion.observe(before, after, 0.0, true);
    let named: Vec<MotionName> = motion.in_flight(0.0).collect();
    assert_eq!(named, [Sweep, Arrive, Ignite, ToastCounter, ChromeFade]);
    for name in &named {
        assert!(
            MotionName::ALL.contains(name),
            "{name:?} is one of the sixteen"
        );
    }
    // The words leave with their transients: the rows' fade is the
    // shortest and the dock card's pop — the other *arrive* — a
    // transition, the sweep the longest, and settling follows the
    // names.
    for step in 0..=Motion::SWEEP_STEPS {
        let now = step as f32;
        let named: Vec<MotionName> = motion.in_flight(now).collect();
        assert_eq!(
            named.contains(&Arrive),
            step < Motion::TRANSITION_STEPS,
            "step {step}: {named:?}"
        );
        assert_eq!(
            named.contains(&Ignite),
            step < Motion::TRANSITION_STEPS,
            "step {step}: {named:?}"
        );
        assert_eq!(
            named.contains(&Sweep),
            step < Motion::SWEEP_STEPS,
            "step {step}: {named:?}"
        );
        assert_eq!(motion.is_settling(now), !named.is_empty(), "step {step}");
    }
    // The fills glide on after every pop: the tally is the last word.
    let late: Vec<MotionName> = motion.in_flight(Motion::SWEEP_STEPS as f32).collect();
    assert_eq!(late, [ToastCounter]);
}
