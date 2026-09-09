use super::*;

use magistr_engine::policy::GovernancePolicy;

use crate::team::directory::{DirectoryConfig, GroupRole, ScimEffect, ScimOperation, ScimUser};

fn roster() -> TeamConfig {
    TeamConfig {
        version: 1,
        default_role: TeamRole::Viewer,
        identity_provider: None,
        budgets: Vec::new(),
        policy: GovernancePolicy::default(),
        directory: DirectoryConfig::with_groups(vec![
            GroupRole {
                group: "eng-*".into(),
                role: TeamRole::Developer,
            },
            GroupRole {
                group: "magistr-owners".into(),
                role: TeamRole::Owner,
            },
        ]),
        members: vec![TeamMember {
            id: "root@example.dev".to_string(),
            name: Some("Root".to_string()),
            role: TeamRole::Owner,
            active: true,
        }],
    }
}

fn user(id: &str, groups: &[&str]) -> ScimUser {
    let mut user = ScimUser::new(id);
    user.display_name = Some(format!("User {id}"));
    user.groups = groups
        .iter()
        .map(|g| crate::team::directory::ScimValue {
            display: Some((*g).to_string()),
            ..crate::team::directory::ScimValue::default()
        })
        .collect();
    user
}

fn deactivate(id: &str) -> ScimRequest {
    ScimRequest::Patch {
        id: id.to_string(),
        operations: vec![ScimOperation {
            op: "replace".into(),
            path: Some("active".into()),
            value: serde_json::json!(false),
        }],
    }
}

// ── Create ─────────────────────────────────────────────────────────────

#[test]
fn a_create_writes_a_roster_row_with_the_role_its_groups_earn() {
    let mut config = roster();
    let outcome = apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["eng-platform"]),
        },
    )
    .expect("provisioning must need no hand edit");
    assert_eq!(outcome.effect, ScimEffect::Created);
    assert_eq!(outcome.role, TeamRole::Developer);
    let row = config.row("ada@example.dev").expect("the row was written");
    assert_eq!(row.role, TeamRole::Developer);
    assert_eq!(row.name.as_deref(), Some("User ada@example.dev"));
    assert!(row.active);
}

#[test]
fn a_create_whose_groups_map_to_nothing_takes_the_rosters_own_default() {
    let mut config = roster();
    let outcome = apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("grace@example.dev", &["finance"]),
        },
    )
    .expect("apply");
    assert_eq!(outcome.role, TeamRole::Viewer);
}

#[test]
fn a_create_over_an_active_member_is_a_uniqueness_conflict() {
    let mut config = roster();
    let error = apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("root@example.dev", &[]),
        },
    )
    .expect_err("RFC 7644 makes this a 409");
    assert!(error.to_string().contains("uniqueness conflict"), "{error}");
}

// ── Deactivate: the signal providers actually send ─────────────────────

/// Okta and Entra both deprovision with a PATCH, not a DELETE. Handling
/// only DELETE would be handling the case nobody sends.
#[test]
fn a_patch_setting_active_false_deactivates_the_member() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["eng-platform"]),
        },
    )
    .expect("create");

    let outcome = apply_scim(&mut config, &deactivate("ada@example.dev")).expect("deactivate");
    assert_eq!(outcome.effect, ScimEffect::Deactivated);
    let row = config.row("ada@example.dev").expect("the row stays");
    assert!(
        !row.active,
        "a soft delete leaves the row and clears active"
    );
}

/// Entra sends the same thing with no `path` and the attribute inside
/// the value object; a reader that handles one dialect handles half its
/// customers.
#[test]
fn a_path_less_patch_deactivates_exactly_as_a_pathed_one_does() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["eng-platform"]),
        },
    )
    .expect("create");

    let outcome = apply_scim(
        &mut config,
        &ScimRequest::Patch {
            id: "ada@example.dev".into(),
            operations: vec![ScimOperation {
                op: "replace".into(),
                path: None,
                value: serde_json::json!({ "active": false }),
            }],
        },
    )
    .expect("apply");
    assert_eq!(outcome.effect, ScimEffect::Deactivated);
    assert!(!config.row("ada@example.dev").expect("row").active);
}

/// A deactivated member must not become the roster's `default_role` —
/// the deprovisioning bug this field exists to close.
#[test]
fn a_deactivated_member_is_invisible_to_identity_resolution() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["eng-platform"]),
        },
    )
    .expect("create");
    apply_scim(&mut config, &deactivate("ada@example.dev")).expect("deactivate");

    let identity = crate::team::Identity {
        name: "ada@example.dev".into(),
        email: Some("ada@example.dev".into()),
        provider: None,
    };
    assert!(config.member_for(&identity).is_none());
    assert!(
        config.row_for(&identity).is_some(),
        "the row itself remains"
    );
}

/// A provider re-adding somebody it deprovisioned sends a POST; there
/// is nothing on its side left to patch.
#[test]
fn a_create_over_a_deactivated_member_revives_the_row() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["eng-platform"]),
        },
    )
    .expect("create");
    apply_scim(&mut config, &deactivate("ada@example.dev")).expect("deactivate");

    let outcome = apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["magistr-owners"]),
        },
    )
    .expect("re-provision");
    assert_eq!(outcome.effect, ScimEffect::Reactivated);
    assert_eq!(outcome.role, TeamRole::Owner);
    assert!(config.row("ada@example.dev").expect("row").active);
    assert_eq!(config.members.len(), 2, "reviving is not a second row");
}

#[test]
fn a_patch_setting_active_true_reactivates() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["eng-platform"]),
        },
    )
    .expect("create");
    apply_scim(&mut config, &deactivate("ada@example.dev")).expect("deactivate");

    let outcome = apply_scim(
        &mut config,
        &ScimRequest::Patch {
            id: "ada@example.dev".into(),
            operations: vec![ScimOperation {
                op: "replace".into(),
                path: Some("active".into()),
                value: serde_json::json!(true),
            }],
        },
    )
    .expect("apply");
    assert_eq!(outcome.effect, ScimEffect::Reactivated);
}

// ── Update ─────────────────────────────────────────────────────────────

#[test]
fn a_patch_of_the_groups_moves_the_member_to_the_new_role() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["eng-platform"]),
        },
    )
    .expect("create");

    let outcome = apply_scim(
        &mut config,
        &ScimRequest::Patch {
            id: "ada@example.dev".into(),
            operations: vec![ScimOperation {
                op: "replace".into(),
                path: Some("groups".into()),
                value: serde_json::json!([{ "display": "magistr-owners" }]),
            }],
        },
    )
    .expect("apply");
    assert_eq!(outcome.effect, ScimEffect::Updated);
    assert_eq!(outcome.role, TeamRole::Owner);
}

#[test]
fn a_replace_rewrites_name_role_and_provisioned_state_together() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["eng-platform"]),
        },
    )
    .expect("create");

    let mut replacement = user("ada@example.dev", &["magistr-owners"]);
    replacement.display_name = Some("Ada L.".into());
    let outcome = apply_scim(
        &mut config,
        &ScimRequest::Replace {
            id: "ada@example.dev".into(),
            user: replacement,
        },
    )
    .expect("apply");
    assert_eq!(outcome.effect, ScimEffect::Updated);
    let row = config.row("ada@example.dev").expect("row");
    assert_eq!(row.name.as_deref(), Some("Ada L."));
    assert_eq!(row.role, TeamRole::Owner);
}

#[test]
fn a_rename_moves_the_row_rather_than_adding_one() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@old.example", &["eng-platform"]),
        },
    )
    .expect("create");

    apply_scim(
        &mut config,
        &ScimRequest::Patch {
            id: "ada@old.example".into(),
            operations: vec![ScimOperation {
                op: "replace".into(),
                path: Some("userName".into()),
                value: serde_json::json!("ada@new.example"),
            }],
        },
    )
    .expect("apply");
    assert!(config.row("ada@old.example").is_none());
    assert!(config.row("ada@new.example").is_some());
    assert_eq!(config.members.len(), 2);
}

#[test]
fn a_rename_onto_an_id_already_taken_is_refused() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &[]),
        },
    )
    .expect("create");
    let error = apply_scim(
        &mut config,
        &ScimRequest::Patch {
            id: "ada@example.dev".into(),
            operations: vec![ScimOperation {
                op: "replace".into(),
                path: Some("userName".into()),
                value: serde_json::json!("root@example.dev"),
            }],
        },
    )
    .expect_err("two members cannot share an id");
    assert!(
        error.to_string().contains("already on the roster"),
        "{error}"
    );
}

/// A roster row is an id, a name, a role and whether the person is
/// provisioned — a `title` patch genuinely changes nothing here. Saying
/// so is what keeps "changed nothing" from reading as success.
#[test]
fn a_patch_of_something_the_roster_does_not_model_is_named_rather_than_dropped() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["eng-platform"]),
        },
    )
    .expect("create");

    let outcome = apply_scim(
        &mut config,
        &ScimRequest::Patch {
            id: "ada@example.dev".into(),
            operations: vec![ScimOperation {
                op: "replace".into(),
                path: Some("title".into()),
                value: serde_json::json!("Principal Engineer"),
            }],
        },
    )
    .expect("apply");
    assert_eq!(outcome.effect, ScimEffect::Unchanged);
    assert_eq!(outcome.ignored_paths, ["title".to_string()]);
}

/// Providers send the schema URN on extension paths; the attribute is
/// the same one.
#[test]
fn a_schema_qualified_path_is_the_attribute_it_names() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &["eng-platform"]),
        },
    )
    .expect("create");

    let outcome = apply_scim(
        &mut config,
        &ScimRequest::Patch {
            id: "ada@example.dev".into(),
            operations: vec![ScimOperation {
                op: "replace".into(),
                path: Some("urn:ietf:params:scim:schemas:core:2.0:User:active".into()),
                value: serde_json::json!(false),
            }],
        },
    )
    .expect("apply");
    assert_eq!(outcome.effect, ScimEffect::Deactivated);
}

// ── Delete ─────────────────────────────────────────────────────────────

/// A hard delete is a different thing from a soft one, and the report
/// says which happened.
#[test]
fn a_delete_removes_the_row_outright() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("ada@example.dev", &[]),
        },
    )
    .expect("create");

    let outcome = apply_scim(
        &mut config,
        &ScimRequest::Delete {
            id: "ada@example.dev".into(),
        },
    )
    .expect("apply");
    assert_eq!(outcome.effect, ScimEffect::Removed);
    assert!(config.row("ada@example.dev").is_none());
}

#[test]
fn an_operation_against_somebody_the_roster_does_not_carry_is_refused() {
    let mut config = roster();
    let error =
        apply_scim(&mut config, &deactivate("ghost@example.dev")).expect_err("no such member");
    assert!(error.to_string().contains("no member"), "{error}");
}

// ── The lockout guard ──────────────────────────────────────────────────

/// An IdP misconfiguration must not be able to lock a team out of its
/// own roster — the same protection a human editor gets.
#[test]
fn deactivating_the_last_owner_is_refused() {
    let mut config = roster();
    let error =
        apply_scim(&mut config, &deactivate("root@example.dev")).expect_err("lockout refused");
    assert!(error.to_string().contains("no active Owner"), "{error}");
    assert!(config.row("root@example.dev").expect("row").active);
}

#[test]
fn demoting_the_last_owner_through_a_group_change_is_refused() {
    let mut config = roster();
    let error = apply_scim(
        &mut config,
        &ScimRequest::Patch {
            id: "root@example.dev".into(),
            operations: vec![ScimOperation {
                op: "replace".into(),
                path: Some("groups".into()),
                value: serde_json::json!([{ "display": "eng-platform" }]),
            }],
        },
    )
    .expect_err("lockout refused");
    assert!(error.to_string().contains("no active Owner"), "{error}");
}

#[test]
fn deactivating_an_owner_is_fine_while_another_active_owner_remains() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("second@example.dev", &["magistr-owners"]),
        },
    )
    .expect("create");
    let outcome =
        apply_scim(&mut config, &deactivate("root@example.dev")).expect("two owners, one may go");
    assert_eq!(outcome.effect, ScimEffect::Deactivated);
}

/// A second Owner who is *deactivated* is not a second Owner.
#[test]
fn a_deactivated_owner_does_not_count_towards_keeping_the_last_one() {
    let mut config = roster();
    apply_scim(
        &mut config,
        &ScimRequest::Create {
            user: user("second@example.dev", &["magistr-owners"]),
        },
    )
    .expect("create");
    apply_scim(&mut config, &deactivate("second@example.dev")).expect("deactivate the spare");
    let error =
        apply_scim(&mut config, &deactivate("root@example.dev")).expect_err("lockout refused");
    assert!(error.to_string().contains("no active Owner"), "{error}");
}

// ── The wire shape ─────────────────────────────────────────────────────

#[test]
fn a_request_document_round_trips_through_json() {
    let request = deactivate("ada@example.dev");
    let text = serde_json::to_string(&request).expect("serialize");
    assert_eq!(
        serde_json::from_str::<ScimRequest>(&text).expect("deserialize"),
        request
    );
}

/// RFC 7644 capitalizes `Operations`; a provider body is copied
/// verbatim into the document, so the capitalized spelling has to be
/// the one that parses.
#[test]
fn the_rfcs_capitalized_operations_key_parses() {
    let text = r#"{
      "op": "patch",
      "id": "ada@example.dev",
      "Operations": [{ "op": "replace", "path": "active", "value": false }]
    }"#;
    let request: ScimRequest = serde_json::from_str(text).expect("parse");
    assert_eq!(request.member_id(), "ada@example.dev");
}

#[test]
fn a_request_naming_no_member_is_refused() {
    let mut config = roster();
    let error = apply_scim(
        &mut config,
        &ScimRequest::Delete {
            id: "   ".to_string(),
        },
    )
    .expect_err("a request about nobody");
    assert!(error.to_string().contains("names no member"), "{error}");
}
