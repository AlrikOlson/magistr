use magistr_cockpit::cockpit::{Model, Place, Size, Widget};

use super::*;
use crate::comp_root::RootComposition;
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

fn view() -> Model {
    Fixture::mock("13", Size::new(120, 40)).expect("view 13 has a fixture")
}

/// The fixture is the capture: `contrast floor` typed over the session,
/// six hits, four of them shown under three caps.
#[test]
fn the_fixture_is_the_captures_search() {
    let model = view();
    assert_eq!(model.place(), Some(Place::Session));
    let search = model.search().expect("⌘⇧F raised it");
    assert_eq!(search.query(), "contrast floor");
    assert_eq!(search.scope(), SearchScope::Everything);
    let results = Search::results(&model);
    assert_eq!(results.count_text(), "6 results");
    assert_eq!(results.rows.len(), 4);
}

#[test]
fn the_float_rises_with_the_model_and_shows_the_captures_words() {
    let closed = Model::new(Size::new(120, 40));
    assert!(SearchComposition::el(&closed, Medium::Night).is_none());

    let model = view();
    let float = SearchComposition::el(&model, Medium::Night).expect("raised");
    let texts = texts(&float);
    let joined = texts.join("");
    assert!(texts.iter().any(|t| t == "\u{2318}\u{21e7}F"), "the chord");
    assert!(texts.iter().any(|t| t == "contrast floor"), "the query");
    assert!(texts.iter().any(|t| t == "6 results"), "the count");
    for scope in SearchScope::ALL {
        assert!(
            texts.iter().any(|t| t == scope.label()),
            "{}",
            scope.label()
        );
    }
    for word in ["reports", "trace", "receipts"] {
        assert!(texts.iter().any(|t| t == word), "the {word} cap");
    }
    assert!(
        !texts
            .iter()
            .any(|t| t == "sessions" && texts.iter().filter(|u| *u == t).count() > 1),
        "no session matched, so no sessions cap beside the chip"
    );
    assert!(
        texts
            .iter()
            .any(|t| t == "Every light palette clears its floor")
    );
    assert!(
        joined.contains(
            "nudged until it clears the contrast floor the validator declares\u{2026} \u{b7} Aug 29"
        ),
        "{joined}"
    );
    assert!(
        joined.contains("Step 387 \u{b7} \u{201c}\u{2026}the pen on screen"),
        "{joined}"
    );
    assert!(
        joined.contains("ensure_contrast at the consumer \u{b7} code \u{b7} theme/ink.rs"),
        "{joined}"
    );
}

#[test]
fn the_chosen_row_is_washed_and_the_first_chip_is_picked() {
    let model = view();
    let float = SearchComposition::el(&model, Medium::Day).expect("raised");
    let mut chosen = None;
    let mut idle = None;
    let mut picked = None;
    let mut unpicked = None;
    float.each(&mut |_, element| match element.id {
        Some(ElementId(SearchComposition::ROWS)) => chosen = Some(element.fill.is_some()),
        Some(ElementId(n)) if n == SearchComposition::ROWS + 1 => {
            idle = Some(element.fill.is_some())
        }
        Some(ElementId(SearchComposition::SCOPES)) => picked = Some(element.fill.is_some()),
        Some(ElementId(n)) if n == SearchComposition::SCOPES + 1 => {
            unpicked = Some(element.stroke.is_some());
        }
        _ => {}
    });
    assert_eq!(chosen, Some(true));
    assert_eq!(idle, Some(false));
    assert_eq!(picked, Some(true));
    assert_eq!(unpicked, Some(true));
}

#[test]
fn the_root_floats_the_search_over_the_dimmed_room_without_its_chrome_and_routes_its_ids() {
    let model = view();
    let clock = magistr_cockpit::cockpit::FrameClock::new(0, std::time::Duration::ZERO);
    let root = RootComposition::el(&model, Medium::Night, (8.0, 17.0), (1100.0, 640.0), &clock);
    let texts = texts(&root);
    assert!(
        texts.iter().any(|t| t == "6 results"),
        "the float is composed"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("The vocabulary is 48 static ids")),
        "the room stays behind the float: {texts:?}"
    );
    assert!(
        !texts
            .iter()
            .any(|t| t == "projects" || t == "Steer" || t == "Checkpoints"),
        "the chrome stands down: {texts:?}"
    );
    assert_eq!(
        RootComposition::target_of(&model, ElementId(SearchComposition::ROWS + 2)),
        Some(Widget::search_row(2)),
    );
    assert_eq!(
        RootComposition::target_of(&model, ElementId(SearchComposition::SCOPES + 3)),
        Some(Widget::search_scope(3)),
    );
    assert_eq!(
        RootComposition::target_of(&model, ElementId(SearchComposition::SCOPES_END)),
        None,
    );
}
