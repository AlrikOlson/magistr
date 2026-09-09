use magistr_cockpit::cockpit::{Place, Size, Surface};

use super::*;
use crate::element::Kind;
use crate::fixture::Fixture;

fn ids_of(root: &Element) -> Vec<ElementId> {
    let mut ids = Vec::new();
    root.each(&mut |_, element| ids.extend(element.id));
    ids
}

/// At the Trace the here stop wears the trace glyph — the volume's
/// `[ic-trace] trace` — and no other stop does; at the Changes journal
/// the record stop is unmarked, since no view of it is built yet.
#[test]
fn the_record_stop_wears_the_trace_glyph_at_the_trace() {
    let mut model = Fixture::mock("05", Size::new(120, 40))
        .expect("view 05 has a fixture")
        .with_place(Place::Trace);
    model.set_surface(Surface::Native);
    let header = HeaderComposition::el(&model, Medium::Night);
    let mut marks = Vec::new();
    header.each(&mut |_, element| {
        if let Kind::Mark { mark, .. } = &element.kind {
            marks.push(*mark);
        }
    });
    assert!(
        marks.contains(&Mark::Icon(Icon::Trace)),
        "the trace stop is marked: {marks:?}"
    );
    let mut changes = model.with_place(Place::Changes);
    changes.set_surface(Surface::Native);
    let header = HeaderComposition::el(&changes, Medium::Night);
    let mut marks = Vec::new();
    header.each(&mut |_, element| {
        if let Kind::Mark { mark, .. } = &element.kind {
            marks.push(*mark);
        }
    });
    assert!(!marks.contains(&Mark::Icon(Icon::Trace)));
}

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

/// A session on a chunk reads `projects › magistr › <chunk>`: the
/// ancestors press, the chunk is the here stop — the composition's
/// whole content is the model's own reading of where you are.
#[test]
fn the_path_walks_projects_project_chunk_off_the_model() {
    let mut model = Fixture::busy(Size::new(120, 40));
    model.set_surface(Surface::Native);
    let header = HeaderComposition::el(&model, Medium::Night);
    let texts = texts(&header);
    assert!(texts.iter().any(|t| t == "projects"));
    assert!(texts.iter().any(|t| t == "magistr"));
    let mut ids = Vec::new();
    header.each(&mut |_, element| ids.extend(element.id));
    assert!(ids.contains(&HeaderComposition::PROJECTS));
}

/// The chunk stop appears exactly when the rail knows one, and it is
/// the here stop — so starting a chunk reshapes the path with no state
/// of the composition's own.
#[test]
fn the_chunk_stop_follows_the_rails_you_are_here() {
    let mut without = Fixture::busy(Size::new(120, 40));
    without.set_surface(Surface::Native);
    assert!(without.rail().chunk().is_none());
    let mut mock = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    mock.set_surface(Surface::Native);
    let chunk = mock
        .rail()
        .chunk()
        .expect("view 05 is on a chunk")
        .to_owned();
    let header = HeaderComposition::el(&mock, Medium::Night);
    assert!(texts(&header).contains(&chunk));
    let bare = HeaderComposition::el(&without, Medium::Night);
    assert!(!texts(&bare).contains(&chunk));
    let mut ids = Vec::new();
    header.each(&mut |_, element| ids.extend(element.id));
    assert!(
        ids.contains(&HeaderComposition::PROJECT),
        "with a chunk below it, the project presses"
    );
}

/// The crumbs are the path, for every place the camera can be at:
/// the header's stops are exactly [`Model::path`]'s labels in order,
/// with one here stop — derived, never assembled here. At the Galaxy
/// the path is `projects` alone; at the Sky the project is the here
/// stop; from the Session down the chunk is.
#[test]
fn the_crumbs_are_derived_from_the_path_for_every_place() {
    for place in Place::ALL {
        let mut model = Fixture::mock("05", Size::new(120, 40))
            .expect("view 05 has a fixture")
            .with_place(place);
        model.set_surface(Surface::Native);
        let path = model.path();
        let header = HeaderComposition::el(&model, Medium::Night);
        let labels: Vec<String> = texts(&header)
            .into_iter()
            .filter(|t| t != "\u{25b8}")
            .collect();
        let expected: Vec<String> = path.iter().map(|stop| stop.label.clone()).collect();
        assert_eq!(labels, expected, "{place:?}");
        assert_eq!(
            path.iter().filter(|stop| stop.here).count(),
            1,
            "{place:?} has one here stop"
        );
        let last = path.last().expect("a path is never empty on a window");
        if place == Place::Session {
            // The started stop trails the here stop at the Session.
            assert_eq!(last.label, "session \u{b7} 09:02");
            assert!(path[path.len() - 2].here, "the chunk is here");
        } else {
            assert!(last.here, "{place:?}: the deepest stop is here");
        }
        match place {
            Place::Galaxy => assert_eq!(expected, ["projects"]),
            Place::Sky => assert_eq!(expected.len(), 2),
            Place::Signals => {
                assert_eq!(
                    expected,
                    ["projects", "magistr", "signals"],
                    "the inbox hangs off the project, chunk or no chunk"
                );
                assert!(
                    ids_of(&header).contains(&HeaderComposition::PROJECT),
                    "the project presses back to the Sky"
                );
            }
            Place::Session => assert_eq!(
                expected,
                ["projects", "magistr", "gui2-2a", "session \u{b7} 09:02"],
                "the Session names the chunk and when it started"
            ),
            // A flow never started stands on the ground, which floats
            // over the Galaxy and wears the Galaxy's own path.
            Place::Birth => assert_eq!(expected, ["projects"]),
            // The Report hangs off the project like the inbox: with no
            // shipped chunk selected under it the here stop is the
            // record's own word.
            Place::Report => {
                assert_eq!(expected, ["projects", "magistr", "report"]);
                assert!(
                    ids_of(&header).contains(&HeaderComposition::PROJECT),
                    "the project presses back to the Sky"
                );
            }
            Place::Changes | Place::Trace => {
                assert_eq!(
                    expected.len(),
                    4,
                    "{place:?} names the chunk and the record"
                );
                assert!(
                    ids_of(&header).contains(&HeaderComposition::SESSION),
                    "{place:?}: the chunk presses back to the Session"
                );
            }
        }
    }
}
