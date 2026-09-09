use super::*;

/// A real Okta `POST /Users` body, trimmed to the attributes a roster
/// row can hold plus a few it cannot.
const OKTA_CREATE: &str = r#"{
  "schemas": ["urn:ietf:params:scim:schemas:core:2.0:User"],
  "externalId": "00u1a2b3c4",
  "userName": "ada@example.dev",
  "name": { "givenName": "Ada", "familyName": "Lovelace" },
  "emails": [
    { "value": "ada@personal.example", "type": "home", "primary": false },
    { "value": "ada@example.dev", "type": "work", "primary": true }
  ],
  "groups": [
    { "value": "00g9z8y7", "display": "eng-platform" },
    { "value": "00g1a2b3", "display": "CN=magistr-owners,OU=Groups,DC=example,DC=com" }
  ],
  "title": "Principal Engineer",
  "active": true
}"#;

fn okta() -> ScimUser {
    serde_json::from_str(OKTA_CREATE).expect("a provider body must parse")
}

#[test]
fn a_provider_body_parses_and_unmodelled_attributes_are_simply_not_read() {
    let user = okta();
    assert_eq!(user.user_name, "ada@example.dev");
    assert_eq!(user.external_id.as_deref(), Some("00u1a2b3c4"));
    assert!(user.active);
}

/// One key, readable in both systems — no magistr-minted id to keep in
/// step with the IdP's.
#[test]
fn the_member_id_is_the_user_name() {
    assert_eq!(okta().member_id(), "ada@example.dev");
}

#[test]
fn the_display_name_is_built_from_the_name_parts_when_there_is_no_display_name() {
    assert_eq!(okta().display().as_deref(), Some("Ada Lovelace"));
}

#[test]
fn an_explicit_display_name_wins_over_the_parts() {
    let mut user = okta();
    user.display_name = Some("A. Lovelace".to_string());
    assert_eq!(user.display().as_deref(), Some("A. Lovelace"));
}

#[test]
fn a_user_with_no_name_at_all_has_no_display_name() {
    assert_eq!(ScimUser::new("bot@example.dev").display(), None);
}

/// A group reference carries the directory's opaque id in `value` and
/// the name in `display`; a mapping row is written against the name.
#[test]
fn a_group_reference_maps_by_its_display_name_not_its_opaque_id() {
    assert_eq!(
        okta().group_names(),
        ["eng-platform".to_string(), "magistr-owners".to_string()],
        "the second is a DN and reduces to its own CN"
    );
}

#[test]
fn a_group_reference_with_no_display_falls_back_to_its_value() {
    let mut user = ScimUser::new("x@example.dev");
    user.groups = vec![ScimValue {
        value: Some("eng-tools".into()),
        ..ScimValue::default()
    }];
    assert_eq!(user.group_names(), ["eng-tools".to_string()]);
}

#[test]
fn the_primary_email_is_the_one_marked_primary() {
    assert_eq!(okta().primary_email(), Some("ada@example.dev"));
}

/// Entra sends no `active` on a create; absent has to mean provisioned,
/// or every new user would land deactivated.
#[test]
fn a_body_that_omits_active_is_provisioned() {
    let user: ScimUser = serde_json::from_str(r#"{"userName":"new@example.dev"}"#).expect("parse");
    assert!(user.active);
}
