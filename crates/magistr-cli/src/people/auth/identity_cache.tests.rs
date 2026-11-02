use super::*;

use tempfile::TempDir;

fn at(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("timestamp")
}

fn identity(provider: &str, issued: i64, expires: i64) -> VerifiedIdentity {
    VerifiedIdentity {
        provider: provider.to_string(),
        subject: format!("{provider}-sub"),
        name: Some("Alice".into()),
        email: Some("alice@example.dev".into()),
        issued_at: at(issued),
        expires_at: at(expires),
    }
}

#[test]
fn load_from_an_absent_path_is_an_empty_cache() {
    let dir = TempDir::new().expect("tempdir");
    let cache = IdentityCache::load_from(&dir.path().join("identity.json")).expect("load");
    assert!(cache.is_empty());
}

#[test]
fn load_from_a_corrupt_file_is_a_parse_error() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("identity.json");
    std::fs::write(&path, "{ not json").expect("write");
    assert!(IdentityCache::load_from(&path).is_err());
}

#[test]
fn save_and_load_round_trip_through_a_created_directory() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("nested").join("identity.json");
    let mut cache = IdentityCache::default();
    cache.put(identity("github", 10, 100));
    cache.save_to(&path).expect("save");

    let back = IdentityCache::load_from(&path).expect("load");
    assert_eq!(back, cache);
    assert_eq!(back.entries().count(), 1);
}

#[test]
fn put_replaces_the_same_providers_earlier_login() {
    let mut cache = IdentityCache::default();
    cache.put(identity("github", 10, 100));
    cache.put(identity("github", 50, 500));
    assert_eq!(cache.entries().count(), 1);
    assert_eq!(
        cache.select(Some("github"), at(60)).map(|id| id.expires_at),
        Some(at(500))
    );
}

#[test]
fn select_by_preference_returns_that_provider() {
    let mut cache = IdentityCache::default();
    cache.put(identity("github", 10, 100));
    cache.put(identity("oidc:https://id.example.dev", 20, 100));
    let chosen = cache
        .select(Some("oidc:https://id.example.dev"), at(30))
        .expect("selected");
    assert_eq!(chosen.provider, "oidc:https://id.example.dev");
}

#[test]
fn an_expired_preferred_login_does_not_fall_through_to_another_provider() {
    let mut cache = IdentityCache::default();
    cache.put(identity("github", 10, 1000));
    cache.put(identity("oidc:https://id.example.dev", 20, 100));
    assert_eq!(
        cache.select(Some("oidc:https://id.example.dev"), at(200)),
        None
    );
}

#[test]
fn an_unknown_preferred_provider_selects_nothing() {
    let mut cache = IdentityCache::default();
    cache.put(identity("github", 10, 1000));
    assert_eq!(cache.select(Some("oidc:https://other.dev"), at(20)), None);
}

#[test]
fn without_a_preference_the_newest_unexpired_login_wins() {
    let mut cache = IdentityCache::default();
    cache.put(identity("github", 10, 1000));
    cache.put(identity("oidc:https://id.example.dev", 200, 1000));
    let chosen = cache.select(None, at(300)).expect("selected");
    assert_eq!(chosen.provider, "oidc:https://id.example.dev");
}

#[test]
fn without_a_preference_expired_logins_are_skipped() {
    let mut cache = IdentityCache::default();
    cache.put(identity("github", 10, 1000));
    cache.put(identity("oidc:https://id.example.dev", 200, 250));
    let chosen = cache.select(None, at(300)).expect("selected");
    assert_eq!(chosen.provider, "github");
}

#[test]
fn an_all_expired_cache_selects_nothing() {
    let mut cache = IdentityCache::default();
    cache.put(identity("github", 10, 100));
    assert_eq!(cache.select(None, at(100)), None);
}

#[test]
fn remove_reports_whether_there_was_anything_to_remove() {
    let mut cache = IdentityCache::default();
    cache.put(identity("github", 10, 100));
    assert!(cache.remove("github"));
    assert!(!cache.remove("github"));
    assert!(cache.is_empty());
}

#[test]
fn clear_reports_how_many_logins_it_dropped() {
    let mut cache = IdentityCache::default();
    cache.put(identity("github", 10, 100));
    cache.put(identity("oidc:https://id.example.dev", 20, 100));
    assert_eq!(cache.clear(), 2);
    assert_eq!(cache.clear(), 0);
}

#[test]
fn a_minimal_json_document_becomes_an_empty_current_cache() {
    let cache: IdentityCache = serde_json::from_str("{}").expect("deserialize");
    assert_eq!(cache.version, 2);
    assert!(cache.is_empty());
}

fn stored_cache(version: Option<u32>) -> serde_json::Value {
    let mut value = serde_json::json!({
        "identities": {
            "github": identity("github", 10, 1000),
            "oidc:https://id.example.dev": identity("oidc:https://id.example.dev", 20, 1000)
        }
    });
    if let Some(version) = version {
        value["version"] = serde_json::json!(version);
    }
    value
}

#[test]
fn direct_deserialization_drops_every_identity_from_legacy_or_unknown_versions() {
    for version in [None, Some(0), Some(1), Some(3), Some(u32::MAX)] {
        let cache: IdentityCache =
            serde_json::from_value(stored_cache(version)).expect("deserialize");
        assert!(cache.is_empty(), "version {version:?} retained identities");
        assert_eq!(cache.entries().count(), 0, "version {version:?}");
        assert!(cache.select(None, at(30)).is_none(), "version {version:?}");
        assert_eq!(cache.version, 2, "version {version:?}");
    }
}

#[test]
fn current_version_retains_verified_identities() {
    let cache: IdentityCache = serde_json::from_value(stored_cache(Some(2))).expect("deserialize");
    assert_eq!(cache.entries().count(), 2);
    assert!(cache.select(Some("github"), at(30)).is_some());
    assert!(
        cache
            .select(Some("oidc:https://id.example.dev"), at(30))
            .is_some()
    );
}

#[test]
fn loading_and_saving_a_legacy_cache_cannot_preserve_old_logins() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("identity.json");
    std::fs::write(
        &path,
        serde_json::to_string(&stored_cache(Some(1))).expect("serialize"),
    )
    .expect("write");
    let cache = IdentityCache::load_from(&path).expect("load");
    assert!(cache.is_empty());
    cache.save_to(&path).expect("save");
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(saved["version"], 2);
    assert_eq!(saved["identities"], serde_json::json!({}));
}

#[test]
fn a_new_login_cannot_upgrade_legacy_identities_alongside_it() {
    let mut cache: IdentityCache =
        serde_json::from_value(stored_cache(Some(1))).expect("deserialize");
    cache.put(identity("oidc:https://new.example.dev", 30, 1000));
    let saved = serde_json::to_value(&cache).expect("serialize");
    assert_eq!(saved["version"], 2);
    let back: IdentityCache = serde_json::from_value(saved).expect("deserialize");
    assert_eq!(back.entries().count(), 1);
    assert!(back.select(Some("github"), at(40)).is_none());
    assert!(
        back.select(Some("oidc:https://id.example.dev"), at(40))
            .is_none()
    );
    assert!(
        back.select(Some("oidc:https://new.example.dev"), at(40))
            .is_some()
    );
}
