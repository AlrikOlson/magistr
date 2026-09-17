use magistr_cockpit::cockpit::{AgentMissing, Model, Size, Surface, Widget};

use super::*;
use crate::comp_root::RootComposition;
use crate::element::Kind;
use crate::fixture::Fixture;

const FIELD: f32 = 588.0;

fn view_14() -> Model {
    Fixture::mock("14", Size::new(120, 40)).expect("view 14 has a fixture")
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

fn has(root: &Element, needle: &str) -> bool {
    texts(root).iter().any(|t| t.contains(needle))
}

fn ids(root: &Element) -> Vec<ElementId> {
    let mut ids = Vec::new();
    root.each(&mut |_, element| ids.extend(element.id));
    ids
}

#[test]
fn a_good_day_composes_nothing() {
    let mut model = Model::new(Size::new(120, 40));
    model.set_surface(Surface::Native);
    assert!(BadDayComposition::el(&model, Medium::Night, FIELD).is_none());
    assert!(BadDayComposition::chips(&model, Medium::Night).is_none());
}

#[test]
fn view_14_says_every_word_of_the_capture() {
    let model = view_14();
    let float = BadDayComposition::el(&model, Medium::Night, FIELD).expect("a bad day");
    for needle in [
        "Blocking \u{b7} New sessions cannot start \u{b7} Running sessions continue",
        "Agent binary not found",
        "claude is not on PATH. Last detected 09:14.",
        "Install agent\u{2026}",
        "Details",
        "Limit \u{b7} 1 h",
        "Session paused at the 10.00 USD limit. Checkpoint cp-6 saved.",
        "Increase limit",
        "View receipts",
        "Connection \u{b7} 3 min",
        "Connection to tidewatch-server lost. The session continues on the server.",
        "Reconnect",
        "e6b",
        "Blocked",
        "Requires agent",
    ] {
        assert!(has(&float, needle), "missing {needle:?}");
    }
    let chip = BadDayComposition::chips(&model, Medium::Night).expect("a count");
    assert!(has(&chip, "2 blocking"));
}

#[test]
fn the_verbs_mint_one_id_each_in_deal_order_and_route_to_their_widgets() {
    let model = view_14();
    let float = BadDayComposition::el(&model, Medium::Night, FIELD).expect("a bad day");
    let minted: Vec<ElementId> = ids(&float)
        .into_iter()
        .filter(|id| (BadDayComposition::VERBS..BadDayComposition::VERBS_END).contains(&id.0))
        .collect();
    assert_eq!(minted.len(), 5, "two, two and one");
    let widgets: Vec<Widget> = minted
        .iter()
        .map(|id| {
            Widget::from_id(RootComposition::target_of(&model, *id).expect("routed"))
                .expect("a named widget")
        })
        .collect();
    assert_eq!(
        widgets,
        vec![
            Widget::FaultInstall,
            Widget::FaultDetails,
            Widget::FaultIncreaseLimit,
            Widget::FaultReceipts,
            Widget::FaultReconnect,
        ]
    );
    assert!(
        RootComposition::target_of(&model, ElementId(BadDayComposition::VERBS + 5)).is_none(),
        "no sixth verb"
    );
}

#[test]
fn the_column_seats_at_forty_seven_percent_of_the_field() {
    let model = view_14();
    let float = BadDayComposition::el(&model, Medium::Night, FIELD).expect("a bad day");
    let mut bottoms = Vec::new();
    float.each(&mut |_, element| {
        if let Some(px) = element.pin[3] {
            bottoms.push(px);
        }
    });
    // The seat's bottom pin is 6% of the field; the foot's is its lift.
    assert!(bottoms.iter().any(|b| (b - FIELD * 0.06).abs() < 0.01));
    assert!(bottoms.contains(&22.0));
}

#[test]
fn a_card_without_a_title_is_one_row_and_a_card_with_one_stacks() {
    let mut model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_agent_missing(AgentMissing::new("claude"));
    model.set_surface(Surface::Native);
    let float = BadDayComposition::el(&model, Medium::Day, FIELD).expect("a bad day");
    assert!(has(&float, "Agent binary not found"));
    assert!(!has(&float, "Last detected"), "no stamp, no clause");
    assert!(BadDayComposition::chips(&model, Medium::Day).is_some_and(|c| has(&c, "1 blocking")));
}
