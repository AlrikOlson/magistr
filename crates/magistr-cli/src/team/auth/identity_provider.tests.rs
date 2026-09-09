use super::*;

fn github() -> IdentityProvider {
    IdentityProvider::Github {
        client_id: "Iv1.abc".into(),
        scope: None,
    }
}

fn oidc(issuer: &str) -> IdentityProvider {
    IdentityProvider::Oidc {
        issuer: issuer.into(),
        client_id: "magistr-cli".into(),
        scope: None,
    }
}

#[test]
fn github_key_is_stable_and_issuer_free() {
    assert_eq!(github().key(), "github");
}

#[test]
fn oidc_cache_keys_preserve_exact_issuer_identity() {
    assert_eq!(
        oidc("https://id.example.dev/").key(),
        "oidc:https://id.example.dev/#client_id=bWFnaXN0ci1jbGk"
    );
    assert_ne!(
        oidc("https://id.example.dev/").key(),
        oidc("https://id.example.dev").key()
    );
}

#[test]
fn oidc_cache_keys_bind_the_registered_client_id() {
    let first = oidc("https://id.example.dev");
    let second = IdentityProvider::Oidc {
        issuer: "https://id.example.dev".into(),
        client_id: "another-client".into(),
        scope: None,
    };
    assert_ne!(first.key(), second.key());
    let delimiter_in_client = IdentityProvider::Oidc {
        issuer: "https://id.example.dev".into(),
        client_id: "a#client_id=b".into(),
        scope: None,
    };
    assert_eq!(
        delimiter_in_client.key(),
        "oidc:https://id.example.dev#client_id=YSNjbGllbnRfaWQ9Yg"
    );
}

#[test]
fn scopes_fall_back_to_the_provider_default() {
    assert_eq!(github().scope(), GITHUB_DEFAULT_SCOPE);
    assert_eq!(oidc("https://id.example.dev").scope(), OIDC_DEFAULT_SCOPE);
}

#[test]
fn an_explicit_scope_wins() {
    let provider = IdentityProvider::Oidc {
        issuer: "https://id.example.dev".into(),
        client_id: "magistr-cli".into(),
        scope: Some("openid".into()),
    };
    assert_eq!(provider.scope(), "openid");
}

#[test]
fn serde_is_tagged_by_kind() {
    let json = serde_json::to_string(&github()).expect("serialize");
    assert!(json.contains("\"kind\":\"github\""), "{json}");
    let back: IdentityProvider = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, github());
}

#[test]
fn oidc_deserializes_without_a_scope_field() {
    let provider: IdentityProvider = serde_json::from_str(
        r#"{"kind":"oidc","issuer":"https://id.example.dev","client_id":"magistr-cli"}"#,
    )
    .expect("deserialize");
    assert_eq!(provider, oidc("https://id.example.dev"));
}

#[test]
fn labels_name_the_provider() {
    assert_eq!(github().label(), "GitHub");
    assert_eq!(
        oidc("https://id.example.dev").label(),
        "OIDC (https://id.example.dev)"
    );
}
