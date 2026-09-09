use super::*;

#[test]
fn a_rule_keeps_its_subject_and_mode() {
    let rule = CharterRule::new("Edit · magistr-tui", RuleMode::Ask);
    assert_eq!(rule.subject(), "Edit · magistr-tui");
    assert_eq!(rule.mode(), RuleMode::Ask);
}
