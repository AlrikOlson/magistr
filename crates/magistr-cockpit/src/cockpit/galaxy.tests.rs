use super::*;

fn three() -> GalaxyData {
    GalaxyData {
        projects: vec![
            GalaxyProject {
                name: "magistr".to_owned(),
                done: Some(211),
                running: 1,
                ..GalaxyProject::default()
            },
            GalaxyProject {
                name: "think-and-ship".to_owned(),
                here: true,
                asking: 1,
                asked_secs: Some(11 * 60),
                running: 1,
                ..GalaxyProject::default()
            },
            GalaxyProject {
                name: "sample-project".to_owned(),
                idle_secs: Some(3 * 86_400),
                ..GalaxyProject::default()
            },
        ],
        spent_cents: 245,
        cap_cents: Some(1200),
        stop_cents: Some(1000),
    }
}

/// Until the job returns there is nothing: no projects, no home, no
/// totals — and the walk has nothing to walk.
#[test]
fn an_unseeded_galaxy_is_empty_and_walks_nowhere() {
    let mut galaxy = Galaxy::default();
    assert!(galaxy.projects().is_empty());
    assert_eq!(galaxy.home(), None);
    assert_eq!((galaxy.running(), galaxy.asking()), (0, 0));
    assert!(!galaxy.select_step(true));
    assert!(!galaxy.select(0));
    assert_eq!(galaxy.selected(), None);
}

/// The totals the header chips read are sums over every project, and
/// the home is the project the data marks as this window's own.
#[test]
fn the_totals_and_the_home_come_from_the_data() {
    let mut galaxy = Galaxy::default();
    galaxy.seed(three());
    assert_eq!(galaxy.running(), 2, "2 running");
    assert_eq!(galaxy.asking(), 1, "1 action required");
    assert_eq!(galaxy.home(), Some(1), "think-and-ship is the window's");
    assert!(galaxy.projects()[1].is_asking());
    assert!(galaxy.projects()[0].is_live());
    assert!(!galaxy.projects()[2].is_live());
}

/// The arrows walk the selection from either end and stop at the ends;
/// a reseed that shrinks the list drops a selection past its end.
#[test]
fn the_selection_walks_between_the_ends_and_survives_a_reseed() {
    let mut galaxy = Galaxy::default();
    galaxy.seed(three());
    assert!(galaxy.select_step(false));
    assert_eq!(
        galaxy.selected(),
        Some(2),
        "back from nothing opens at the last"
    );
    assert!(galaxy.select_step(true));
    assert_eq!(galaxy.selected(), Some(2), "and stops at the end");
    assert!(galaxy.select_step(false));
    assert!(galaxy.select_step(false));
    assert!(galaxy.select_step(false));
    assert_eq!(galaxy.selected(), Some(0), "stops at the start too");
    assert!(galaxy.select(2));
    let mut fewer = three();
    fewer.projects.truncate(1);
    galaxy.seed(fewer);
    assert_eq!(
        galaxy.selected(),
        None,
        "a selection past the new end is dropped"
    );
    assert!(
        !galaxy.select(5),
        "and nothing past the end can be selected"
    );
}

/// The wheel zooms between two stops; pushing against a stop moves
/// nothing, and reset puts the field back to rest.
#[test]
fn zoom_is_held_between_its_stops() {
    let mut galaxy = Galaxy::default();
    assert!((galaxy.zoom() - 1.0).abs() < f32::EPSILON);
    assert!(galaxy.zoom_by(2.0));
    assert!(galaxy.zoom_by(2.0));
    assert!((galaxy.zoom() - Galaxy::ZOOM_MAX).abs() < f32::EPSILON);
    assert!(!galaxy.zoom_by(2.0), "already at the far stop");
    assert!(galaxy.reset_zoom());
    assert!(!galaxy.reset_zoom(), "already at rest");
    assert!(galaxy.zoom_by(0.1));
    assert!((galaxy.zoom() - Galaxy::ZOOM_MIN).abs() < f32::EPSILON);
}
