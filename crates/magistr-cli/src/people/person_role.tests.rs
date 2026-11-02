use super::*;

const ALL_ROLES: [PersonRole; 5] = [
    PersonRole::Owner,
    PersonRole::Admin,
    PersonRole::Developer,
    PersonRole::Viewer,
    PersonRole::Ci,
];

#[test]
fn serde_lowercase_roundtrip() {
    for role in ALL_ROLES {
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, format!("\"{role}\""));
        let back: PersonRole = serde_json::from_str(&json).unwrap();
        assert_eq!(back, role);
    }
}

#[test]
fn from_str_accepts_case_insensitive_and_dev_alias() {
    assert_eq!("Owner".parse::<PersonRole>().unwrap(), PersonRole::Owner);
    assert_eq!("ADMIN".parse::<PersonRole>().unwrap(), PersonRole::Admin);
    assert_eq!("dev".parse::<PersonRole>().unwrap(), PersonRole::Developer);
    assert_eq!(
        " viewer ".parse::<PersonRole>().unwrap(),
        PersonRole::Viewer
    );
    assert_eq!("ci".parse::<PersonRole>().unwrap(), PersonRole::Ci);
}

#[test]
fn from_str_rejects_unknown_with_role_list() {
    let err = "bogus".parse::<PersonRole>().unwrap_err();
    assert!(err.contains("bogus"), "{err}");
    assert!(err.contains("owner, admin, developer, viewer, ci"), "{err}");
}

#[test]
fn owner_can_everything() {
    for cap in Capability::ALL {
        assert!(PersonRole::Owner.can(cap), "owner should have {cap:?}");
    }
}

#[test]
fn admin_can_everything_except_transfer_ownership() {
    for cap in Capability::ALL {
        let expected = cap != Capability::TransferOwnership;
        assert_eq!(PersonRole::Admin.can(cap), expected, "{cap:?}");
    }
}

#[test]
fn developer_matrix_is_build_and_view() {
    for cap in Capability::ALL {
        let expected = matches!(
            cap,
            Capability::EditPlan | Capability::RunSessions | Capability::View
        );
        assert_eq!(PersonRole::Developer.can(cap), expected, "{cap:?}");
    }
}

#[test]
fn viewer_can_only_view() {
    for cap in Capability::ALL {
        assert_eq!(
            PersonRole::Viewer.can(cap),
            cap == Capability::View,
            "{cap:?}"
        );
    }
}

#[test]
fn ci_runs_and_edits_plan_but_never_approves_or_manages() {
    assert!(PersonRole::Ci.can(Capability::RunSessions));
    assert!(PersonRole::Ci.can(Capability::EditPlan));
    assert!(PersonRole::Ci.can(Capability::View));
    assert!(!PersonRole::Ci.can(Capability::ApprovePermissions));
    assert!(!PersonRole::Ci.can(Capability::ManageTeam));
    assert!(!PersonRole::Ci.can(Capability::TransferOwnership));
}

#[test]
fn capability_labels_are_distinct() {
    let labels: std::collections::HashSet<_> = Capability::ALL.iter().map(|c| c.label()).collect();
    assert_eq!(labels.len(), Capability::ALL.len());
}
