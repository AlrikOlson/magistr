use super::*;
use crate::team::{RosterChange, TeamRole};

const ALL_ROLES: [TeamRole; 5] = [
    TeamRole::Owner,
    TeamRole::Admin,
    TeamRole::Developer,
    TeamRole::Viewer,
    TeamRole::Ci,
];

/// Every change that leaves Owner alone.
const NON_OWNER_CHANGES: [RosterChange; 3] = [
    RosterChange::Add(TeamRole::Developer),
    RosterChange::Remove(TeamRole::Viewer),
    RosterChange::SetRole {
        from: TeamRole::Viewer,
        to: TeamRole::Admin,
    },
];

#[test]
fn owner_may_make_any_change() {
    for change in NON_OWNER_CHANGES {
        assert!(roster_access(TeamRole::Owner, change), "{change:?}");
    }
    assert!(roster_access(
        TeamRole::Owner,
        RosterChange::Add(TeamRole::Owner)
    ));
    assert!(roster_access(
        TeamRole::Owner,
        RosterChange::SetRole {
            from: TeamRole::Owner,
            to: TeamRole::Admin
        }
    ));
}

#[test]
fn admin_edits_the_roster_but_never_touches_owner() {
    for change in NON_OWNER_CHANGES {
        assert!(roster_access(TeamRole::Admin, change), "{change:?}");
    }
    // The self-promotion an unenforced roster would have allowed.
    assert!(!roster_access(
        TeamRole::Admin,
        RosterChange::SetRole {
            from: TeamRole::Admin,
            to: TeamRole::Owner
        }
    ));
    assert!(!roster_access(
        TeamRole::Admin,
        RosterChange::Add(TeamRole::Owner)
    ));
    assert!(!roster_access(
        TeamRole::Admin,
        RosterChange::Remove(TeamRole::Owner)
    ));
    assert!(!roster_access(
        TeamRole::Admin,
        RosterChange::SetRole {
            from: TeamRole::Owner,
            to: TeamRole::Viewer
        }
    ));
}

#[test]
fn developer_viewer_and_ci_may_not_edit_the_roster_at_all() {
    for role in [TeamRole::Developer, TeamRole::Viewer, TeamRole::Ci] {
        for change in NON_OWNER_CHANGES {
            assert!(!roster_access(role, change), "{role} {change:?}");
        }
        assert!(
            !roster_access(role, RosterChange::Add(TeamRole::Owner)),
            "{role}"
        );
    }
}

#[test]
fn only_owner_holds_the_ownership_transfer_right() {
    let promote = RosterChange::SetRole {
        from: TeamRole::Developer,
        to: TeamRole::Owner,
    };
    let allowed: Vec<TeamRole> = ALL_ROLES
        .into_iter()
        .filter(|r| roster_access(*r, promote))
        .collect();
    assert_eq!(allowed, vec![TeamRole::Owner]);
}

#[test]
fn touches_owner_only_when_owner_is_on_either_side() {
    assert!(RosterChange::Add(TeamRole::Owner).touches_owner());
    assert!(RosterChange::Remove(TeamRole::Owner).touches_owner());
    assert!(
        RosterChange::SetRole {
            from: TeamRole::Owner,
            to: TeamRole::Ci
        }
        .touches_owner()
    );
    assert!(
        RosterChange::SetRole {
            from: TeamRole::Ci,
            to: TeamRole::Owner
        }
        .touches_owner()
    );
    assert!(!RosterChange::Add(TeamRole::Admin).touches_owner());
    assert!(
        !RosterChange::SetRole {
            from: TeamRole::Viewer,
            to: TeamRole::Admin
        }
        .touches_owner()
    );
}
