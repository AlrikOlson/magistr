use super::*;

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

fn verified(provider: &str, name: &str, email: &str) -> VerifiedIdentity {
    let issued = chrono::DateTime::from_timestamp(0, 0).expect("timestamp");
    VerifiedIdentity {
        provider: provider.to_string(),
        subject: "sub-1".into(),
        name: Some(name.to_string()),
        email: Some(email.to_string()),
        issued_at: issued,
        expires_at: issued,
    }
}

#[test]
fn env_user_takes_precedence_over_everything() {
    let id = from_parts(
        s("Alice <alice@example.dev>"),
        Some(verified("github", "Verified Vera", "vera@example.dev")),
        s("Git Name"),
        s("git@example.dev"),
        s("osuser"),
    );
    assert_eq!(id.name, "Alice");
    assert_eq!(id.email.as_deref(), Some("alice@example.dev"));
}

#[test]
fn env_user_bare_email_derives_name_from_local_part() {
    let id = from_parts(s("bob@example.dev"), None, None, None, None);
    assert_eq!(id.name, "bob");
    assert_eq!(id.email.as_deref(), Some("bob@example.dev"));
}

#[test]
fn env_user_bare_username_has_no_email() {
    let id = from_parts(s("carol"), None, s("Git Name"), s("git@example.dev"), None);
    assert_eq!(id.name, "carol");
    assert_eq!(id.email, None);
}

#[test]
fn env_user_bracket_form_without_name_uses_local_part() {
    let id = from_parts(s("<dave@example.dev>"), None, None, None, None);
    assert_eq!(id.name, "dave");
    assert_eq!(id.email.as_deref(), Some("dave@example.dev"));
}

#[test]
fn git_identity_used_when_no_env() {
    let id = from_parts(None, None, s("Git Name"), s("git@example.dev"), s("osuser"));
    assert_eq!(id.name, "Git Name");
    assert_eq!(id.email.as_deref(), Some("git@example.dev"));
}

#[test]
fn os_user_fills_name_when_git_has_only_email() {
    let id = from_parts(None, None, None, s("git@example.dev"), s("osuser"));
    assert_eq!(id.name, "osuser");
    assert_eq!(id.email.as_deref(), Some("git@example.dev"));
}

#[test]
fn email_local_part_fills_name_as_last_resort() {
    let id = from_parts(None, None, None, s("erin@example.dev"), None);
    assert_eq!(id.name, "erin");
}

#[test]
fn os_user_alone_gives_name_without_email() {
    let id = from_parts(None, None, None, None, s("osuser"));
    assert_eq!(id.name, "osuser");
    assert_eq!(id.email, None);
}

#[test]
fn no_sources_resolves_to_unknown() {
    let id = from_parts(None, None, None, None, None);
    assert_eq!(id.name, "unknown");
    assert_eq!(id.email, None);
}

#[test]
fn id_prefers_email_over_name() {
    let with_email = Identity {
        name: "Alice".into(),
        email: Some("alice@example.dev".into()),
        provider: None,
    };
    assert_eq!(with_email.id(), "alice@example.dev");

    let without_email = Identity {
        name: "alice".into(),
        email: None,
        provider: None,
    };
    assert_eq!(without_email.id(), "alice");
}

#[test]
fn env_user_outranks_a_verified_login_but_is_not_verified_by_it() {
    let id = from_parts(
        s("Alice <alice@example.dev>"),
        Some(verified("github", "Verified Vera", "vera@example.dev")),
        None,
        None,
        None,
    );
    assert_eq!(id.name, "Alice");
    assert_eq!(
        id.provider, None,
        "a local override must not inherit the login's verification"
    );
}

#[test]
fn a_verified_login_outranks_git_config() {
    let id = from_parts(
        None,
        Some(verified("github", "Verified Vera", "vera@example.dev")),
        s("Git Name"),
        s("git@example.dev"),
        s("osuser"),
    );
    assert_eq!(id.name, "Verified Vera");
    assert_eq!(id.email.as_deref(), Some("vera@example.dev"));
    assert_eq!(id.provider.as_deref(), Some("github"));
}

#[test]
fn a_verified_login_without_a_name_falls_back_to_its_email_local_part() {
    let mut login = verified("github", "", "vera@example.dev");
    login.name = None;
    let id = from_parts(None, Some(login), s("Git Name"), None, None);
    assert_eq!(id.name, "vera");
    assert_eq!(id.provider.as_deref(), Some("github"));
}

#[test]
fn local_resolution_is_never_marked_verified() {
    let id = from_parts(None, None, s("Git Name"), s("git@example.dev"), None);
    assert_eq!(id.provider, None);
}
