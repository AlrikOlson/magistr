use super::*;

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::medium::Medium;

fn mocks() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("mocks")
}

/// The 19 view ids are distinct, and every one of them has a capture
/// in both media at the size the stills are rendered at — so a capture
/// that goes missing or is re-taken at another viewport fails here, in
/// the required test gate, rather than as a size mismatch inside the
/// parity gate.
#[test]
fn every_view_has_both_captures_at_the_capture_size() {
    let ids: BTreeSet<&str> = Fixture::VIEWS.into_iter().collect();
    assert_eq!(ids.len(), Fixture::VIEWS.len(), "a view id repeats");
    for view in Fixture::VIEWS {
        for medium in Medium::ALL {
            let path = mocks().join(format!("{}-{view}.png", medium.prefix()));
            let capture = tiny_skia::Pixmap::load_png(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(
                (capture.width(), capture.height()),
                Fixture::CAPTURE,
                "{} is not at the capture size",
                path.display()
            );
        }
    }
}

/// The capture grid is as many whole cells as fit: on the 9x20 cell
/// the stills draw with, 125 columns by 32 rows.
#[test]
fn the_capture_grid_is_whole_cells() {
    assert_eq!(Fixture::capture_grid((9.0, 20.0)), Size::new(122, 32));
    // A cell wider than the capture still yields a grid.
    assert_eq!(Fixture::capture_grid((2000.0, 2000.0)), Size::new(1, 1));
}

/// What the last built frame says, in paint order.
fn said(frame: &crate::Frame) -> String {
    frame
        .says()
        .into_iter()
        .map(|(text, _)| text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn still_of(view: &str) -> (crate::Frame, Model) {
    let mut frame = crate::Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let mut model = Fixture::mock(view, size).expect("the view is built");
    let (width, height) = Fixture::CAPTURE;
    let clock = magistr_cockpit::cockpit::FrameClock::new(0, std::time::Duration::ZERO);
    frame
        .still(&mut model, &clock, width, height)
        .expect("the frame renders");
    (frame, model)
}

/// View 05 has a fixture and carries the capture's own data, which the
/// painted frame shows through the composition root; an unknown id
/// answers `None`, which the gate reports as SKIP.
#[test]
fn view_05_has_a_fixture_and_an_unknown_view_has_none() {
    let (frame, model) = still_of("05");
    assert_eq!(model.size().rows, 32);
    let text = said(&frame);
    assert!(text.contains("hit registry answers the same"), "{text}");
    assert!(text.contains("cockpit/surface.rs"), "{text}");
    // The card is collapsed in the composed feed and wears the capture's anatomy: the
    // head carries the verb, the path and the receipt's measure; the preview of what
    // changed and the receipt strip ride under it.
    assert!(text.contains("+47 \u{b7} receipt #241"), "{text}");
    assert!(text.contains("+ pub enum Surface"), "{text}");
    assert!(text.contains("inside cp-4\u{2019}s reach"), "{text}");
    assert!(
        text.contains("Steer"),
        "the composer is at the foot: {text}"
    );
    assert!(
        text.contains("Nearby work"),
        "the nearby work stays visible: {text}"
    );
    assert!(text.contains("38,204 tok"), "the foot counts: {text}");
    let size = model.size();
    assert!(Fixture::mock("99", size).is_none());
    assert!(Fixture::mock("F9", size).is_none());
}

/// View 06 is the Moment: the plan card the capture floats, its checkpoint receipt, the
/// numbered steps — and none of view 05's supervision chrome, because third altitude is
/// one decision and nothing else.
#[test]
fn view_06_is_the_moment_and_the_session_chrome_stands_down() {
    let (frame, model) = still_of("06");
    assert!(model.moment().is_some(), "a plan ask is on screen");
    let text = said(&frame);
    assert!(text.contains("plan \u{b7} two-phase"), "{text}");
    assert!(text.contains("Re-bless 12 golden screenshots"), "{text}");
    assert!(
        text.contains("1. Render the 6 surfaces in both themes"),
        "{text}"
    );
    // The session is dimmed behind it, never blanked.
    assert!(text.contains("The vocabulary is 48 static ids"), "{text}");
    // The draft remains in its place behind the modal decision.
    assert!(!text.contains("Checkpoints"), "{text}");
    assert!(!text.contains("Charter"), "{text}");
    assert!(text.contains("Steer"), "{text}");
}

/// View 13 is the search: the float over the dimmed session — the
/// chord, the query, the count the matcher answers, the scope chips
/// and the three kind caps — with every piece of altitude 2's chrome
/// gone, because search is a decision screen too.
#[test]
fn view_13_is_the_search_over_the_dimmed_session_with_the_chrome_gone() {
    let (frame, model) = still_of("13");
    let search = model.search().expect("⌘⇧F is up");
    assert_eq!(search.query(), "contrast floor");
    let text = said(&frame);
    assert!(text.contains("6 results"), "{text}");
    for word in ["everything", "reports", "receipts", "trace", "sessions"] {
        assert!(text.contains(word), "the {word} chip: {text}");
    }
    assert!(
        text.contains("Every light palette clears its floor"),
        "{text}"
    );
    assert!(text.contains("Step 387"), "{text}");
    assert!(text.contains("theme/ink.rs"), "{text}");
    // The session is dimmed behind it, never blanked.
    assert!(text.contains("The vocabulary is 48 static ids"), "{text}");
    // And the chrome is gone: no path, no rail, no composer, no foot.
    assert!(!text.contains("projects"), "{text}");
    assert!(!text.contains("Checkpoints"), "{text}");
    assert!(!text.contains("Steer"), "{text}");
    assert!(!text.contains("tok"), "{text}");
}

/// View 07 is the review: it replaces the session under the same
/// altitude path — the scope, the three files with their verdicts, the
/// selected diff and the agent's one-line why, with the answer verbs.
#[test]
fn view_07_is_the_review_and_every_part_of_it_is_state_the_cockpit_keeps() {
    let (frame, model) = still_of("07");
    // Derived, never asserted: the scope is the spine's own answer.
    assert_eq!(model.review_since(), Some("cp-3"));
    // `Frame::build` is what declares the window, and the review is a
    // native surface — so the predicate is asked of the model the frame
    // painted rather than of the one the fixture handed over.
    assert!(model.review().is_some(), "an edit ask is on screen");
    let text = said(&frame);
    assert!(text.contains("Review"), "{text}");
    for file in ["ink.rs", "css.rs", "css.tests.rs"] {
        assert!(text.contains(file), "{file} missing in {text}");
    }
    assert!(text.contains(", palette: &Palette"), "the diff: {text}");
    assert!(
        text.contains("the consumer is protected"),
        "the note: {text}"
    );
    assert!(text.contains("projects"), "the path stays: {text}");
    // A review is not a Moment: no plan card floats over it.
    assert!(model.moment().is_none(), "an edit ask raises no plan card");
}

/// View 05's model is in the phase the volume's foot lights.
///
/// The capture crosses `research` and `plan` and lights `execute`, and that only holds
/// if the iteration is started BEFORE the phase frame arrives: `LoopEvent::Started`
/// begins a fresh iteration, and a fresh iteration has no phase yet — so a fixture that
/// announced the phase first had it wiped a message later and drew a foot that said
/// nothing. The real order is the loop's too: the driver starts the iteration, then the
/// agent calls `magistr_set_phase`.
#[test]
fn view_05_holds_the_phase_the_volumes_foot_lights() {
    let model = Fixture::mock("05", Size::new(125, 32)).expect("view 05 has a fixture");
    assert_eq!(
        model.phase(),
        Some(magistr_cockpit::cockpit::DisplayPhase::Execute),
        "the capture lights `execute`"
    );
    assert!(
        model.loop_status().is_some(),
        "and the iteration cap is still `AGENT · ITERATION 4 / 12`"
    );
}

/// View 10 is the ⌘, surface: the Appearance pane with its three
/// theme cards, both switches, the density segments and the source
/// row, beside the sidebar's three scopes — and nothing of the
/// session behind it.
#[test]
fn view_10_is_the_settings_surface_alone() {
    let (frame, model) = still_of("10");
    let settings = model.settings().expect("⌘, is up");
    assert_eq!(
        settings.group(),
        magistr_cockpit::cockpit::SettingsGroup::Appearance
    );
    let text = said(&frame);
    for word in [
        "settings",
        "esc \u{b7} Close",
        "Search settings",
        "This app",
        "This project",
        "This team \u{b7} 4 people",
        "~/Code/magistr",
        "read-only",
        "Appearance",
        "The sky",
        "The chart",
        "Follow OS",
        "Show activity heatmap",
        "Texture",
        "Comfortable",
        "Compact",
        "~/.config/magistr/config.toml",
        "Open file",
    ] {
        assert!(text.contains(word), "{word}: {text}");
    }
    assert!(
        !text.contains("research"),
        "no foot under a surface: {text}"
    );
}

#[test]
fn view_14_is_the_bad_day_over_the_faded_sky() {
    let (frame, model) = still_of("14");
    let day = model.bad_day().expect("three faults");
    assert_eq!(day.faults().len(), 3);
    assert_eq!(day.blocking(), 2);
    // The stillest screen in the app: no motion — the loop's own clock
    // is the one animation left, and it is the driver's to stop.
    assert!(
        !model.animates(magistr_cockpit::cockpit::Animation::Motion),
        "no motion under a bad day"
    );
    let text = said(&frame);
    for word in [
        "2 blocking",
        "Agent binary not found",
        "Install agent\u{2026}",
        "Limit \u{b7} 1 h",
        "Checkpoint cp-6 saved",
        "Connection \u{b7} 3 min",
        "Reconnect",
        "Requires agent",
    ] {
        assert!(text.contains(word), "{word}: {text}");
    }
}
