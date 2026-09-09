use super::*;

fn project(name: &str, done: Option<u32>, running: u32, asking: u32) -> GalaxyProject {
    GalaxyProject {
        name: name.to_owned(),
        done,
        running,
        asking,
        ..GalaxyProject::default()
    }
}

/// History is density: no history is the floor, every sixty-four
/// shipped chunks add a star, and the ceiling holds — the capture's
/// magistr (211 done) wears six shipped stars.
#[test]
fn shipped_history_sets_the_count_between_the_floor_and_the_ceiling() {
    let count = |done| Cluster::of(&project("p", done, 0, 0)).stars().len();
    assert_eq!(count(None), Cluster::FLOOR);
    assert_eq!(count(Some(0)), Cluster::FLOOR);
    assert_eq!(count(Some(63)), Cluster::FLOOR);
    assert_eq!(count(Some(64)), Cluster::FLOOR + 1);
    assert_eq!(count(Some(211)), 6, "the capture's magistr");
    assert_eq!(count(Some(10_000)), Cluster::FLOOR + Cluster::CEILING);
}

/// A live session and a pending question each add a star at the
/// chain's tail — working then asking — and nothing else is live.
#[test]
fn the_live_stars_are_the_tail_of_the_chain() {
    let both = Cluster::of(&project("p", Some(211), 1, 1));
    let states = both.states();
    assert_eq!(states.len(), 8);
    assert_eq!(states[6], StarState::Executing);
    assert_eq!(states[7], StarState::Stuck);
    assert!(states[..6].iter().all(|s| *s == StarState::Done));
    let quiet = Cluster::of(&project("p", None, 0, 0));
    assert!(quiet.states().iter().all(|s| *s == StarState::Done));
    let asking = Cluster::of(&project("p", None, 0, 2));
    assert_eq!(asking.states().last(), Some(&StarState::Stuck));
    assert_eq!(
        asking.states().len(),
        4,
        "a question is one star however many wait"
    );
}

/// The chain visits every star once; every star is inside the unit
/// box; and the shape is the name's — the same name is the same
/// cluster, two names are two.
#[test]
fn the_shape_is_the_names_and_stays_in_the_box() {
    let a = Cluster::of(&project("magistr", Some(211), 1, 0));
    let b = Cluster::of(&project("Magistr", Some(211), 1, 0));
    assert_eq!(a, b, "case does not change a project");
    let c = Cluster::of(&project("quicksilver", Some(211), 1, 0));
    assert_ne!(a.stars(), c.stars(), "two projects are two clusters");
    for cluster in [&a, &c] {
        for &(x, y) in cluster.stars() {
            assert!((-1.0..=1.0).contains(&x), "{x}");
            assert!((-0.5..=0.5).contains(&y), "{y}");
        }
        let edges = cluster.edges();
        assert_eq!(edges.len(), cluster.stars().len() - 1);
        assert!(edges.windows(2).all(|w| w[0].1 == w[1].0), "one chain");
    }
}

/// The derivation is pinned: a change here is a redesign of every
/// project's cluster, never an incidental refactor.
#[test]
fn the_derivation_is_pinned_across_builds() {
    let stars: Vec<(i32, i32)> = Cluster::of(&project("magistr", Some(211), 1, 0))
        .stars()
        .iter()
        .map(|&(x, y)| ((x * 1000.0).round() as i32, (y * 1000.0).round() as i32))
        .collect();
    assert_eq!(
        stars,
        vec![
            (881, 32),
            (555, 342),
            (-193, 355),
            (-916, 198),
            (-516, -154),
            (-223, -374),
            (577, -398)
        ]
    );
}
