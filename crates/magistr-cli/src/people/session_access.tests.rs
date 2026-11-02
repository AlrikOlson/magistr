use super::*;
use crate::people::{PersonRole, SessionAction};

const ALL_ROLES: [PersonRole; 5] = [
    PersonRole::Owner,
    PersonRole::Admin,
    PersonRole::Developer,
    PersonRole::Viewer,
    PersonRole::Ci,
];

#[test]
fn every_role_may_view_any_session() {
    for role in ALL_ROLES {
        assert!(session_access(role, SessionAction::View, false), "{role}");
        assert!(session_access(role, SessionAction::View, true), "{role}");
    }
}

#[test]
fn developer_and_ci_attach_only_their_own_sessions() {
    for role in [PersonRole::Developer, PersonRole::Ci] {
        assert!(session_access(role, SessionAction::Attach, true), "{role}");
        assert!(
            !session_access(role, SessionAction::Attach, false),
            "{role}"
        );
    }
}

#[test]
fn admin_and_owner_attach_any_session() {
    for role in [PersonRole::Owner, PersonRole::Admin] {
        assert!(session_access(role, SessionAction::Attach, false), "{role}");
        assert!(session_access(role, SessionAction::Attach, true), "{role}");
    }
}

#[test]
fn viewer_never_attaches() {
    assert!(!session_access(
        PersonRole::Viewer,
        SessionAction::Attach,
        true
    ));
    assert!(!session_access(
        PersonRole::Viewer,
        SessionAction::Attach,
        false
    ));
}

#[test]
fn take_over_needs_ownership_or_admin() {
    // Admin+ may take over anyone's session.
    for role in [PersonRole::Owner, PersonRole::Admin] {
        assert!(
            session_access(role, SessionAction::TakeOver, false),
            "{role}"
        );
    }
    // Developer/Ci/Viewer may only hand off sessions they own.
    for role in [PersonRole::Developer, PersonRole::Ci, PersonRole::Viewer] {
        assert!(
            session_access(role, SessionAction::TakeOver, true),
            "{role}"
        );
        assert!(
            !session_access(role, SessionAction::TakeOver, false),
            "{role}"
        );
    }
}
