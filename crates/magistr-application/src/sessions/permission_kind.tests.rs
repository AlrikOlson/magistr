use super::*;
use alloc::vec::Vec;

#[test]
fn every_kind_has_a_label_and_a_distinct_accelerator() {
    let labels: Vec<&str> = PermissionKind::ALL.iter().map(|k| k.label()).collect();
    assert_eq!(labels, ["Allow", "Always", "Deny", "Never"]);
    let keys: Vec<char> = PermissionKind::ALL
        .iter()
        .map(|k| k.accelerator())
        .collect();
    assert_eq!(keys, ['a', 'A', 'd', 'D']);
    for kind in PermissionKind::ALL {
        assert_eq!(
            PermissionKind::from_accelerator(kind.accelerator()),
            Some(kind)
        );
    }
    assert_eq!(PermissionKind::from_accelerator('q'), None);
    // Case is the whole distinction between "once" and "for the
    // session": `A` is not `a` held down harder, it is the shifted key
    // the card prints as ⇧A.
    assert_eq!(
        PermissionKind::from_accelerator('A'),
        Some(PermissionKind::AllowAlways)
    );
    let caps: Vec<&str> = PermissionKind::ALL.iter().map(|k| k.key_cap()).collect();
    assert_eq!(caps, ["A", "\u{21e7}A", "D", "\u{21e7}D"]);
}

#[test]
fn the_allow_kinds_allow_and_the_reject_kinds_do_not() {
    assert!(PermissionKind::AllowOnce.is_allow());
    assert!(PermissionKind::AllowAlways.is_allow());
    assert!(!PermissionKind::RejectOnce.is_allow());
    assert!(!PermissionKind::RejectAlways.is_allow());
}
