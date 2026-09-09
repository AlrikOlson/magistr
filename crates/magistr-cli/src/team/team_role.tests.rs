use super::*;

const ALL_ROLES: [TeamRole; 5] = [
    TeamRole::Owner,
    TeamRole::Admin,
    TeamRole::Developer,
    TeamRole::Viewer,
    TeamRole::Ci,
];

#[test]
fn serde_lowercase_roundtrip() {
    for role in ALL_ROLES {
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, format!("\"{role}\""));
        let back: TeamRole = serde_json::from_str(&json).unwrap();
        assert_eq!(back, role);
    }
}

#[test]
fn from_str_accepts_case_insensitive_and_dev_alias() {
    assert_eq!("Owner".parse::<TeamRole>().unwrap(), TeamRole::Owner);
    assert_eq!("ADMIN".parse::<TeamRole>().unwrap(), TeamRole::Admin);
    assert_eq!("dev".parse::<TeamRole>().unwrap(), TeamRole::Developer);
    assert_eq!(" viewer ".parse::<TeamRole>().unwrap(), TeamRole::Viewer);
    assert_eq!("ci".parse::<TeamRole>().unwrap(), TeamRole::Ci);
}

#[test]
fn from_str_rejects_unknown_with_role_list() {
    let err = "bogus".parse::<TeamRole>().unwrap_err();
    assert!(err.contains("bogus"), "{err}");
    assert!(err.contains("owner, admin, developer, viewer, ci"), "{err}");
}

#[test]
fn owner_can_everything() {
    for cap in Capability::ALL {
        assert!(TeamRole::Owner.can(cap), "owner should have {cap:?}");
    }
}

#[test]
fn admin_can_everything_except_transfer_ownership() {
    for cap in Capability::ALL {
        let expected = cap != Capability::TransferOwnership;
        assert_eq!(TeamRole::Admin.can(cap), expected, "{cap:?}");
    }
}

#[test]
fn developer_matrix_is_build_and_view() {
    for cap in Capability::ALL {
        let expected = matches!(
            cap,
            Capability::EditRoadmap | Capability::RunSessions | Capability::View
        );
        assert_eq!(TeamRole::Developer.can(cap), expected, "{cap:?}");
    }
}

#[test]
fn viewer_can_only_view() {
    for cap in Capability::ALL {
        assert_eq!(
            TeamRole::Viewer.can(cap),
            cap == Capability::View,
            "{cap:?}"
        );
    }
}

#[test]
fn ci_runs_and_edits_roadmap_but_never_approves_or_manages() {
    assert!(TeamRole::Ci.can(Capability::RunSessions));
    assert!(TeamRole::Ci.can(Capability::EditRoadmap));
    assert!(TeamRole::Ci.can(Capability::View));
    assert!(!TeamRole::Ci.can(Capability::ApprovePermissions));
    assert!(!TeamRole::Ci.can(Capability::ManageTeam));
    assert!(!TeamRole::Ci.can(Capability::TransferOwnership));
}

#[test]
fn capability_labels_are_distinct() {
    let labels: std::collections::HashSet<_> = Capability::ALL.iter().map(|c| c.label()).collect();
    assert_eq!(labels.len(), Capability::ALL.len());
}
