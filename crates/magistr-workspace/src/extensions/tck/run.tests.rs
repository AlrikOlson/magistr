use super::*;
use crate::{
    extensions::{ExtensionLane, LaneFuture, PluginGuestPolicy, PluginInvokeOutcome},
    worker::WorkerGuestEnd,
};
use std::sync::Mutex;

const GOOD_TOML: &str = r#"
[plugin]
publisher = "acme"
pkg = "hello"
version = "0.1.0"

[component]
path = "guest.wasm"

[[tools]]
name = "greet"
description = "Greets the payload"

[limits]
fuel = 100000
memory_bytes = 1048576
"#;

fn lang_pack_toml() -> String {
    GOOD_TOML.replace("name = \"greet\"", "name = \"classify-file\"")
}

fn declares_observe_toml() -> String {
    format!("{GOOD_TOML}\n[capabilities]\nimports = [\"observe\"]\n")
}

/// A throwaway plugin directory holding a manifest and a component file.
fn plugin_dir(toml: Option<&str>, component: Option<&[u8]>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    if let Some(toml) = toml {
        std::fs::write(dir.path().join("plugin.toml"), toml).unwrap();
    }
    if let Some(component) = component {
        std::fs::write(dir.path().join("guest.wasm"), component).unwrap();
    }
    dir
}

#[derive(Debug, Clone)]
struct Call {
    guest: Vec<u8>,
    payload: String,
    capabilities: Vec<String>,
    policy: PluginGuestPolicy,
}

fn ok() -> PluginInvokeOutcome {
    PluginInvokeOutcome::ok(10, r#"{"a":1}"#)
}

fn cap_denied(import: &str) -> PluginInvokeOutcome {
    PluginInvokeOutcome::ended(WorkerGuestEnd::CapDenied(import.into()), 0)
}

/// A scripted invoke lane. The handler decides the outcome per call; the
/// kit's own abuse fixtures are recognized by their bytes and answer their
/// expected kills unless `broken_sandbox` says otherwise.
struct ScriptedLane {
    handler: Box<dyn Fn(&Call) -> PluginInvokeOutcome + Send + Sync>,
    broken_sandbox: bool,
    calls: Mutex<Vec<Call>>,
}

impl ScriptedLane {
    fn new(handler: impl Fn(&Call) -> PluginInvokeOutcome + Send + Sync + 'static) -> Self {
        Self {
            handler: Box::new(handler),
            broken_sandbox: false,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
}

impl ExtensionLane for ScriptedLane {
    fn invoke_tool(
        &self,
        guest: Vec<u8>,
        _export: String,
        payload: String,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, PluginInvokeOutcome> {
        let call = Call {
            guest,
            payload,
            capabilities,
            policy,
        };
        self.calls.lock().unwrap().push(call.clone());
        let fixture = std::str::from_utf8(&call.guest).unwrap_or_default();
        let answer = if !self.broken_sandbox && fixture.contains("(export \"spin\")") {
            PluginInvokeOutcome::ended(WorkerGuestEnd::OutOfFuel, 0)
        } else if !self.broken_sandbox && fixture.contains("(export \"grow\")") {
            PluginInvokeOutcome::ended(WorkerGuestEnd::MemoryCap, 0)
        } else {
            (self.handler)(&call)
        };
        Box::pin(async move { Ok(answer) })
    }
}

async fn run(dir: &tempfile::TempDir, lane: &ScriptedLane) -> TckReport {
    run_plugin_tck(dir.path(), lane, TckFuzz::default()).await
}

#[tokio::test]
async fn a_conforming_tool_plugin_is_green_in_three_chapters_and_lang_pack_is_inapplicable() {
    let lane = ScriptedLane::new(|_| ok());
    let report = run(&plugin_dir(Some(GOOD_TOML), Some(b"(component)")), &lane).await;
    assert_eq!(report.plugin.as_deref(), Some("acme.hello"));
    assert!(report.passed, "{report:?}");
    assert_eq!(
        report.chapters.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![
            TckChapterId::Interface,
            TckChapterId::Capabilities,
            TckChapterId::Resources,
            TckChapterId::LangPackFuzz
        ]
    );
    assert!(
        !report
            .chapter(TckChapterId::LangPackFuzz)
            .unwrap()
            .applicable
    );
    for id in [
        TckChapterId::Interface,
        TckChapterId::Capabilities,
        TckChapterId::Resources,
    ] {
        let chapter = report.chapter(id).unwrap();
        assert!(chapter.applicable && chapter.passed, "{id:?}");
    }
    assert!(
        serde_json::to_string(&report)
            .unwrap()
            .contains("\"lang-pack-fuzz\"")
    );
}

#[tokio::test]
async fn a_missing_manifest_or_component_fails_interface_without_touching_the_lane() {
    let lane = ScriptedLane::new(|_| ok());
    let report = run(&plugin_dir(None, Some(b"(component)")), &lane).await;
    assert_eq!(report.plugin, None);
    assert!(!report.passed);
    assert!(!report.chapter(TckChapterId::Interface).unwrap().passed);
    assert!(
        !report
            .chapter(TckChapterId::Capabilities)
            .unwrap()
            .applicable
    );
    assert!(!report.chapter(TckChapterId::Resources).unwrap().applicable);
    assert!(lane.calls().is_empty());

    let report = run(&plugin_dir(Some(GOOD_TOML), None), &lane).await;
    assert!(!report.passed);
    let interface = report.chapter(TckChapterId::Interface).unwrap();
    assert!(!interface.probe("component-present").unwrap().passed);
    assert!(lane.calls().is_empty());
}

#[tokio::test]
async fn under_declaration_fails_declared_sufficiency() {
    let lane = ScriptedLane::new(|call| {
        if call.capabilities.iter().any(|c| c == "observe") {
            ok()
        } else {
            cap_denied("observe")
        }
    });
    let report = run(&plugin_dir(Some(GOOD_TOML), Some(b"(component)")), &lane).await;
    assert!(!report.passed);
    let capabilities = report.chapter(TckChapterId::Capabilities).unwrap();
    assert!(!capabilities.passed);
    let sufficiency = capabilities.probe("declared-imports-suffice").unwrap();
    assert!(!sufficiency.passed);
    assert!(sufficiency.detail.contains("does not declare"));
}

#[tokio::test]
async fn over_declaration_fails_least_privilege_and_honest_declaration_passes() {
    let lane = ScriptedLane::new(|_| ok());
    let report = run(
        &plugin_dir(Some(&declares_observe_toml()), Some(b"(component)")),
        &lane,
    )
    .await;
    assert!(!report.passed);
    let probe = report
        .chapter(TckChapterId::Capabilities)
        .unwrap()
        .probe("least-privilege")
        .unwrap()
        .clone();
    assert!(!probe.passed);
    assert!(probe.detail.contains("does not use"));

    let lane = ScriptedLane::new(|call| {
        if call.capabilities.iter().any(|c| c == "observe") {
            ok()
        } else {
            cap_denied("observe")
        }
    });
    let report = run(
        &plugin_dir(Some(&declares_observe_toml()), Some(b"(component)")),
        &lane,
    )
    .await;
    let capabilities = report.chapter(TckChapterId::Capabilities).unwrap();
    assert!(capabilities.passed);
    assert!(
        capabilities
            .probe("least-privilege")
            .unwrap()
            .detail
            .contains("denied before instantiation")
    );
}

#[tokio::test]
async fn a_trapping_tool_fails_interface_and_starvation_accepts_only_typed_ends() {
    let lane = ScriptedLane::new(|_| {
        PluginInvokeOutcome::ended(WorkerGuestEnd::GuestTrap("unreachable".into()), 5)
    });
    let report = run(&plugin_dir(Some(GOOD_TOML), Some(b"(component)")), &lane).await;
    assert!(!report.passed);
    assert!(!report.chapter(TckChapterId::Interface).unwrap().passed);
    let starvation = report
        .chapter(TckChapterId::Resources)
        .unwrap()
        .probe("tool-greet-starvation-fuel")
        .unwrap()
        .clone();
    assert!(!starvation.passed);
}

#[tokio::test]
async fn the_kits_own_abuse_fixtures_must_die_typed_under_kit_owned_policy() {
    let mut lane = ScriptedLane::new(|_| ok());
    lane.broken_sandbox = true;
    let report = run(&plugin_dir(Some(GOOD_TOML), Some(b"(component)")), &lane).await;
    let resources = report.chapter(TckChapterId::Resources).unwrap();
    assert!(!resources.passed);
    assert!(!resources.probe("environment-fuel-kill").unwrap().passed);
    assert!(!resources.probe("environment-memory-kill").unwrap().passed);

    let lane = ScriptedLane::new(|_| ok());
    run(&plugin_dir(Some(GOOD_TOML), Some(b"(component)")), &lane).await;
    let calls = lane.calls();
    let spin = calls
        .iter()
        .find(|c| {
            std::str::from_utf8(&c.guest)
                .unwrap_or_default()
                .contains("(export \"spin\")")
        })
        .unwrap();
    let grow = calls
        .iter()
        .find(|c| {
            std::str::from_utf8(&c.guest)
                .unwrap_or_default()
                .contains("(export \"grow\")")
        })
        .unwrap();
    assert_eq!(spin.policy.fuel, 100_000);
    assert_eq!(grow.policy.memory_cap_bytes, 4 * 64 * 1024);
    assert!(spin.capabilities.is_empty() && grow.capabilities.is_empty());
}

#[tokio::test]
async fn the_lang_pack_chapter_is_deterministic_sized_and_names_a_failing_entry() {
    let first = ScriptedLane::new(|_| ok());
    let report = run(
        &plugin_dir(Some(&lang_pack_toml()), Some(b"(component)")),
        &first,
    )
    .await;
    let fuzz = report.chapter(TckChapterId::LangPackFuzz).unwrap();
    assert!(fuzz.applicable && fuzz.passed);
    assert!(fuzz.probes[0].detail.contains("all ok"));
    let second = ScriptedLane::new(|_| ok());
    run(
        &plugin_dir(Some(&lang_pack_toml()), Some(b"(component)")),
        &second,
    )
    .await;
    let payloads = |calls: Vec<Call>| -> Vec<String> {
        calls
            .into_iter()
            .map(|c| c.payload)
            .filter(|p| p.contains("\"classify-file\"") && p.contains("\"path\""))
            .collect()
    };
    let corpus = payloads(first.calls());
    assert_eq!(corpus, payloads(second.calls()));
    assert_eq!(corpus.len(), 9 + 64);
    assert_eq!(
        TckFuzz {
            iterations: 5,
            seed: 1
        }
        .corpus()
        .len(),
        14
    );

    let counter = Mutex::new(0);
    let lane = ScriptedLane::new(move |call| {
        if !call.payload.contains("\"path\"") {
            return ok();
        }
        let mut n = counter.lock().unwrap();
        *n += 1;
        if *n == 20 {
            PluginInvokeOutcome::ended(WorkerGuestEnd::GuestTrap("boom".into()), 1)
        } else {
            ok()
        }
    });
    let report = run(
        &plugin_dir(Some(&lang_pack_toml()), Some(b"(component)")),
        &lane,
    )
    .await;
    let fuzz = report.chapter(TckChapterId::LangPackFuzz).unwrap();
    assert!(!fuzz.passed);
    assert!(fuzz.probes[0].detail.contains("corpus[19]"));
    assert!(!report.passed);
}

#[tokio::test]
async fn a_dead_lane_is_a_failed_probe_never_a_kit_failure() {
    struct DeadLane;
    impl ExtensionLane for DeadLane {}
    let dir = plugin_dir(Some(GOOD_TOML), Some(b"(component)"));
    let report = run_plugin_tck(dir.path(), &DeadLane, TckFuzz::default()).await;
    assert!(!report.passed);
    let interface = report.chapter(TckChapterId::Interface).unwrap();
    assert!(
        interface
            .probe("tool-greet-invokes")
            .unwrap()
            .detail
            .contains("lane failed")
    );
}
