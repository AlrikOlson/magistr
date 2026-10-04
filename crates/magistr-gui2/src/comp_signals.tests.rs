use std::time::Duration;

use magistr_cockpit::cockpit::{Place, Size, Surface, Widget};

use super::*;
use crate::comp_root::RootComposition;
use crate::comp_sky::SkyComposition;
use crate::element::Kind;
use crate::fixture::Fixture;

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.trim().to_owned());
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
    Fixture::mock("12", Size::new(120, 40)).expect("view 12 has a fixture")
}

fn now() -> FrameClock {
    FrameClock::new(0, Duration::ZERO).with_epoch_secs(Fixture::NOW_SECS)
}

/// The fixture is the capture's inbox: three signals, two of them open,
/// at the Signals place with the project's three-stop path.
#[test]
fn the_fixture_is_the_captures_inbox() {
    let model = view();
    assert_eq!(model.place(), Some(Place::Signals));
    assert_eq!(model.roadmap().inbox().len(), 3);
    assert_eq!(SignalsComposition::open_count(&model), 2);
    let labels: Vec<String> = model.path().into_iter().map(|stop| stop.label).collect();
    assert_eq!(labels, ["projects", "magistr", "signals"]);
}

/// The header slot counts the open signals, and says so in the volume's
/// words; an empty inbox says `0 open` in the quiet chip.
#[test]
fn the_chip_counts_the_open_signals() {
    assert_eq!(
        texts(&SignalsComposition::chips(&view(), Medium::Night)),
        vec!["2 open"]
    );
    let empty = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_place(Place::Signals);
    assert_eq!(
        texts(&SignalsComposition::chips(&empty, Medium::Day)),
        vec!["0 open"]
    );
    assert!(
        texts(&SignalsComposition::el(&empty, Medium::Day, &now())).is_empty(),
        "no cards for no signals"
    );
}

/// The three cards in the capture's own copy: the Open card's cap with
/// who and how long ago, its summary as the title and the stakeholder's
/// words under it, its three verbs; the Researched card's findings and
/// two verbs; the Promoted card naming its star and the star's status,
/// with no verbs at all.
#[test]
fn the_cards_read_the_captures_states_words_and_verbs() {
    let model = view();
    let body = SignalsComposition::el(&model, Medium::Night, &now());
    let texts = texts(&body);
    for expected in [
        "Open \u{b7} developer \u{b7} 2 d",
        "Retina window renders text at half size",
        "Affects all native windows. Cell metrics ignore the display scale factor.",
        "Promote",
        "Research",
        "Snooze",
        "Researched \u{b7} team \u{b7} 5 d",
        "Screen readers cannot trigger actions in the accessibility tree",
        "AccessKit 0.24 supports actions. All actions are currently refused \
         (gui2-accesskit-actions). Estimated effort: 1 chunk.",
        "Promoted \u{b7} 7 d",
        "Loop pause does not survive a machine change",
        "Promoted to: session migration (Completed)",
    ] {
        assert!(texts.iter().any(|t| t == expected), "{expected}: {texts:?}");
    }
    assert_eq!(
        texts.iter().filter(|t| *t == "Promote").count(),
        2,
        "Promote on the open and the researched card, never the promoted"
    );
    assert_eq!(
        texts.iter().filter(|t| *t == "Research").count(),
        1,
        "Research on the open card only"
    );
    let ids = ids(&body);
    for expected in [
        SignalsComposition::PROMOTE,
        SignalsComposition::RESEARCH,
        SignalsComposition::SNOOZE,
        SignalsComposition::PROMOTE + 1,
        SignalsComposition::SNOOZE + 1,
    ] {
        assert!(
            ids.contains(&ElementId(expected)),
            "{expected:#x} is hittable"
        );
    }
    assert!(
        !ids.contains(&ElementId(SignalsComposition::RESEARCH + 1)),
        "a researched signal offers no Research"
    );
    assert!(
        !ids.contains(&ElementId(SignalsComposition::PROMOTE + 2)),
        "the promoted card has no verbs and takes no open index"
    );
}

/// Without a wall-clock reading the caps carry no age: the frame that
/// gave none draws the state and the source alone.
#[test]
fn a_clock_with_no_epoch_draws_no_age() {
    let body = SignalsComposition::el(&view(), Medium::Day, &FrameClock::new(0, Duration::ZERO));
    let texts = texts(&body);
    assert!(
        texts.iter().any(|t| t == "Open \u{b7} developer"),
        "{texts:?}"
    );
    assert!(texts.iter().any(|t| t == "Promoted"), "{texts:?}");
    assert!(!texts.iter().any(|t| t.ends_with(" d")), "{texts:?}");
}

/// The frame's wiring: each verb's id is the cockpit's own signal widget
/// at the signal's open index.
#[test]
fn the_ids_route_to_the_cockpits_signal_widgets() {
    let model = view();
    assert_eq!(
        RootComposition::target_of(&model, ElementId(SignalsComposition::PROMOTE + 1)),
        Some(Widget::signal_promote(1))
    );
    assert_eq!(
        RootComposition::target_of(&model, ElementId(SignalsComposition::RESEARCH)),
        Some(Widget::signal_research(0))
    );
    assert_eq!(
        RootComposition::target_of(&model, ElementId(SignalsComposition::SNOOZE + 1)),
        Some(Widget::signal_snooze(1))
    );
}

/// The root at the Signals place is the header and the inbox — no rail,
/// no foot, no composer — and the header's right slot is the count.
#[test]
fn the_root_routes_the_signals_place_to_the_inbox() {
    let model = view();
    let root = RootComposition::el(&model, Medium::Night, (8.0, 17.0), (1100.0, 640.0), &now());
    let texts = texts(&root);
    assert!(texts.iter().any(|t| t == "2 open"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "signals"), "the here stop");
    assert!(
        texts
            .iter()
            .any(|t| t == "Promoted to: session migration (Completed)"),
        "{texts:?}"
    );
    let ids = ids(&root);
    assert!(
        !ids.contains(&RootComposition::DIVIDER),
        "no rail on the inbox"
    );
    assert!(
        !ids.contains(&RootComposition::STAGE),
        "no turn on the inbox"
    );
}

/// The Sky's chrome carries the same count while there is one, so the
/// two rooms of altitude 1 cannot count apart.
#[test]
fn the_sky_chrome_carries_the_open_count() {
    let sky = view().with_place(Place::Sky);
    let counted = texts(&SkyComposition::el(&sky, Medium::Night));
    assert!(counted.iter().any(|t| t == "2 open"), "{counted:?}");
    let quiet = Fixture::mock("04", Size::new(120, 40)).expect("view 04 has a fixture");
    let silent = texts(&SkyComposition::el(&quiet, Medium::Night));
    assert!(
        !silent.iter().any(|t| t.ends_with(" open")),
        "no signals, no chip: {silent:?}"
    );
}
