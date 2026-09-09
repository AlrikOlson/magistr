use super::*;
use crate::cockpit::Widget;

/// The card keeps its cap in the case it was written in: the `Cap`
/// voice is what uppercases it on a window, so a string that shouted
/// would shout twice there and once on a surface with no voices.
#[test]
fn the_cap_is_kept_in_the_case_it_was_written_in() {
    let card = BriefCard::new(
        Severity::Alert,
        "Stuck · 4 h",
        BriefBody::Act {
            text: "e6b requires a decision.".to_owned(),
            verb: "Answer",
            widget: Widget::BriefAnswer,
        },
    );
    assert_eq!(card.cap(), "Stuck · 4 h");
    assert_eq!(card.severity(), Severity::Alert);
}

/// Cards sort by their band alone — the whole deal is
/// `sort_by_key(BriefCard::severity)` and there is no second table to
/// keep in step with `Severity::ALL`.
#[test]
fn cards_sort_by_their_band_alone() {
    let body = || BriefBody::Measure {
        readout: "0.00 / 1.00".to_owned(),
        fill: 0,
        tick: None,
    };
    let mut cards: Vec<BriefCard> = [Severity::Quiet, Severity::Alert, Severity::Good]
        .into_iter()
        .map(|band| BriefCard::new(band, band.label(), body()))
        .collect();
    cards.sort_by_key(BriefCard::severity);
    let bands: Vec<Severity> = cards.iter().map(BriefCard::severity).collect();
    assert_eq!(
        bands,
        vec![Severity::Alert, Severity::Good, Severity::Quiet]
    );
}
