use super::*;

fn bash() -> Permission {
    Permission::new(
        7,
        "Bash",
        vec![
            PermissionChoice::new("reject", PermissionKind::RejectOnce),
            PermissionChoice::new("allow", PermissionKind::AllowOnce),
            PermissionChoice::new("allow_always", PermissionKind::AllowAlways),
        ],
    )
}

#[test]
fn choices_answer_by_kind_and_the_buttons_come_in_strip_order() {
    let p = bash();
    assert_eq!(p.id(), 7);
    assert_eq!(p.title(), "Bash");
    assert_eq!(p.choice(PermissionKind::AllowOnce), Some("allow"));
    assert_eq!(p.choice(PermissionKind::AllowAlways), Some("allow_always"));
    assert_eq!(p.choice(PermissionKind::RejectOnce), Some("reject"));
    assert_eq!(p.choice(PermissionKind::RejectAlways), None);
    let buttons: Vec<(PermissionKind, &str)> = p.buttons().collect();
    assert_eq!(
        buttons,
        [
            (PermissionKind::AllowOnce, "allow"),
            (PermissionKind::AllowAlways, "allow_always"),
            (PermissionKind::RejectOnce, "reject"),
        ]
    );
}

#[test]
fn allow_and_deny_fall_back_to_the_session_wide_kinds() {
    let p = bash();
    assert_eq!(p.allow(), Some("allow"));
    assert_eq!(p.deny(), Some("reject"));
    let only_always = Permission::new(
        1,
        "Edit",
        vec![
            PermissionChoice::new("never", PermissionKind::RejectAlways),
            PermissionChoice::new("always", PermissionKind::AllowAlways),
        ],
    );
    assert_eq!(only_always.allow(), Some("always"));
    assert_eq!(only_always.deny(), Some("never"));
    let none = Permission::new(2, "Read", Vec::new());
    assert_eq!(none.allow(), None);
    assert_eq!(none.deny(), None);
    assert_eq!(none.buttons().count(), 0);
}
