use super::*;
use crate::cockpit::Widget;

#[test]
fn the_age_is_the_coarsest_single_unit() {
    assert_eq!(Fault::age_text(Duration::from_secs(12)), "12 s");
    assert_eq!(Fault::age_text(Duration::from_secs(3 * 60 + 40)), "3 min");
    assert_eq!(Fault::age_text(Duration::from_secs(3600 + 59 * 60)), "1 h");
    assert_eq!(
        Fault::age_text(Duration::from_secs(2 * 86_400 + 5 * 3600)),
        "2 d"
    );
}

#[test]
fn the_cap_joins_the_age_only_when_one_is_known() {
    let fault = Fault::new(Severity::Attention, "Limit", "Session paused.");
    assert_eq!(fault.cap(), "Limit");
    let aged = fault.aged(Duration::from_secs(3600));
    assert_eq!(aged.cap(), "Limit \u{b7} 1 h");
    assert_eq!(aged.age(), Some(Duration::from_secs(3600)));
}

#[test]
fn the_builders_carry_every_face_of_the_card() {
    let fault = Fault::new(Severity::Alert, "Blocking", "It broke.")
        .titled("Agent binary not found")
        .detailed("the receipt")
        .blocking()
        .verb(FaultVerb::lit("Install", Widget::FaultInstall))
        .verb(FaultVerb::new("Details", Widget::FaultDetails))
        .ringing("e6b", "Requires agent");
    assert_eq!(fault.severity(), Severity::Alert);
    assert_eq!(fault.title(), Some("Agent binary not found"));
    assert_eq!(fault.text(), "It broke.");
    assert_eq!(fault.detail(), Some("the receipt"));
    assert!(fault.blocks());
    assert_eq!(fault.star(), Some(("e6b", "Requires agent")));
    let verbs = fault.verbs();
    assert_eq!(verbs.len(), 2);
    assert!(verbs[0].is_lit(), "the fixing verb is the lit exit");
    assert_eq!(verbs[0].verb(), "Install");
    assert_eq!(verbs[0].widget(), Widget::FaultInstall);
    assert!(!verbs[1].is_lit(), "the telling verb is neutral");
}

#[test]
fn a_plain_fault_neither_blocks_nor_rings() {
    let fault = Fault::new(Severity::Attention, "Connection", "Lost.");
    assert!(!fault.blocks());
    assert_eq!(fault.star(), None);
    assert_eq!(fault.title(), None);
    assert_eq!(fault.detail(), None);
    assert!(fault.verbs().is_empty());
}
