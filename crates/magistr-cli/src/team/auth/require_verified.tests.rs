use super::*;

use super::super::identity_provider::IdentityProvider;
use crate::team::TeamRole;
use crate::team::directory::DirectoryConfig;

fn identity(provider: Option<&str>) -> Identity {
    Identity {
        name: "Alice".into(),
        email: Some("alice@example.dev".into()),
        provider: provider.map(String::from),
    }
}

fn team(provider: Option<IdentityProvider>) -> TeamConfig {
    TeamConfig {
        budgets: Vec::new(),
        policy: Default::default(),
        version: 1,
        default_role: TeamRole::Viewer,
        identity_provider: provider,
        members: Vec::new(),
        directory: DirectoryConfig::default(),
    }
}

fn github() -> IdentityProvider {
    IdentityProvider::Github {
        client_id: "Iv1.abc".into(),
        scope: None,
    }
}

#[test]
fn solo_mode_never_demands_a_login() {
    assert!(require_verified(None, &identity(None)).is_ok());
}

#[test]
fn a_team_that_sets_no_provider_never_demands_a_login() {
    assert!(require_verified(Some(&team(None)), &identity(None)).is_ok());
}

#[test]
fn the_required_provider_is_accepted() {
    assert!(require_verified(Some(&team(Some(github()))), &identity(Some("github"))).is_ok());
}

#[test]
fn an_unverified_identity_is_refused_with_the_command_to_run() {
    let err =
        require_verified(Some(&team(Some(github()))), &identity(None)).expect_err("should refuse");
    assert!(matches!(err, MagistrError::Permission(_)), "{err:?}");
    let text = err.to_string();
    assert!(text.contains("GitHub"), "{text}");
    assert!(text.contains("magistr login"), "{text}");
    assert!(text.contains("self-asserted"), "{text}");
}

#[test]
fn the_wrong_provider_is_refused_and_names_both() {
    let required = IdentityProvider::Oidc {
        issuer: "https://id.example.dev".into(),
        client_id: "magistr-cli".into(),
        scope: None,
    };
    let err = require_verified(Some(&team(Some(required))), &identity(Some("github")))
        .expect_err("should refuse");
    let text = err.to_string();
    assert!(text.contains("https://id.example.dev"), "{text}");
    assert!(text.contains("github"), "{text}");
}

#[test]
fn an_oidc_login_for_another_client_at_the_same_issuer_is_refused() {
    let required = IdentityProvider::Oidc {
        issuer: "https://id.example.dev".into(),
        client_id: "team-client".into(),
        scope: None,
    };
    let other_client = IdentityProvider::Oidc {
        issuer: "https://id.example.dev".into(),
        client_id: "public-client".into(),
        scope: None,
    };
    let matching = identity(Some(&required.key()));
    let wrong_client = identity(Some(&other_client.key()));
    let config = team(Some(required));
    assert!(require_verified(Some(&config), &matching).is_ok());
    assert!(
        require_verified(Some(&config), &wrong_client).is_err(),
        "a token verified for public-client must not authorize team-client"
    );
}

#[test]
fn cached_oidc_identity_is_not_selected_for_another_registered_client() {
    use super::super::{IdentityCache, VerifiedIdentity};
    use chrono::{TimeDelta, Utc};

    let required = IdentityProvider::Oidc {
        issuer: "https://id.example.dev".into(),
        client_id: "team-client".into(),
        scope: None,
    };
    let other_client = IdentityProvider::Oidc {
        issuer: "https://id.example.dev".into(),
        client_id: "public-client".into(),
        scope: None,
    };
    let now = Utc::now();
    let mut cache = IdentityCache::default();
    cache.put(VerifiedIdentity {
        provider: other_client.key(),
        subject: "same-user".into(),
        name: Some("Alice".into()),
        email: Some("alice@example.dev".into()),
        issued_at: now,
        expires_at: now + TimeDelta::seconds(60),
    });
    assert!(cache.select(Some(&other_client.key()), now).is_some());
    assert!(
        cache.select(Some(&required.key()), now).is_none(),
        "the team's cache preference must include its OIDC client ID"
    );
}
