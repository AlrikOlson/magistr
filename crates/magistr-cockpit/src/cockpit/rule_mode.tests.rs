use super::*;

#[test]
fn the_badge_words_are_the_captures() {
    assert_eq!(RuleMode::Ask.label(), "ASK");
    assert_eq!(RuleMode::Auto.label(), "AUTO");
    assert_eq!(RuleMode::Off.label(), "OFF");
}
