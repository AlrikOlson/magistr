use super::super::id_token_claims::id_token_claims;
use super::super::oidc_test_fixture::OidcTestFixture;
use super::*;
use std::cell::Cell;

fn unavailable() -> Result<(JwkSet, Duration), MagistrError> {
    Err(MagistrError::Other("fixture outage".into()))
}

#[test]
fn reuses_known_keys_only_within_the_same_flow_cache() {
    let fixture = OidcTestFixture::new("key-1");
    let now = Instant::now();
    let calls = Cell::new(0);
    let fetch = || {
        calls.set(calls.get() + 1);
        Ok((fixture.jwks(), MAX_AGE))
    };
    let mut first = JwksCache::default();
    assert!(first.keys_for("key-1", now, fetch).is_ok());
    assert!(
        first
            .keys_for("key-1", now + Duration::from_secs(299), fetch)
            .is_ok()
    );
    assert_eq!(calls.get(), 1);
    let mut second = JwksCache::default();
    assert!(second.keys_for("key-1", now, fetch).is_ok());
    assert_eq!(calls.get(), 2);
}

#[test]
fn expires_at_provider_ttl_and_caps_long_lived_provider_cache() {
    for (provider_ttl, effective_ttl) in [(10, 10), (300, 300), (86400, 300)] {
        let fixture = OidcTestFixture::new("key-1");
        let now = Instant::now();
        let calls = Cell::new(0);
        let fetch = || {
            calls.set(calls.get() + 1);
            Ok((fixture.jwks(), Duration::from_secs(provider_ttl)))
        };
        let mut cache = JwksCache::default();
        cache.keys_for("key-1", now, fetch).expect("initial fetch");
        cache
            .keys_for("key-1", now + Duration::from_secs(effective_ttl - 1), fetch)
            .expect("fresh cache");
        assert_eq!(calls.get(), 1);
        cache
            .keys_for("key-1", now + Duration::from_secs(effective_ttl), fetch)
            .expect("expired cache refresh");
        assert_eq!(calls.get(), 2);
    }
}

#[test]
fn no_cache_ttl_requires_a_new_fetch_even_for_the_same_key() {
    let fixture = OidcTestFixture::new("key-1");
    let now = Instant::now();
    let calls = Cell::new(0);
    let fetch = || {
        calls.set(calls.get() + 1);
        Ok((fixture.jwks(), Duration::ZERO))
    };
    let mut cache = JwksCache::default();
    cache.keys_for("key-1", now, fetch).expect("initial fetch");
    cache
        .keys_for("key-1", now, fetch)
        .expect("revalidate immediately");
    assert_eq!(calls.get(), 2);
}

#[test]
fn failed_expired_refresh_discards_trust_and_throttles_retries() {
    let fixture = OidcTestFixture::new("key-1");
    let now = Instant::now();
    let mut cache = JwksCache::default();
    cache
        .keys_for("key-1", now, || Ok((fixture.jwks(), MAX_AGE)))
        .expect("initial fetch");
    let expired = now + MAX_AGE;
    assert!(cache.keys_for("key-1", expired, unavailable).is_err());
    assert!(cache.keys.is_none(), "failed refresh retained stale keys");
    let calls = Cell::new(0);
    let fetch = || {
        calls.set(calls.get() + 1);
        Ok((fixture.jwks(), MAX_AGE))
    };
    assert!(
        cache
            .keys_for("key-1", expired + Duration::from_secs(29), fetch)
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert!(
        cache
            .keys_for("key-1", expired + MISS_COOLDOWN, fetch)
            .is_ok()
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn first_unknown_kid_refreshes_immediately_and_supports_rotation() {
    let old = OidcTestFixture::new("old");
    let rotated = OidcTestFixture::other("new");
    let now = Instant::now();
    let mut cache = JwksCache::default();
    cache
        .keys_for("old", now, || Ok((old.jwks(), MAX_AGE)))
        .expect("initial key");
    let calls = Cell::new(0);
    let fetch = || {
        calls.set(calls.get() + 1);
        Ok((rotated.jwks(), MAX_AGE))
    };
    let keys = cache
        .keys_for("new", now, fetch)
        .expect("first unknown kid refreshes immediately after a valid load");
    assert!(keys.find("new").is_some());
    assert!(keys.find("old").is_none());
    assert_eq!(calls.get(), 1);

    let wall_time = chrono::Utc::now();
    let claims = OidcTestFixture::claims("https://issuer.example", "client", wall_time);
    let token = rotated.sign(&claims);
    let identity = id_token_claims(
        &token,
        keys,
        "https://issuer.example",
        "client",
        wall_time,
        wall_time,
    )
    .expect("rotated signature authenticates against fetched key");
    assert_eq!(identity.sub, "user-1");

    // Reusing the new kid with the retired private key cannot authenticate.
    let forged = OidcTestFixture::new("new").sign(&claims);
    assert!(
        id_token_claims(
            &forged,
            keys,
            "https://issuer.example",
            "client",
            wall_time,
            wall_time,
        )
        .is_err()
    );

    let reused = cache
        .keys_for("new", now + Duration::from_secs(1), unavailable)
        .expect("reuse rotated key without fetch");
    assert!(
        id_token_claims(
            &token,
            reused,
            "https://issuer.example",
            "client",
            wall_time,
            wall_time,
        )
        .is_ok()
    );
}

#[test]
fn random_key_misses_are_throttled_without_disabling_fresh_known_keys() {
    let fixture = OidcTestFixture::new("known");
    let now = Instant::now();
    let calls = Cell::new(0);
    let fetch = || {
        calls.set(calls.get() + 1);
        Ok((fixture.jwks(), MAX_AGE))
    };
    let mut cache = JwksCache::default();
    cache.keys_for("known", now, fetch).expect("initial key");
    for second in 1..31 {
        assert!(
            cache
                .keys_for(
                    &format!("random-{second}"),
                    now + Duration::from_secs(second),
                    fetch
                )
                .is_err()
        );
        assert!(
            cache
                .keys_for("known", now + Duration::from_secs(second), fetch)
                .is_ok()
        );
        assert_eq!(
            calls.get(),
            2,
            "first miss refreshes at t1; t{second} must not refetch"
        );
    }
    assert_eq!(
        calls.get(),
        2,
        "first miss refreshes; subsequent misses are throttled"
    );
    assert!(
        cache
            .keys_for("still-unknown", now + Duration::from_secs(31), fetch)
            .is_err()
    );
    assert_eq!(
        calls.get(),
        3,
        "the miss cooldown expires after thirty seconds"
    );
    assert!(
        cache
            .keys_for("another-random", now + Duration::from_secs(32), fetch)
            .is_err()
    );
    assert_eq!(calls.get(), 3);
}

#[test]
fn failed_rotation_refresh_never_falls_back_to_old_fresh_keys() {
    let fixture = OidcTestFixture::new("old");
    let now = Instant::now();
    let mut cache = JwksCache::default();
    cache
        .keys_for("old", now, || Ok((fixture.jwks(), MAX_AGE)))
        .expect("initial key");
    assert!(
        cache
            .keys_for("new", now + MISS_COOLDOWN, unavailable)
            .is_err()
    );
    assert!(cache.keys.is_none());
    assert!(
        cache
            .keys_for("old", now + MISS_COOLDOWN, unavailable)
            .is_err()
    );
}

#[test]
fn initial_fetch_failure_is_throttled_until_cooldown_expires() {
    let now = Instant::now();
    let calls = Cell::new(0);
    let fail = || {
        calls.set(calls.get() + 1);
        unavailable()
    };
    let mut cache = JwksCache::default();
    for second in 0..30 {
        assert!(
            cache
                .keys_for("key-1", now + Duration::from_secs(second), fail)
                .is_err()
        );
    }
    assert_eq!(calls.get(), 1);
    assert!(cache.keys_for("key-1", now + MISS_COOLDOWN, fail).is_err());
    assert_eq!(calls.get(), 2);
}

#[test]
fn accepts_exactly_sixty_four_keys_and_rejects_empty_or_oversized_sets() {
    let fixture = OidcTestFixture::new("key-0");
    let key = fixture.jwks().keys.remove(0);
    for count in [0, 64, 65] {
        let mut keys = Vec::with_capacity(count);
        for index in 0..count {
            let mut key = key.clone();
            key.common.key_id = Some(format!("key-{index}"));
            keys.push(key);
        }
        let mut cache = JwksCache::default();
        let result = cache.keys_for("key-0", Instant::now(), || Ok((JwkSet { keys }, MAX_AGE)));
        assert_eq!(result.is_ok(), count == 64, "{count} keys");
        if count != 64 {
            assert!(cache.keys.is_none());
        }
    }
}

#[test]
fn invalid_refresh_discards_previously_trusted_keys() {
    let fixture = OidcTestFixture::new("old");
    let now = Instant::now();
    let mut cache = JwksCache::default();
    cache
        .keys_for("old", now, || Ok((fixture.jwks(), MAX_AGE)))
        .expect("initial key");
    assert!(
        cache
            .keys_for("new", now + MISS_COOLDOWN, || Ok((
                JwkSet { keys: Vec::new() },
                MAX_AGE
            )))
            .is_err()
    );
    assert!(cache.keys.is_none());
}
