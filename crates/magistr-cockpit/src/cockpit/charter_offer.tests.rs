use super::*;

#[test]
fn an_offer_reads_its_scope_and_record() {
    let offer = CharterOffer::new(
        "edit:crates/magistr-desktop",
        "Edit \u{b7} crates/magistr-desktop",
        14,
        14,
    );
    assert_eq!(offer.key(), "edit:crates/magistr-desktop");
    assert_eq!(offer.subject(), "Edit \u{b7} crates/magistr-desktop");
    assert_eq!(offer.tally(), "allowed 14 of 14");
}
