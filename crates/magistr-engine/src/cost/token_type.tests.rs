use super::*;

#[test]
fn attribute_values_are_the_two_the_convention_allows() {
    assert_eq!(TokenType::Input.as_str(), "input");
    assert_eq!(TokenType::Output.as_str(), "output");
    assert_eq!(TokenType::ALL.len(), 2);
}

#[test]
fn serializes_as_the_attribute_value() {
    let json = serde_json::to_string(&TokenType::Output).expect("serializes");
    assert_eq!(json, "\"output\"");
}

#[test]
fn display_matches_as_str() {
    for kind in TokenType::ALL {
        assert_eq!(kind.to_string(), kind.as_str());
    }
}
