use super::*;
use crate::harness::fixtures;
use serde_json::json;

#[test]
fn empty_tracks_have_no_focus_or_fabricated_items() {
    let mut roadmap = Roadmap::default();
    roadmap.move_track_focus(1);
    assert!(roadmap.track_rows().is_empty());
    assert!(roadmap.track_focus().is_none());
    assert!(!roadmap.reveal_selected());
}

#[test]
fn complete_snapshot_counts_do_not_depend_on_history_search_or_track_filter() {
    let chunks = fixtures::chunks_from(vec![
        json!({"id":"done","name":"Foundation","status":"done","group":"Core"}),
        json!({"id":"ready","name":"Ready work","status":"pending","group":"Core","deps":["done"]}),
        json!({"id":"waiting","name":"Waiting work","status":"pending","group":"Core","deps":["missing"]}),
        json!({"id":"blocked","name":"Blocked work","status":"pending","group":"Core","blocked_by":{"kind":"external","reason":"Needs access"}}),
        json!({"id":"working","name":"Working work","status":"in_progress","group":"Core"}),
        json!({"id":"backlog","name":"Planned work","status":"backlog","group":"Core"}),
        json!({"id":"other","name":"Other work","status":"pending","group":"Other","deps":["waiting"]}),
        json!({"id":"obsolete","name":"Retired work","status":"obsoleted","group":"Other"}),
    ]);
    let mut roadmap = Roadmap::default();
    roadmap.seed(&chunks, &[]);
    let counts = |roadmap: &Roadmap| {
        roadmap
            .track_rows()
            .iter()
            .find(|r| r.label() == "Core")
            .unwrap()
            .counts()
            .unwrap()
    };
    assert_eq!(
        counts(&roadmap),
        TrackCounts {
            ready: 1,
            working: 1,
            waiting: 1,
            blocked: 1,
            backlog: 1
        }
    );
    roadmap.cycle_track_filter();
    roadmap.set_track_query("Foundation".into());
    assert_eq!(
        counts(&roadmap),
        TrackCounts {
            ready: 1,
            working: 1,
            waiting: 1,
            blocked: 1,
            backlog: 1
        }
    );
    roadmap.set_track_query(String::new());
    assert_eq!(roadmap.track_filter_label(), "Core");
    assert_eq!(roadmap.track_rows()[1].label(), "Working work");
    assert_eq!(roadmap.dependents("waiting")[0].name, "Other work");
    assert_eq!(roadmap.track_of(roadmap.dependents("waiting")[0]), "Other");
    roadmap.browse.dependencies = true;
    assert!(roadmap.graph_ids().contains(&"done"));
    assert!(roadmap.is_selected_visible());
}

#[test]
fn full_history_track_navigation_keeps_geometry_lazy_and_ties_stable() {
    let chunks = fixtures::chunks_from(
        (0..900)
            .map(|n| {
                json!({
                    "id":format!("item-{n:03}"), "name":format!("A long readable work name {n:03}"),
                    "status": if n % 3 == 0 { "done" } else { "pending" },
                    "priority":100, "group":format!("Track {:02}", n % 90),
                })
            })
            .collect(),
    );
    let mut roadmap = Roadmap::default();
    roadmap.seed(&chunks, &[]);
    let tracks = roadmap.track_rows();
    assert_eq!(
        tracks
            .iter()
            .filter(|r| matches!(r, TrackRow::Heading { .. }))
            .count(),
        60
    );
    assert_eq!(
        tracks
            .iter()
            .filter(|r| matches!(r, TrackRow::Item(..)))
            .count(),
        600
    );
    roadmap.set_track_query("long readable".into());
    assert_eq!(
        roadmap
            .track_rows()
            .iter()
            .filter(|r| matches!(r, TrackRow::Item(..)))
            .count(),
        900
    );
    assert!(roadmap.geometry.full.get().is_none());
    assert!(roadmap.geometry.frontier.get().is_none());
    let ids: Vec<_> = roadmap
        .track_rows()
        .iter()
        .filter_map(|r| match r {
            TrackRow::Item(c, _) => Some(c.id.clone()),
            _ => None,
        })
        .collect();
    let mut reordered = chunks;
    reordered.reverse();
    roadmap.seed(&reordered, &[]);
    assert_eq!(
        roadmap
            .track_rows()
            .iter()
            .filter_map(|r| match r {
                TrackRow::Item(c, _) => Some(c.id.clone()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        ids
    );
    roadmap.move_track_focus(isize::MAX);
    roadmap.move_track_focus(isize::MIN);
    assert!(matches!(roadmap.track_focus(), Some(TrackFocus::Track(_))));
    roadmap.branch_track_focus(false);
    roadmap.set_track_query(String::new());
    assert!(roadmap.geometry.full.get().is_none());
}
