use super::*;

#[test]
fn a_tally_counts_allows_and_denies_apart() {
    let mut tally = ScopeTally::new(
        "tool:WebFetch",
        "WebFetch",
        PolicyCondition::tool("WebFetch"),
    );
    assert_eq!(tally.asked(), 0);
    assert!(!tally.is_clean(), "nothing answered is not a clean record");
    tally.note(true);
    tally.note(true);
    assert!(tally.is_clean());
    tally.note(false);
    assert_eq!((tally.allowed, tally.denied, tally.asked()), (2, 1, 3));
    assert!(!tally.is_clean(), "one deny spoils the record");
}

#[test]
fn a_tally_round_trips_with_its_condition() {
    let mut tally = ScopeTally::new(
        "edit:src",
        "Edit \u{b7} src",
        PolicyCondition::all_of(vec![
            PolicyCondition::tool("Edit"),
            PolicyCondition::path("src/*"),
        ]),
    );
    tally.note(true);
    let json = serde_json::to_string(&tally).expect("serialize");
    let back: ScopeTally = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, tally);
}
