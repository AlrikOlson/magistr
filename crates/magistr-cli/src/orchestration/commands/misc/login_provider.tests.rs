use super::*;

fn github(client_id: &str) -> IdentityProvider {
    IdentityProvider::Github {
        client_id: client_id.into(),
        scope: None,
    }
}

fn oidc(issuer: &str, client_id: &str) -> IdentityProvider {
    IdentityProvider::Oidc {
        issuer: issuer.into(),
        client_id: client_id.into(),
        scope: None,
    }
}

fn resolve(
    configured: Option<&IdentityProvider>,
    kind: Option<&str>,
    issuer: Option<&str>,
    client_id: Option<&str>,
) -> Result<IdentityProvider, MagistrError> {
    login_provider(configured, kind, issuer, client_id, None, None)
}

#[test]
fn the_roster_provider_is_the_default() {
    let configured = oidc("https://id.example.dev", "magistr-cli");
    assert_eq!(
        resolve(Some(&configured), None, None, None).expect("resolve"),
        configured
    );
}

#[test]
fn no_roster_and_no_flags_explains_both_ways_in() {
    let err = resolve(None, None, None, None).expect_err("should refuse");
    let text = err.to_string();
    assert!(text.contains("--provider github"), "{text}");
    assert!(text.contains("identity_provider"), "{text}");
}

#[test]
fn flags_alone_build_a_github_provider() {
    assert_eq!(
        resolve(None, Some("github"), None, Some("Iv1.abc")).expect("resolve"),
        github("Iv1.abc")
    );
}

#[test]
fn flags_alone_build_an_oidc_provider() {
    assert_eq!(
        resolve(
            None,
            Some("oidc"),
            Some("https://id.example.dev"),
            Some("magistr-cli")
        )
        .expect("resolve"),
        oidc("https://id.example.dev", "magistr-cli")
    );
}

#[test]
fn a_client_id_flag_overrides_the_rosters_while_keeping_its_issuer() {
    let configured = oidc("https://id.example.dev", "team-client");
    let resolved = resolve(Some(&configured), None, None, Some("my-client")).expect("resolve");
    assert_eq!(resolved, oidc("https://id.example.dev", "my-client"));
}

#[test]
fn naming_a_different_kind_does_not_inherit_the_rosters_fields() {
    let configured = oidc("https://id.example.dev", "team-client");
    let err = resolve(Some(&configured), Some("github"), None, None).expect_err("should refuse");
    assert!(err.to_string().contains("client id"), "{err}");
}

#[test]
fn naming_the_same_kind_does_inherit_them() {
    let configured = oidc("https://id.example.dev", "team-client");
    assert_eq!(
        resolve(Some(&configured), Some("oidc"), None, None).expect("resolve"),
        configured
    );
}

#[test]
fn github_falls_back_to_the_environment_client_id() {
    let resolved =
        login_provider(None, Some("github"), None, None, None, Some("Iv1.env")).expect("resolve");
    assert_eq!(resolved, github("Iv1.env"));
}

#[test]
fn an_explicit_client_id_beats_the_environment() {
    let resolved = login_provider(
        None,
        Some("github"),
        None,
        Some("Iv1.flag"),
        None,
        Some("Iv1.env"),
    )
    .expect("resolve");
    assert_eq!(resolved, github("Iv1.flag"));
}

#[test]
fn oidc_without_an_issuer_says_so() {
    let err = resolve(None, Some("oidc"), None, Some("magistr-cli")).expect_err("should refuse");
    assert!(err.to_string().contains("issuer"), "{err}");
}

#[test]
fn a_scope_flag_is_carried_through() {
    let resolved = login_provider(
        None,
        Some("github"),
        None,
        Some("Iv1.abc"),
        Some("read:org"),
        None,
    )
    .expect("resolve");
    assert_eq!(resolved.scope(), "read:org");
}

#[test]
fn an_unknown_kind_lists_the_ones_that_exist() {
    let err = resolve(None, Some("ldap"), None, None).expect_err("should refuse");
    let text = err.to_string();
    assert!(text.contains("github"), "{text}");
    assert!(text.contains("oidc"), "{text}");
}
