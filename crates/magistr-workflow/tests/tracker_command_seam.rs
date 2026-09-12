//! The served seam: the port the `tracker_setup` tool builds, executed by a
//! test against a provider this crate has never heard of and with no network
//! anywhere.
//!
//! # What was actually wrong, which is not the obvious diagnosis
//!
//! The obvious diagnosis was a port builder that matched on the provider
//! string and knew only `github` and `linear`. By the time this file was
//! written that was false twice over: the provider list had moved into
//! [`PROVIDERS`], and `tracker::registry::build_in` already took the
//! registration table as a parameter — with
//! `tests/tracker_provider_registry.rs` already driving it through a table it
//! composed itself. The seam EXISTED and stopped one level too low. The served
//! path reached its port through the arity that closes over [`PROVIDERS`], so
//! a test could prove the registry dispatches to an invented provider and
//! still could not make the served path do it.
//!
//! # Why the table, and not the two obvious alternatives
//!
//! The alternatives were a `cfg(test)` fake arm or a real,
//! production-reachable fake provider key. Both were rejected, and the
//! argument is in [`build_tracker_port_in`]'s doc comment rather than repeated
//! here. Passing the table costs neither, because there is exactly one
//! port-construction body and both callers run it.
//!
//! [`PROVIDERS`]: magistr_workflow::tracker::PROVIDERS
//! [`build_tracker_port_in`]: magistr_workflow::tracker::host::build_tracker_port_in

use magistr_workflow::tracker::TrackerConfig;
use magistr_workflow::tracker::fake::FakeTracker;
use magistr_workflow::tracker::host::{build_tracker_port_in, tracker_port_async_in};
use magistr_workflow::tracker::port::{TrackerError, TrackerPort};
use magistr_workflow::tracker::registry::{PROVIDERS, ProviderBuild, ProviderRegistration};

/// The provider key this whole file turns on. Deliberately not a plausible
/// product name: if this string ever appears in [`PROVIDERS`], the test below
/// that says it must not is the one that will notice.
const FAKE_PROVIDER: &str = "seamco";

/// A destination the fake accepts and every real adapter would refuse. It is
/// never dialled — [`FakeTracker`] makes no network call — but it has to be
/// non-empty to clear the adapter's own refusal.
const FAKE_TARGET: &str = "seamco-workspace";

/// A registration declared entirely in this test file. No core file knows it
/// exists, which is the property the end-to-end test is spending.
const SEAM_PROVIDER: ProviderRegistration = ProviderRegistration {
    key: FAKE_PROVIDER,
    build: build_seam_port,
};

fn build_seam_port(request: &ProviderBuild<'_>) -> Result<Box<dyn TrackerPort>, TrackerError> {
    // The adapter's own refusal, kept real rather than stubbed out: a
    // registration that accepted anything would make the "port construction
    // ran" half of this test unfalsifiable.
    if request.target.is_empty() {
        return Err(TrackerError::Unsupported(
            "the seam provider needs a target".into(),
        ));
    }
    Ok(Box::new(FakeTracker::new(FAKE_PROVIDER)))
}

/// The production table plus this file's registration.
///
/// Built rather than `const`, because [`ProviderRegistration`] holds a `&'static
/// str` and a fn pointer and cannot be concatenated at compile time. The
/// production entries are copied through verbatim so the path under test is
/// choosing from a SUPERSET of what ships, not from a replacement — a table
/// containing only the fake would prove dispatch works and prove nothing about
/// the real providers still being reachable beside it.
fn table_with_seam() -> Vec<ProviderRegistration> {
    let mut table: Vec<ProviderRegistration> = PROVIDERS
        .iter()
        .map(|r| ProviderRegistration {
            key: r.key,
            build: r.build,
        })
        .collect();
    table.push(SEAM_PROVIDER);
    table
}

/// The one scratch directory this binary's environment points at, for the whole
/// process and every test in it.
///
/// `PersistenceConfig::from_env` reads process-global environment, so tests
/// setting it per-test would race. Worse, a test that DIDN'T set it would
/// silently resolve the developer's real data dir: credential resolution reads
/// stored credentials from `PersistenceConfig::from_env().data_dir`, so every
/// test here must be inside the scratch before it builds any port.
fn scratch() -> &'static std::path::Path {
    static SCRATCH: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    SCRATCH
        .get_or_init(|| {
            let dir = tempfile::TempDir::new().expect("tempdir");
            // SAFETY: set exactly once, before any test in this binary reads
            // them, and this binary runs nothing else.
            unsafe {
                std::env::set_var("THINK_AND_SHIP_DATA_DIR", dir.path());
                std::env::set_var("THINK_AND_SHIP_PERSIST", "true");
                std::env::set_var("THINK_AND_SHIP_PROJECT_NAME", "tracker-command-seam");
            }
            dir
        })
        .path()
}

fn config_for(provider: &str, target: &str) -> TrackerConfig {
    TrackerConfig {
        enabled: true,
        provider: Some(provider.to_string()),
        target: Some(target.to_string()),
        ..TrackerConfig::default()
    }
}

/// THE TEST THIS FILE EXISTS FOR.
///
/// `tracker_port_async_in` is the served path one wrapper below the
/// `tracker_setup` tool: credential resolution and port construction, with the
/// tool differing only in passing [`PROVIDERS`]. The port it answers with is
/// the seam adapter's, which only the foreign table could have produced.
#[tokio::test]
async fn the_served_setup_path_builds_a_port_for_a_provider_this_crate_has_never_heard_of() {
    let _ = scratch();
    let port = tracker_port_async_in(&table_with_seam(), &config_for(FAKE_PROVIDER, FAKE_TARGET))
        .await
        .expect("the served path builds a port against the seam provider");
    assert_eq!(
        port.provider(),
        FAKE_PROVIDER,
        "the port is the one the foreign table registered"
    );
}

/// Port construction runs the adapter's own refusal, not a stub around it.
#[test]
fn the_builder_runs_the_registered_adapter_including_its_refusal() {
    let _ = scratch();
    let error =
        match build_tracker_port_in(&table_with_seam(), &config_for(FAKE_PROVIDER, ""), None) {
            Ok(port) => panic!(
                "an empty target must be refused by the adapter itself; got '{}'",
                port.provider()
            ),
            Err(e) => e,
        };
    assert!(
        error.to_string().contains("needs a target"),
        "the refusal is the adapter's own: {error}"
    );
}

/// The served path must refuse a provider that is in NO table, through the
/// same call the happy path uses.
///
/// Without this the test above is satisfied by a build that ignores its
/// argument entirely and returns something — deliberately breaking the lookup
/// showed exactly that. A dispatch that cannot refuse is not dispatching.
#[tokio::test]
async fn the_same_path_refuses_a_provider_no_table_contains() {
    let _ = scratch();
    let error = match tracker_port_async_in(
        &table_with_seam(),
        &config_for("no-such-provider-anywhere", FAKE_TARGET),
    )
    .await
    {
        Ok(port) => panic!(
            "an unregistered provider must not build a port; got '{}'",
            port.provider()
        ),
        Err(e) => e.to_string(),
    };
    assert!(
        error.contains("no-such-provider-anywhere"),
        "the refusal names what the human typed: {error}"
    );
    // …and it advertises the table it was actually given, fake included, which
    // is the proof that the refusal reads the same slice the lookup does.
    assert!(
        error.contains(FAKE_PROVIDER),
        "the refusal lists the table it was handed: {error}"
    );
}

/// The safety half, and the reason no `cfg(test)` arm was needed.
///
/// The fake is unreachable in production for the only durable reason: the
/// shipped table does not contain it. This asserts that directly, against the
/// real const rather than against a copy, and then the stronger form: the
/// production table refuses it through the same builder.
#[tokio::test]
async fn no_production_provider_string_can_select_the_fake() {
    let _ = scratch();
    assert!(
        PROVIDERS.iter().all(|r| r.key != FAKE_PROVIDER),
        "the shipped table must never register '{FAKE_PROVIDER}'"
    );
    match tracker_port_async_in(PROVIDERS, &config_for(FAKE_PROVIDER, FAKE_TARGET)).await {
        Ok(port) => panic!(
            "the shipped table built the test's fake provider as '{}'",
            port.provider()
        ),
        Err(e) => assert!(
            e.to_string().contains(FAKE_PROVIDER),
            "the shipped refusal names the provider it rejected: {e}"
        ),
    }
}
