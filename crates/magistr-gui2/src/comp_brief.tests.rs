use magistr_cockpit::cockpit::{Model, Motion, Msg, Size, StageTab, Surface, update};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::harness::fixtures;
use magistr_cockpit::input::{KeyCode, KeyEvent};

use super::*;
use crate::element::Kind;

/// A window with news to report, opened after six hours away — the
/// cockpit's own Brief conditions, folded through the real `update`.
fn briefed() -> Model {
    let mut model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_tab(StageTab::Roadmap);
    update(
        &mut model,
        Msg::Engine(EngineEvent::local(fixtures::frame(
            "roadmap",
            "chunk_completed",
            serde_json::json!({
                "chunk": {"id": "e10-fleet", "name": "e10-fleet", "status": "done"}
            }),
        ))),
    );
    model.with_away(std::time::Duration::from_secs(6 * 3600 + 12 * 60))
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

/// The composition exists exactly when the model's Brief does: a quiet
/// project answers nothing, news answers the overlay, and Esc through
/// the one `update` puts it away.
#[test]
fn the_brief_rises_with_news_and_esc_puts_it_away() {
    let quiet = Model::new(Size::new(120, 40));
    assert!(BriefComposition::el(&quiet, Medium::Night).is_none());
    let mut model = briefed();
    let brief = model.brief().expect("there is news");
    let overlay =
        BriefComposition::el(&model, Medium::Night).expect("the composition follows the model");
    let texts = texts(&overlay);
    assert!(texts.iter().any(|t| t == "Summary"));
    assert!(
        texts.iter().any(|t| t == "away 6 h 12 m"),
        "the away is the Brief's own reading"
    );
    drop(brief);
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Esc)));
    assert!(
        BriefComposition::el(&model, Medium::Night).is_none(),
        "esc dismissed it through the one update"
    );
}

/// The deal is the model's clock: cards the deal has not reached hold
/// ghosts, and ticking the model through `update` lands them — no tick
/// counting lives in the composition.
#[test]
fn the_deal_lands_the_cards_through_ticks() {
    let mut model = briefed();
    let cards = model.brief().expect("news").cards().len();
    let overlay = BriefComposition::el(&model, Medium::Night).expect("the overlay");
    assert!(
        texts(&overlay).iter().any(|t| t == "dealing\u{2026}"),
        "at the raise the deal has not reached every card"
    );
    let deal = Motion::RISE_STAGGER * cards as u64 + Motion::ARRIVAL_STEPS + 1;
    for _ in 0..deal {
        update(&mut model, Msg::Tick);
    }
    assert!(!model.brief_dealing(), "the deal is over");
    let landed = BriefComposition::el(&model, Medium::Night).expect("still up");
    let texts = texts(&landed);
    assert!(
        !texts.iter().any(|t| t == "dealing\u{2026}"),
        "every card has landed"
    );
    for card in model.brief().expect("news").cards() {
        assert!(
            texts.iter().any(|t| t == card.cap()),
            "{} is on its card",
            card.cap()
        );
    }
}
