use magistr_cockpit::cockpit::Size;

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

fn has(root: &Element, needle: &str) -> bool {
    texts(root).iter().any(|t| t.contains(needle))
}

/// View 05's foot: research and plan crossed with their checks, execute
/// live, verify ahead, and the running token total at the right edge.
#[test]
fn the_foot_crosses_passed_phases_lights_the_live_one_and_counts_tokens() {
    let model = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    let foot = FootComposition::el(&model, Medium::Night);
    assert!(has(&foot, "research \u{2713}"), "research is crossed");
    assert!(has(&foot, "plan \u{2713}"), "plan is crossed");
    assert!(has(&foot, "execute"), "execute is on the bar");
    assert!(
        !has(&foot, "execute \u{2713}"),
        "execute is live, not crossed"
    );
    assert!(has(&foot, "verify"), "verify is ahead");
    assert!(has(&foot, "38,204 tok"), "the capture's token total");
}

/// A fresh session's foot is quiet: every phase word ahead, none lit,
/// none crossed, and no token total claimed.
#[test]
fn a_fresh_foot_is_all_ahead_and_claims_no_totals() {
    let model = magistr_cockpit::cockpit::Model::new(Size::new(120, 40));
    let foot = FootComposition::el(&model, Medium::Night);
    for word in ["research", "plan", "execute", "verify"] {
        assert!(has(&foot, word), "{word} is on the bar");
    }
    assert!(!has(&foot, "\u{2713}"), "nothing is crossed");
    assert!(!has(&foot, "tok"), "no token total");
}

/// A decision screen carries the phase the decision is being made in and nothing else —
/// no run of phases, no running totals.
#[test]
fn a_deciding_foot_keeps_only_the_phase_the_decision_is_in() {
    let model = Fixture::mock("06", Size::new(120, 40)).expect("view 06 has a fixture");
    assert!(model.moment().is_some(), "view 06 is a Moment");
    let foot = FootComposition::el(&model, Medium::Night);
    assert!(!has(&foot, "tok"), "a decision screen carries no totals");
    let words = texts(&foot);
    assert!(words.len() <= 1, "one phase word at most, got {words:?}");
}
