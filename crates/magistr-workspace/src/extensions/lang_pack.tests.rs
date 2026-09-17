use super::*;
use crate::{
    config::ResolveOptions,
    extensions::{ExtensionLane, LaneFuture, LangPackLoadOutcome, PluginGuestPolicy, PluginHealth},
    worker::{WorkerGuestEnd, WorkerLanguageLane, WorkerLanguageRegistration},
    workspace_policy::WorkspacePolicy,
};
use std::{fs, sync::Mutex};

const MANIFEST: &str = r#"
[plugin]
publisher = "workbench"
pkg = "lang-pack-gleam"
version = "0.1.0"

[component]
path = "guest.wasm"

[[tools]]
name = "classify-file"
description = "x"
"#;

struct Rig {
    dir: tempfile::TempDir,
    policy: WorkspacePolicy,
}

impl Rig {
    fn new(records: serde_json::Value, manifest: Option<&str>, component: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".workbench")).unwrap();
        fs::create_dir_all(root.join("plugins/lang-pack-gleam")).unwrap();
        if let Some(manifest) = manifest {
            fs::write(root.join("plugins/lang-pack-gleam/plugin.toml"), manifest).unwrap();
        }
        if component {
            fs::write(
                root.join("plugins/lang-pack-gleam/guest.wasm"),
                b"(component)",
            )
            .unwrap();
        }
        fs::write(
            root.join(".workbench/plugins.json"),
            serde_json::json!({ "plugins": records }).to_string(),
        )
        .unwrap();
        let policy = WorkspacePolicy::resolve(&ResolveOptions::new(root)).unwrap();
        Self { dir, policy }
    }

    fn record(tier: &str) -> serde_json::Value {
        serde_json::json!([{
            "name": "workbench.lang-pack-gleam",
            "source": "registry",
            "tier": tier,
            "dir": "plugins/lang-pack-gleam",
        }])
    }

    fn state_dir(&self) -> std::path::PathBuf {
        self.dir.path().join(".workbench")
    }

    fn host<'a>(&'a self, lane: &'a dyn ExtensionLane) -> PluginHost<'a> {
        PluginHost::new(&self.policy, self.state_dir(), lane)
            .with_clock(|| "2026-08-22T00:00:00Z".to_owned())
    }
}

struct FakeLoad {
    answer: Result<LangPackLoadOutcome, String>,
    loads: Mutex<Vec<(Vec<String>, PluginGuestPolicy, bool)>>,
}

impl FakeLoad {
    fn registered(lane: WorkerLanguageLane) -> Self {
        Self {
            answer: Ok(LangPackLoadOutcome {
                end: WorkerGuestEnd::Ok,
                fuel_used: 1,
                registration: Some(WorkerLanguageRegistration {
                    id: "gleam".into(),
                    extensions: vec!["gleam".into()],
                    lane,
                    queries_ok: true,
                    notes: vec!["lane note".into()],
                }),
            }),
            loads: Mutex::new(Vec::new()),
        }
    }

    fn ending(end: WorkerGuestEnd) -> Self {
        Self {
            answer: Ok(LangPackLoadOutcome {
                end,
                fuel_used: 1,
                registration: None,
            }),
            loads: Mutex::new(Vec::new()),
        }
    }

    fn failing(message: &str) -> Self {
        Self {
            answer: Err(message.to_owned()),
            loads: Mutex::new(Vec::new()),
        }
    }
}

impl ExtensionLane for FakeLoad {
    fn load_lang_pack(
        &self,
        _guest: Vec<u8>,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
        blessed: bool,
    ) -> LaneFuture<'_, LangPackLoadOutcome> {
        self.loads
            .lock()
            .unwrap()
            .push((capabilities, policy, blessed));
        let answer = self.answer.clone();
        Box::pin(async move {
            answer.map_err(|message| {
                crate::contracts::WorkspaceError::new(
                    crate::contracts::ErrorKind::EngineUnavailable,
                    message,
                )
            })
        })
    }
}

#[tokio::test]
async fn a_healthy_pack_boots_with_lane_provenance_and_the_tier_selects_the_lane() {
    let rig = Rig::new(Rig::record("verified"), Some(MANIFEST), true);
    let lane = FakeLoad::registered(WorkerLanguageLane::Wasm);
    let statuses = LangPackBoot::new(&rig.host(&lane)).boot().await;
    assert_eq!(statuses.len(), 1);
    let status = &statuses[0];
    assert!(status.ok);
    assert_eq!(status.language.as_deref(), Some("gleam"));
    assert_eq!(status.lane.as_deref(), Some("wasm"));
    assert_eq!(status.queries_ok, Some(true));
    assert_eq!(status.notes, vec!["lane note"]);
    {
        let loads = lane.loads.lock().unwrap();
        assert!(
            loads[0].0.is_empty(),
            "a lang pack declares zero capabilities"
        );
        assert!(!loads[0].2, "verified is not blessed");
    }

    let rig = Rig::new(Rig::record("blessed"), Some(MANIFEST), true);
    let lane = FakeLoad::registered(WorkerLanguageLane::Native);
    let statuses = LangPackBoot::new(&rig.host(&lane)).boot().await;
    assert_eq!(statuses[0].lane.as_deref(), Some("native"));
    assert!(lane.loads.lock().unwrap()[0].2);
}

#[tokio::test]
async fn non_lang_pack_plugins_are_not_touched_and_boot_refusals_are_status_rows() {
    let rig = Rig::new(
        serde_json::json!([{ "name": "workbench.lsp-resolver", "source": "registry" }]),
        Some(MANIFEST),
        true,
    );
    let lane = FakeLoad::failing("must not load");
    assert!(LangPackBoot::new(&rig.host(&lane)).boot().await.is_empty());
    assert!(lane.loads.lock().unwrap().is_empty());

    let rig = Rig::new(Rig::record("verified"), None, true);
    let statuses = LangPackBoot::new(&rig.host(&lane)).boot().await;
    assert!(!statuses[0].ok);
    assert_eq!(statuses[0].outcome, "manifest-unreadable");

    let rig = Rig::new(Rig::record("verified"), Some(MANIFEST), false);
    let statuses = LangPackBoot::new(&rig.host(&lane)).boot().await;
    assert_eq!(statuses[0].outcome, "missing-component");

    let rig = Rig::new(
        serde_json::json!([{ "name": "workbench.lang-pack-gleam", "source": "registry" }]),
        Some(MANIFEST),
        true,
    );
    let statuses = LangPackBoot::new(&rig.host(&lane)).boot().await;
    assert_eq!(statuses[0].outcome, "missing-component");
    assert!(statuses[0].detail.contains("inventory-only"));
    assert!(lane.loads.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_failing_pack_breaches_the_breaker_and_a_breached_pack_stays_quiet() {
    let rig = Rig::new(Rig::record("verified"), Some(MANIFEST), true);
    let trap = FakeLoad::ending(WorkerGuestEnd::GuestTrap("boom".into()));
    let statuses = LangPackBoot::new(&rig.host(&trap)).boot().await;
    assert!(!statuses[0].ok);
    assert_eq!(statuses[0].outcome, "guest-trap");
    assert_eq!(statuses[0].detail, "boom");
    let breaches = PluginHealth::new(&rig.state_dir()).read();
    assert_eq!(breaches.len(), 1);
    assert_eq!(breaches[0].reason, "boom");

    let healthy = FakeLoad::registered(WorkerLanguageLane::Wasm);
    let statuses = LangPackBoot::new(&rig.host(&healthy)).boot().await;
    assert_eq!(statuses[0].outcome, "breakered");
    assert!(healthy.loads.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_registration_refusal_is_a_status_row_and_a_breach() {
    let rig = Rig::new(Rig::record("verified"), Some(MANIFEST), true);
    let lane = FakeLoad::failing("pack language id `rust` shadows a bundled grammar; refused");
    let statuses = LangPackBoot::new(&rig.host(&lane)).boot().await;
    assert!(!statuses[0].ok);
    assert_eq!(statuses[0].outcome, "registration-refused");
    assert!(statuses[0].detail.contains("shadows a bundled grammar"));
    assert_eq!(PluginHealth::new(&rig.state_dir()).read().len(), 1);
    assert!(
        !serde_json::to_string(&statuses)
            .unwrap()
            .contains("\"queriesOk\"")
    );
}

#[test]
fn admission_captures_the_carrier_bytes_and_keeps_refusals_as_rows() {
    let rig = Rig::new(Rig::record("blessed"), Some(MANIFEST), true);
    let lane = FakeLoad::failing("admission never loads");
    let admission = LangPackBoot::new(&rig.host(&lane)).admit();
    assert!(lane.loads.lock().unwrap().is_empty());
    assert_eq!(admission.packs.len(), 1);
    assert!(admission.refused.is_empty());
    let pack = &admission.packs[0];
    assert_eq!(pack.plugin, "workbench.lang-pack-gleam");
    assert_eq!(pack.component.bytes, b"(component)");
    assert!(pack.blessed);
    assert!(pack.capabilities.is_empty());
    assert!(!admission.is_empty());

    let rig = Rig::new(Rig::record("verified"), Some(MANIFEST), false);
    let admission = LangPackBoot::new(&rig.host(&lane)).admit();
    assert!(admission.is_empty());
    assert_eq!(admission.refused.len(), 1);
    assert_eq!(admission.refused[0].outcome, "missing-component");
}
