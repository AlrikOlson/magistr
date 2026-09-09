use super::*;

fn run(now: f32) -> Option<ShipRun> {
    ShipRun::at(3, 1, vec![7], 100.0, now)
}

/// The moment begins with the flare at the workstream's full hue and
/// no light on the edge at all.
#[test]
fn the_moment_opens_on_the_flare_with_nothing_run_yet() {
    let run = run(100.0).expect("the run has just started");
    assert_eq!(run.star, 3);
    assert_eq!(run.region, 1);
    assert_eq!(run.unlocked, vec![7]);
    assert_eq!(run.ignite, 0.0);
    assert_eq!(run.edge_run, 0.0);
}

/// The light cannot leave a star that has not lit: the edge-run is
/// pinned at zero for the whole of the ignite and only moves after it.
#[test]
fn the_light_leaves_only_once_the_star_has_ignited() {
    for step in 0..ShipRun::IGNITE_STEPS {
        let run = run(100.0 + step as f32).expect("still igniting");
        assert_eq!(run.edge_run, 0.0, "step {step}");
        assert!(run.ignite < 1.0, "step {step}");
    }
    let settled = run(100.0 + ShipRun::IGNITE_STEPS as f32).expect("the ignite is over");
    assert_eq!(settled.ignite, 1.0);
    assert_eq!(settled.edge_run, 0.0, "and the run starts from the star");
    let running = run(100.0 + ShipRun::IGNITE_STEPS as f32 + 3.0).expect("running");
    assert!(running.edge_run > 0.0 && running.edge_run < 1.0);
}

/// A settled sky draws what a sky that shipped nothing draws: the run
/// stops existing rather than pinning itself at its end frame.
#[test]
fn the_run_goes_out_of_existence_when_it_is_over() {
    assert!(run(100.0 + ShipRun::STEPS as f32 - 0.5).is_some());
    assert!(run(100.0 + ShipRun::STEPS as f32).is_none());
    assert!(run(1_000.0).is_none());
    assert!(run(99.0).is_none(), "and a tick before it began is no run");
}

/// The light runs to exactly the stars this completion unlocked —
/// not to the star it left, and not to one that was ready already.
#[test]
fn the_light_runs_only_to_what_the_completion_unlocked() {
    let run = run(100.0 + ShipRun::IGNITE_STEPS as f32 + 1.0).expect("running");
    assert!(run.runs_to(7));
    assert!(!run.runs_to(3), "the star it left is not a target");
    assert!(!run.runs_to(9), "nor is a star it never unlocked");
}
