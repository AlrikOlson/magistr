use super::*;
#[test]
fn family_names_and_stamps_keep_the_wire_contract() {
    for family in Family::ALL {
        assert_eq!(Family::parse(family.as_str()), Some(family));
        assert_eq!(serde_json::to_value(family).unwrap(), family.as_str());
    }
    assert_eq!(Family::parse("unknown"), None);
    assert_eq!((EPOCH_KEY, SEQ_KEY), ("epoch", "seq"));
}
