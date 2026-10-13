use super::*;

#[test]
fn the_offer_reads_its_record_back() {
    let offer = DelegationOffer::new(
        "edit:crates/magistr-desktop",
        "Edit \u{b7} crates/magistr-desktop",
        14,
        14,
    );
    assert_eq!(
        offer.line(),
        "Edit \u{b7} crates/magistr-desktop \u{b7} allowed 14 of 14 \u{2014} delegate?"
    );
}
