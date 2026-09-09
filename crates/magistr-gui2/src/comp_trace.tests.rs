use magistr_cockpit::cockpit::{Place, Size, Widget};

use super::*;
use crate::element::Kind;
use crate::fixture::Fixture;

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

fn ids(root: &Element) -> Vec<ElementId> {
    let mut ids = Vec::new();
    root.each(&mut |_, element| ids.extend(element.id));
    ids
}

fn view() -> Model {
    Fixture::mock("11", Size::new(120, 40)).expect("view 11 has a fixture")
}

/// The fixture is the capture's trace: 102 main steps, 8 pins, one kept
/// branch off 386, 388 opened — at the Trace.
#[test]
fn the_fixture_is_the_captures_trace() {
    let model = view();
    assert_eq!(model.place(), Some(Place::Trace));
    assert_eq!(TraceComposition::main_steps(&model).len(), 102);
    assert_eq!(model.trace().steps().iter().filter(|s| s.pinned).count(), 8);
    assert!(model.trace().is_expanded(388));
    assert_eq!(model.trace().branch_status("alt"), "kept");
    let window: Vec<u32> = TraceComposition::window(&model)
        .iter()
        .map(|(_, step)| step.number)
        .collect();
    assert_eq!(
        window,
        [386, 388, 389],
        "the branch step is not on the main line"
    );
}

/// The header slot reads the trace's own counts, in the volume's words.
#[test]
fn the_chips_count_the_main_line_and_the_pins() {
    let chips = texts(&TraceComposition::chips(&view(), Medium::Night));
    assert_eq!(chips, vec!["102 steps \u{b7} 8 pinned", "esc \u{b7} Back"]);
}

/// The feed: three cards in the capture's own copy — the quiet 386 with
/// its plain ref, the pinned 388 with its two chips and its verbs, the
/// live 389 with none — every card hittable as its trace row.
#[test]
fn the_feed_shows_the_windows_cards_with_their_states_refs_and_verbs() {
    let model = view();
    let body = TraceComposition::el(&model, Medium::Night);
    let texts = texts(&body);
    for expected in [
        "386",
        "chunk:gui2-2-session-pane",
        "388 \u{b7} Pinned",
        "task:design-canvas",
        "chunk:gui2-2a",
        "Unpin",
        "View receipts",
        "389 \u{b7} In progress",
        "Close gui2-2a: nine volumes, ten decisions, the catalog rule",
    ] {
        assert!(texts.iter().any(|t| t == expected), "{expected}: {texts:?}");
    }
    assert!(
        !texts.iter().any(|t| t == "387"),
        "the branch step is the rail's, not the feed's"
    );
    let ids = ids(&body);
    for (index, _) in TraceComposition::window(&model) {
        assert!(
            ids.contains(&ElementId(TraceComposition::CARDS + index as u64)),
            "card {index} is hittable"
        );
    }
    assert!(ids.contains(&ElementId(TraceComposition::PINS + 388)));
    assert!(ids.contains(&ElementId(TraceComposition::RECEIPTS + 388)));
    assert!(
        !ids.contains(&ElementId(TraceComposition::PINS + 386)),
        "verbs only on the opened step"
    );
    assert!(
        ids.contains(&TraceComposition::GRAPH),
        "the graph is reserved"
    );
}

/// The rail reads the trace: the main count, the branches' count and
/// the newest branch's status, and the three newest pins by number.
#[test]
fn the_rail_reads_main_branches_and_the_newest_pins() {
    let body = TraceComposition::el(&view(), Medium::Day);
    let texts = texts(&body);
    for expected in [
        "Branches",
        "main",
        "102 steps",
        "branches",
        "1 \u{b7} kept",
        "Pinned",
        "388 \u{b7} open gui2-2a",
        "387 \u{b7} close session pane",
        "381 \u{b7} gui2 is the flagship",
    ] {
        assert!(texts.iter().any(|t| t == expected), "{expected}: {texts:?}");
    }
    assert!(
        !texts.iter().any(|t| t.starts_with("340 ")),
        "only the newest three pins are listed"
    );
}

/// The lane is derived from the same window: the anchor before it,
/// then the three — 388 focused, 389 open, the fork leaving 386 for
/// the branch step's feed index.
#[test]
fn the_lane_is_the_windows_steps_with_their_marks_and_the_fork() {
    let model = view();
    let lane = TraceComposition::lane(&model);
    let steps = model.trace().steps();
    let numbers: Vec<u32> = lane.iter().map(|stop| steps[stop.index].number).collect();
    assert_eq!(numbers, [385, 386, 388, 389]);
    assert_eq!(
        lane.iter().map(|s| s.focused).collect::<Vec<_>>(),
        [false, false, true, false]
    );
    assert_eq!(
        lane.iter().map(|s| s.open).collect::<Vec<_>>(),
        [false, false, false, true]
    );
    let fork = lane[1].fork.expect("386 forks");
    assert_eq!(steps[fork].number, 387);
    assert!(lane.iter().filter(|s| s.fork.is_some()).count() == 1);
}

/// An empty trace composes honestly: no cards, the rail's empty pin
/// state, a lane with no stops, a chip that says so.
#[test]
fn an_empty_trace_has_no_cards_and_an_empty_lane() {
    use magistr_cockpit::cockpit::Surface;
    let model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_place(Place::Trace);
    assert!(TraceComposition::lane(&model).is_empty());
    let texts = texts(&TraceComposition::el(&model, Medium::Night));
    assert!(texts.iter().any(|t| t == "No pins yet"));
    assert!(texts.iter().any(|t| t == "0 steps"));
    assert!(texts.iter().any(|t| t == "none"));
    let chips = texts_of_chips(&model);
    assert_eq!(chips[0], "0 steps \u{b7} 0 pinned");
}

fn texts_of_chips(model: &Model) -> Vec<String> {
    texts(&TraceComposition::chips(model, Medium::Night))
}

/// The frame's wiring: the graph's field is the trace graph widget, a
/// card is its trace row, the verbs are the pin and the chunk jump.
#[test]
fn the_ids_route_to_the_cockpits_trace_widgets() {
    use crate::comp_root::RootComposition;
    let model = view();
    assert_eq!(
        RootComposition::target_of(&model, TraceComposition::GRAPH),
        Some(Widget::TraceGraph.id())
    );
    assert_eq!(
        RootComposition::target_of(&model, ElementId(TraceComposition::CARDS + 101)),
        Some(Widget::trace_row(101))
    );
    assert_eq!(
        RootComposition::target_of(&model, ElementId(TraceComposition::PINS + 388)),
        Some(Widget::trace_pin(388))
    );
    assert_eq!(
        RootComposition::target_of(&model, ElementId(TraceComposition::RECEIPTS + 388)),
        Some(Widget::trace_chunk(388))
    );
}
