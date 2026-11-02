use super::*;
use crate::people::{PersonRole, RosterChange};

const ALL_ROLES: [PersonRole; 5] = [
    PersonRole::Owner,
    PersonRole::Admin,
    PersonRole::Developer,
    PersonRole::Viewer,
    PersonRole::Ci,
];

/// Every change that leaves Owner alone.
const NON_OWNER_CHANGES: [RosterChange; 3] = [
    RosterChange::Add(PersonRole::Developer),
    RosterChange::Remove(PersonRole::Viewer),
    RosterChange::SetRole {
        from: PersonRole::Viewer,
        to: PersonRole::Admin,
    },
];

#[test]
fn owner_may_make_any_change() {
    for change in NON_OWNER_CHANGES {
        assert!(roster_access(PersonRole::Owner, change), "{change:?}");
    }
    assert!(roster_access(
        PersonRole::Owner,
        RosterChange::Add(PersonRole::Owner)
    ));
    assert!(roster_access(
        PersonRole::Owner,
        RosterChange::SetRole {
            from: PersonRole::Owner,
            to: PersonRole::Admin
        }
    ));
}

#[test]
fn admin_edits_the_roster_but_never_touches_owner() {
    for change in NON_OWNER_CHANGES {
        assert!(roster_access(PersonRole::Admin, change), "{change:?}");
    }
    // The self-promotion an unenforced roster would have allowed.
    assert!(!roster_access(
        PersonRole::Admin,
        RosterChange::SetRole {
            from: PersonRole::Admin,
            to: PersonRole::Owner
        }
    ));
    assert!(!roster_access(
        PersonRole::Admin,
        RosterChange::Add(PersonRole::Owner)
    ));
    assert!(!roster_access(
        PersonRole::Admin,
        RosterChange::Remove(PersonRole::Owner)
    ));
    assert!(!roster_access(
        PersonRole::Admin,
        RosterChange::SetRole {
            from: PersonRole::Owner,
            to: PersonRole::Viewer
        }
    ));
}

#[test]
fn developer_viewer_and_ci_may_not_edit_the_roster_at_all() {
    for role in [PersonRole::Developer, PersonRole::Viewer, PersonRole::Ci] {
        for change in NON_OWNER_CHANGES {
            assert!(!roster_access(role, change), "{role} {change:?}");
        }
        assert!(
            !roster_access(role, RosterChange::Add(PersonRole::Owner)),
            "{role}"
        );
    }
}

#[test]
fn only_owner_holds_the_ownership_transfer_right() {
    let promote = RosterChange::SetRole {
        from: PersonRole::Developer,
        to: PersonRole::Owner,
    };
    let allowed: Vec<PersonRole> = ALL_ROLES
        .into_iter()
        .filter(|r| roster_access(*r, promote))
        .collect();
    assert_eq!(allowed, vec![PersonRole::Owner]);
}

#[test]
fn touches_owner_only_when_owner_is_on_either_side() {
    assert!(RosterChange::Add(PersonRole::Owner).touches_owner());
    assert!(RosterChange::Remove(PersonRole::Owner).touches_owner());
    assert!(
        RosterChange::SetRole {
            from: PersonRole::Owner,
            to: PersonRole::Ci
        }
        .touches_owner()
    );
    assert!(
        RosterChange::SetRole {
            from: PersonRole::Ci,
            to: PersonRole::Owner
        }
        .touches_owner()
    );
    assert!(!RosterChange::Add(PersonRole::Admin).touches_owner());
    assert!(
        !RosterChange::SetRole {
            from: PersonRole::Viewer,
            to: PersonRole::Admin
        }
        .touches_owner()
    );
}
