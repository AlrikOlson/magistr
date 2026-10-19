use super::*;
use crate::{
    config::ResolveOptions,
    contracts::{ErrorKind, WorkspaceResponse},
    extensions::{
        ExtensionLane, InstalledPluginRecord, LaneFuture, PluginGuestPolicy, PluginInventory,
        PluginInvokeOutcome, PluginManifest,
    },
    worker::WorkerGuestEnd,
    workspace_policy::WorkspacePolicy,
};
use std::{
    fs,
    sync::{Arc, Mutex},
};

pub(crate) const GOOD_TOML: &str = r#"
[plugin]
publisher = "acme"
pkg = "hello"
version = "0.1.0"

[component]
path = "guest.wasm"

[[tools]]
name = "greet"
description = "Greets the payload"

[capabilities]
imports = ["observe"]
fs-read = ["src/**"]

[limits]
fuel = 500
memory_bytes = 1048576
"#;

/// One recorded lane call: what the host handed to the guest.
#[derive(Debug, Clone)]
pub(crate) struct Invoke {
    pub export: String,
    pub payload: String,
    pub capabilities: Vec<String>,
    pub policy: PluginGuestPolicy,
}

/// A scripted lane: answers the configured outcome and records every call.
pub(crate) struct FakeLane {
    pub outcome: Mutex<Result<PluginInvokeOutcome, String>>,
    pub invokes: Mutex<Vec<Invoke>>,
}

impl FakeLane {
    pub fn answering(outcome: PluginInvokeOutcome) -> Self {
        Self {
            outcome: Mutex::new(Ok(outcome)),
            invokes: Mutex::new(Vec::new()),
        }
    }

    pub fn failing(message: &str) -> Self {
        Self {
            outcome: Mutex::new(Err(message.to_owned())),
            invokes: Mutex::new(Vec::new()),
        }
    }

    pub fn calls(&self) -> Vec<Invoke> {
        self.invokes.lock().unwrap().clone()
    }
}

impl ExtensionLane for FakeLane {
    fn invoke_tool(
        &self,
        _guest: Vec<u8>,
        export: String,
        payload: String,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, PluginInvokeOutcome> {
        self.invokes.lock().unwrap().push(Invoke {
            export,
            payload,
            capabilities,
            policy,
        });
        let answer = self.outcome.lock().unwrap().clone();
        Box::pin(async move {
            answer.map_err(|message| {
                crate::contracts::WorkspaceError::new(ErrorKind::EngineUnavailable, message)
            })
        })
    }
}

pub(crate) fn ok_outcome() -> PluginInvokeOutcome {
    PluginInvokeOutcome::ok(10, r#"{"greeting":"hi"}"#)
}

/// A real temp workspace with one installed plugin (manifest + component).
pub(crate) struct Workspace {
    pub dir: tempfile::TempDir,
    pub policy: WorkspacePolicy,
}

impl Workspace {
    pub fn new(toml: &str, grants: Option<&[&str]>, component: &[u8]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".magistr/workspace")).unwrap();
        fs::create_dir_all(root.join("plugins/hello")).unwrap();
        fs::write(root.join("plugins/hello/plugin.toml"), toml).unwrap();
        fs::write(root.join("plugins/hello/guest.wasm"), component).unwrap();
        let mut record = serde_json::json!({
            "name": "acme-hello",
            "source": "sideload",
            "dir": "plugins/hello",
        });
        if let Some(grants) = grants {
            record["grants"] = serde_json::json!(grants);
        }
        fs::write(
            root.join(".magistr/workspace/plugins.json"),
            serde_json::json!({ "plugins": [record] }).to_string(),
        )
        .unwrap();
        let policy = WorkspacePolicy::resolve(&ResolveOptions::new(root)).unwrap();
        Self { dir, policy }
    }

    pub fn state_dir(&self) -> std::path::PathBuf {
        self.dir.path().join(".magistr/workspace")
    }

    pub fn host<'a>(&'a self, lane: &'a dyn ExtensionLane) -> PluginHost<'a> {
        PluginHost::new(&self.policy, self.state_dir(), lane)
            .with_clock(|| "2026-08-22T00:00:00Z".to_owned())
    }

    pub fn rewrite_component(&self, bytes: &[u8]) {
        fs::write(self.dir.path().join("plugins/hello/guest.wasm"), bytes).unwrap();
    }

    pub fn breaches(&self) -> Vec<PluginHealthRecord> {
        PluginHealth::new(&self.state_dir()).read()
    }
}

fn refused(response: WorkspaceResponse<serde_json::Value>) -> (ErrorKind, String) {
    match response {
        WorkspaceResponse::Failure { error, .. } => (error.kind, error.message),
        WorkspaceResponse::Success { results, .. } => {
            panic!("expected refusal, got {:?}", results.rows())
        }
    }
}

#[tokio::test]
async fn plugin_tools_list_under_derived_namespaces_and_unknown_names_answer_none() {
    let ws = Workspace::new(GOOD_TOML, None, b"(component)");
    let lane = FakeLane::answering(ok_outcome());
    let host = ws.host(&lane);
    let (tools, notes) = host.list_tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "acme.hello/greet");
    assert!(tools[0].description.contains("sideload, untrusted"));
    assert!(notes.is_empty());
    assert!(PluginHost::is_plugin_tool_name("acme.hello/greet"));
    assert!(!PluginHost::is_plugin_tool_name("search"));
    let args = serde_json::json!({});
    assert!(
        host.run_tool("acme.hello/nope", &args, 4000)
            .await
            .is_none()
    );
    assert!(
        host.run_tool("other.pkg/greet", &args, 4000)
            .await
            .is_none()
    );
    assert!(lane.calls().is_empty());
}

#[tokio::test]
async fn a_manifest_declaring_a_core_tool_name_is_refused_wholesale() {
    let ws = Workspace::new(
        &GOOD_TOML.replace("name = \"greet\"", "name = \"search\""),
        None,
        b"(component)",
    );
    let lane = FakeLane::answering(ok_outcome());
    let host = ws.host(&lane);
    let (tools, notes) = host.list_tools();
    assert!(tools.is_empty());
    assert!(notes[0].contains("invalid manifest") && notes[0].contains("reserved"));
    assert!(
        host.run_tool("acme.hello/search", &serde_json::json!({}), 4000)
            .await
            .is_none()
    );
}

#[tokio::test]
async fn default_deny_and_declared_intersect_granted_reach_the_guest() {
    let ws = Workspace::new(GOOD_TOML, None, b"(component)");
    let lane = FakeLane::answering(ok_outcome());
    ws.host(&lane)
        .run_tool("acme.hello/greet", &serde_json::json!({}), 4000)
        .await
        .unwrap();
    assert_eq!(lane.calls()[0].capabilities, Vec::<String>::new());

    let ws = Workspace::new(
        GOOD_TOML,
        Some(&["observe", "magistr:tool/host@0.1.0"]),
        b"(component)",
    );
    let lane = FakeLane::answering(ok_outcome());
    ws.host(&lane)
        .run_tool("acme.hello/greet", &serde_json::json!({"who": "x"}), 4000)
        .await
        .unwrap();
    let call = &lane.calls()[0];
    assert_eq!(call.capabilities, vec!["observe"]);
    assert_eq!(call.export, "invoke");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&call.payload).unwrap(),
        serde_json::json!({"v": 1, "tool": "greet", "args": {"who": "x"}})
    );
    assert_eq!(
        call.policy,
        PluginGuestPolicy {
            fuel: 500,
            memory_cap_bytes: 1_048_576
        }
    );
}

#[tokio::test]
async fn a_cap_denied_guest_end_is_plugin_cap_denied_with_no_breach() {
    let ws = Workspace::new(GOOD_TOML, None, b"(component)");
    let lane = FakeLane::answering(PluginInvokeOutcome::ended(
        WorkerGuestEnd::CapDenied("observe".into()),
        0,
    ));
    let response = ws
        .host(&lane)
        .run_tool("acme.hello/greet", &serde_json::json!({}), 4000)
        .await
        .unwrap();
    let (kind, message) = refused(response);
    assert_eq!(kind, ErrorKind::PluginCapDenied);
    assert!(message.contains("\"observe\""));
    assert!(ws.breaches().is_empty());
}

#[tokio::test]
async fn a_trapped_guest_is_plugin_crashed_breached_on_disk_and_short_circuited_after() {
    let ws = Workspace::new(GOOD_TOML, None, b"(component)");
    let lane = FakeLane::answering(PluginInvokeOutcome::ended(
        WorkerGuestEnd::GuestTrap("unreachable".into()),
        5,
    ));
    let host = ws.host(&lane);
    let (kind, message) = refused(
        host.run_tool("acme.hello/greet", &serde_json::json!({}), 4000)
            .await
            .unwrap(),
    );
    assert_eq!(
        kind,
        ErrorKind::PluginCrashed {
            plugin: "acme.hello".into()
        }
    );
    assert!(message.contains("trapped"));
    let breaches = ws.breaches();
    assert_eq!(breaches.len(), 1);
    assert_eq!(breaches[0].plugin, "acme.hello");
    assert_eq!(breaches[0].reason, "guest-trap: unreachable");
    assert_eq!(breaches[0].at, "2026-08-22T00:00:00Z");
    let (kind, message) = refused(
        host.run_tool("acme.hello/greet", &serde_json::json!({}), 4000)
            .await
            .unwrap(),
    );
    assert!(matches!(kind, ErrorKind::PluginCrashed { .. }));
    assert!(message.contains("circuit-breakered"));
    assert_eq!(lane.calls().len(), 1, "the lane was not touched again");
    // A changed component clears the breaker (quarantine recovery story).
    ws.rewrite_component(b"(component (core module))");
    *lane.outcome.lock().unwrap() = Ok(PluginInvokeOutcome::ended(WorkerGuestEnd::OutOfFuel, 500));
    let (_, message) = refused(
        host.run_tool("acme.hello/greet", &serde_json::json!({}), 4000)
            .await
            .unwrap(),
    );
    assert!(message.contains("exhausted its fuel"));
    assert_eq!(lane.calls().len(), 2);
    assert_eq!(ws.breaches().len(), 2);
}

#[tokio::test]
async fn manifest_limits_clamp_downward_against_the_host_ceiling_never_up() {
    let greedy = GOOD_TOML
        .replace("fuel = 500", "fuel = 5000000000000")
        .replace("memory_bytes = 1048576", "memory_bytes = 5000000000000");
    let ws = Workspace::new(&greedy, None, b"(component)");
    let lane = FakeLane::answering(ok_outcome());
    ws.host(&lane)
        .run_tool("acme.hello/greet", &serde_json::json!({}), 4000)
        .await
        .unwrap();
    assert_eq!(lane.calls()[0].policy, PluginGuestPolicy::ceiling());
}

#[tokio::test]
async fn an_ok_result_is_host_wrapped_and_json_parsed() {
    let ws = Workspace::new(GOOD_TOML, None, b"(component)");
    let lane = FakeLane::answering(ok_outcome());
    let response = ws
        .host(&lane)
        .run_tool("acme.hello/greet", &serde_json::json!({"name": "x"}), 4000)
        .await
        .unwrap();
    let WorkspaceResponse::Success { results, notes, .. } = response else {
        panic!("expected success");
    };
    assert_eq!(results.rows()[0], serde_json::json!({"greeting": "hi"}));
    let joined = notes.unwrap().join(" ");
    assert!(joined.contains("host-wrapped") && joined.contains("sideload"));

    *lane.outcome.lock().unwrap() = Ok(PluginInvokeOutcome::ok(1, "not json"));
    let WorkspaceResponse::Success { results, .. } = ws
        .host(&lane)
        .run_tool("acme.hello/greet", &serde_json::json!({}), 4000)
        .await
        .unwrap()
    else {
        panic!("expected success");
    };
    assert_eq!(results.rows()[0], serde_json::json!({"raw": "not json"}));
}

#[tokio::test]
async fn a_failed_lane_is_engine_unavailable_not_a_breach_and_a_missing_component_is_crashed() {
    let ws = Workspace::new(GOOD_TOML, None, b"(component)");
    let lane = FakeLane::failing("SANDBOX_UNAVAILABLE: no host");
    let (kind, message) = refused(
        ws.host(&lane)
            .run_tool("acme.hello/greet", &serde_json::json!({}), 4000)
            .await
            .unwrap(),
    );
    assert_eq!(kind, ErrorKind::EngineUnavailable);
    assert!(message.contains("no host"));
    assert!(ws.breaches().is_empty());

    fs::remove_file(ws.dir.path().join("plugins/hello/guest.wasm")).unwrap();
    let lane = FakeLane::answering(ok_outcome());
    let (kind, message) = refused(
        ws.host(&lane)
            .run_tool("acme.hello/greet", &serde_json::json!({}), 4000)
            .await
            .unwrap(),
    );
    assert!(matches!(kind, ErrorKind::PluginCrashed { .. }));
    assert!(message.contains("missing"));
    assert!(lane.calls().is_empty());
}

#[tokio::test]
async fn the_subprocess_grant_is_inert_below_blessed_and_the_inventory_fails_closed() {
    let resolver = include_str!("../../fixtures/plugins/lsp-resolver/plugin.toml");
    let manifest = PluginManifest::parse(resolver).unwrap().manifest;
    let record = |source: &str, tier: Option<&str>| {
        let mut value = serde_json::json!({
            "name": "magistr.lsp-resolver",
            "source": source,
            "dir": "plugins/lsp-resolver",
            "grants": ["magistr:subprocess/subprocess@0.2.0"],
        });
        if let Some(tier) = tier {
            value["tier"] = serde_json::json!(tier);
        }
        InstalledPluginRecord::from_value(&value).unwrap()
    };
    let blessed = EffectiveCapabilities::resolve(&record("registry", Some("blessed")), &manifest);
    assert_eq!(
        blessed.effective,
        vec!["magistr:subprocess/subprocess@0.2.0"]
    );
    assert!(blessed.notes.is_empty());
    let verified = EffectiveCapabilities::resolve(&record("registry", Some("verified")), &manifest);
    assert!(verified.effective.is_empty());
    assert!(verified.notes[0].contains("reserved to blessed"));
    // A sideload cannot label itself blessed: the tier is dropped at decode.
    let sideload = record("sideload", Some("blessed"));
    assert_eq!(sideload.tier, None);
    assert!(
        EffectiveCapabilities::resolve(&sideload, &manifest)
            .effective
            .is_empty()
    );
    assert_eq!(sideload.pkg(), "lsp-resolver");
    let escaping = InstalledPluginRecord::from_value(&serde_json::json!({
        "name": "x", "dir": "../outside", "grants": ["a", 1]
    }))
    .unwrap();
    assert_eq!(escaping.dir, None);
    assert_eq!(escaping.grants, None);
    assert!(InstalledPluginRecord::from_value(&serde_json::json!({"dir": "d"})).is_err());
}

#[test]
fn the_inventory_and_breaker_files_are_read_per_call_and_tolerate_absence() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join(".magistr/workspace");
    assert_eq!(PluginInventory::load(&state).unwrap(), Vec::new());
    let health = PluginHealth::new(&state);
    assert!(health.read().is_empty());
    health
        .append(PluginHealthRecord {
            plugin: "a.b".into(),
            component_hash: "h1".into(),
            reason: "boom".into(),
            at: "2026-08-22T00:00:00Z".into(),
        })
        .unwrap();
    assert!(health.breach("a.b", "h1").is_some());
    assert!(health.breach("a.b", "h2").is_none());
    let text = fs::read_to_string(state.join("plugin-health.json")).unwrap();
    assert!(text.contains("\"componentHash\": \"h1\""));
    fs::write(state.join("plugins.json"), "{\"plugins\": 1}").unwrap();
    assert!(PluginInventory::load(&state).is_err());
    fs::write(state.join("plugin-health.json"), "garbage").unwrap();
    assert!(health.read().is_empty());
    let _ = Arc::new(());
}
