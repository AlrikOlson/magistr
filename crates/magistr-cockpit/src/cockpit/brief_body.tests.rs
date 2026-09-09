use super::*;

/// An acting body offers its verb and the id that verb clicks; a
/// measurement offers neither, which is what keeps the Brief to one lit
/// exit however many cards it deals.
#[test]
fn only_an_acting_body_offers_a_verb() {
    let act = BriefBody::Act {
        text: "e6b requires a decision: OTLP endpoint scope.".to_owned(),
        verb: "Answer",
        widget: Widget::BriefAnswer,
    };
    assert_eq!(act.verb(), Some(("Answer", Widget::BriefAnswer)));
    let measure = BriefBody::Measure {
        readout: "3.61 / 12.00 USD".to_owned(),
        fill: 300,
        tick: None,
    };
    assert_eq!(measure.verb(), None);
}
