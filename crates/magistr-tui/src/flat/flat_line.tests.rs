use super::*;

#[test]
fn a_role_leads_and_a_continuation_indents() {
    assert_eq!(
        FlatLine::settled(Some(Role::Agent), "Hello there").render(),
        "agent: Hello there"
    );
    assert_eq!(
        FlatLine::settled(None, "-  let x = 1;").render(),
        "  -  let x = 1;"
    );
}

#[test]
fn a_blank_line_is_blank_rather_than_two_spaces() {
    assert_eq!(FlatLine::settled(None, "").render(), "");
    assert_eq!(FlatLine::settled(Some(Role::Note), "").render(), "note:");
}

#[test]
fn a_line_is_unsettled_only_when_it_is_made_so() {
    let open = FlatLine::new(Some(Role::Agent), "Hello ther", false);
    assert!(!open.is_settled());
    assert_eq!(open.role(), Some(Role::Agent));
    assert_eq!(open.text(), "Hello ther");
    assert!(FlatLine::settled(Some(Role::Agent), "Hello there").is_settled());
}
