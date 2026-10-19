use super::*;
use crate::{
    config::ResolveOptions,
    extensions::{
        ExtensionLane, LaneFuture, PluginHealth, PluginHealthRecord, ResolverLaneOutcome,
    },
    sandbox::CAP_SUBPROCESS,
    worker::{WorkerLocation, WorkerSubprocessSpec},
    workspace_policy::WorkspacePolicy,
};
use std::{fs, sync::Mutex, time::Duration};

const NOW: &str = "2026-08-22T12:00:00Z";
const NOW_SECS: u64 = 1_787_400_000;

/// A workspace on real disk — the jail canonicalizes against real files.
struct Rig {
    dir: tempfile::TempDir,
    policy: WorkspacePolicy,
}

impl Rig {
    fn new(record: Option<serde_json::Value>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".magistr/workspace")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/greet.ts"), "export function greet() {}\n").unwrap();
        fs::write(root.join("src/use.ts"), "\n\ngreet();\n").unwrap();
        fs::create_dir_all(root.join("plugins/lsp-resolver")).unwrap();
        fs::write(
            root.join("plugins/lsp-resolver/plugin.toml"),
            include_str!("../../fixtures/plugins/lsp-resolver/plugin.toml"),
        )
        .unwrap();
        fs::write(root.join("plugins/lsp-resolver/guest.wasm"), b"(component)").unwrap();
        let plugins = record.map_or_else(Vec::new, |record| vec![record]);
        fs::write(
            root.join(".magistr/workspace/plugins.json"),
            serde_json::json!({ "plugins": plugins }).to_string(),
        )
        .unwrap();
        let policy = WorkspacePolicy::resolve(&ResolveOptions::new(root)).unwrap();
        Self { dir, policy }
    }

    fn blessed() -> serde_json::Value {
        serde_json::json!({
            "name": PluginResolver::PLUGIN_NAME,
            "source": "registry",
            "tier": "blessed",
            "dir": "plugins/lsp-resolver",
            "grants": [CAP_SUBPROCESS],
        })
    }

    fn state_dir(&self) -> std::path::PathBuf {
        self.dir.path().join(".magistr/workspace")
    }

    fn seed_breach(&self, age_secs: u64) {
        PluginHealth::new(&self.state_dir())
            .append(PluginHealthRecord {
                plugin: PluginResolver::PLUGIN_NAME.into(),
                component_hash: PluginComponent::read(
                    &self.dir.path().join("plugins/lsp-resolver/guest.wasm"),
                )
                .unwrap()
                .sha256,
                reason: "seeded".into(),
                at: HostClock::format(NOW_SECS - age_secs),
            })
            .unwrap();
    }

    fn breaches(&self) -> Vec<PluginHealthRecord> {
        PluginHealth::new(&self.state_dir()).read()
    }

    fn host<'a>(&'a self, lane: &'a dyn ExtensionLane) -> PluginHost<'a> {
        PluginHost::new(&self.policy, self.state_dir(), lane).with_clock(|| NOW.to_owned())
    }
}

fn spec() -> LanguageServerSpec {
    LanguageServerSpec {
        language_id: "fake".into(),
        bin: "/fake-lsp".into(),
        args: vec!["--stdio".into()],
    }
}

fn ok_outcome(locations: Vec<(&str, u32, u32)>) -> ResolverLaneOutcome {
    ResolverLaneOutcome {
        end: WorkerGuestEnd::Ok,
        fuel_used: 1000,
        locations: locations
            .into_iter()
            .map(|(path, line, column)| WorkerLocation {
                path: path.into(),
                line,
                column,
            })
            .collect(),
        sessions_opened: 1,
        sessions_reused: 0,
    }
}

/// (symbol, capabilities, specs, root) as the lane received them.
type Invoke = (String, Vec<String>, Vec<WorkerSubprocessSpec>, String);

struct FakeResolve {
    answer: Mutex<Option<ResolverLaneOutcome>>,
    hang: bool,
    invokes: Mutex<Vec<Invoke>>,
}

impl FakeResolve {
    fn answering(outcome: ResolverLaneOutcome) -> Self {
        Self {
            answer: Mutex::new(Some(outcome)),
            hang: false,
            invokes: Mutex::new(Vec::new()),
        }
    }

    fn hanging() -> Self {
        Self {
            answer: Mutex::new(None),
            hang: true,
            invokes: Mutex::new(Vec::new()),
        }
    }

    fn symbols(&self) -> Vec<String> {
        self.invokes
            .lock()
            .unwrap()
            .iter()
            .map(|i| i.0.clone())
            .collect()
    }
}

impl ExtensionLane for FakeResolve {
    fn resolve_references(
        &self,
        _guest: Vec<u8>,
        symbol: String,
        capabilities: Vec<String>,
        _policy: PluginGuestPolicy,
        specs: Vec<WorkerSubprocessSpec>,
        root: String,
    ) -> LaneFuture<'_, ResolverLaneOutcome> {
        self.invokes
            .lock()
            .unwrap()
            .push((symbol, capabilities, specs, root));
        let answer = self.answer.lock().unwrap().clone();
        let hang = self.hang;
        Box::pin(async move {
            if hang {
                std::future::pending::<()>().await;
            }
            answer.ok_or_else(|| {
                crate::contracts::WorkspaceError::new(
                    crate::contracts::ErrorKind::EngineUnavailable,
                    "lane down",
                )
            })
        })
    }
}

async fn resolve(
    rig: &Rig,
    lane: &FakeResolve,
    specs: Vec<LanguageServerSpec>,
    warm: bool,
) -> Vec<ResolvedEdge> {
    let host = rig.host(lane);
    PluginResolver::new(&host, rig.dir.path().display().to_string(), specs)
        .with_budget(Duration::from_millis(25))
        .resolve_references("greet", warm)
        .await
}

#[tokio::test]
async fn a_healthy_blessed_plugin_upgrades_references_with_admitted_relative_edges() {
    let rig = Rig::new(Some(Rig::blessed()));
    let lane = FakeResolve::answering(ok_outcome(vec![("src/greet.ts", 1, 17)]));
    let edges = resolve(&rig, &lane, vec![spec()], true).await;
    assert_eq!(
        edges,
        vec![ResolvedEdge {
            path: "src/greet.ts".into(),
            line: 1,
            column: 17
        }]
    );
    let invokes = lane.invokes.lock().unwrap();
    assert_eq!(invokes[0].0, "greet");
    assert_eq!(invokes[0].1, vec![CAP_SUBPROCESS]);
    assert_eq!(invokes[0].2[0].language_id, "fake");
    assert!(
        invokes[0]
            .3
            .ends_with(rig.dir.path().file_name().unwrap().to_str().unwrap())
    );
    assert!(rig.breaches().is_empty());
}

#[tokio::test]
async fn gating_never_invokes_below_tier_one_without_specs_below_blessed_or_ungranted() {
    let rig = Rig::new(Some(Rig::blessed()));
    let lane = FakeResolve::answering(ok_outcome(vec![("src/greet.ts", 1, 17)]));
    assert!(resolve(&rig, &lane, vec![spec()], false).await.is_empty());
    assert!(resolve(&rig, &lane, Vec::new(), true).await.is_empty());
    assert!(lane.symbols().is_empty());

    let mut verified = Rig::blessed();
    verified["tier"] = serde_json::json!("verified");
    let rig = Rig::new(Some(verified));
    assert!(resolve(&rig, &lane, vec![spec()], true).await.is_empty());
    let mut ungranted = Rig::blessed();
    ungranted["grants"] = serde_json::json!([]);
    let rig = Rig::new(Some(ungranted));
    assert!(resolve(&rig, &lane, vec![spec()], true).await.is_empty());
    let rig = Rig::new(None);
    assert!(resolve(&rig, &lane, vec![spec()], true).await.is_empty());
    assert!(lane.symbols().is_empty());
}

#[tokio::test]
async fn a_fresh_breach_short_circuits_and_an_expired_one_lazily_retries() {
    let rig = Rig::new(Some(Rig::blessed()));
    rig.seed_breach(1);
    let lane = FakeResolve::answering(ok_outcome(vec![("src/greet.ts", 1, 17)]));
    assert!(resolve(&rig, &lane, vec![spec()], true).await.is_empty());
    assert!(lane.symbols().is_empty());

    let rig = Rig::new(Some(Rig::blessed()));
    rig.seed_breach(61);
    assert_eq!(resolve(&rig, &lane, vec![spec()], true).await.len(), 1);
    assert_eq!(lane.symbols(), vec!["greet"]);
}

#[tokio::test]
async fn crashes_budget_and_malformed_locations_degrade_and_breach_but_cap_denied_does_not() {
    let rig = Rig::new(Some(Rig::blessed()));
    let trap = FakeResolve::answering(ResolverLaneOutcome {
        end: WorkerGuestEnd::GuestTrap("unreachable".into()),
        fuel_used: 5,
        locations: Vec::new(),
        sessions_opened: 0,
        sessions_reused: 0,
    });
    assert!(resolve(&rig, &trap, vec![spec()], true).await.is_empty());
    assert!(rig.breaches()[0].reason.contains("guest-trap"));

    let rig = Rig::new(Some(Rig::blessed()));
    let denied = FakeResolve::answering(ResolverLaneOutcome {
        end: WorkerGuestEnd::CapDenied(CAP_SUBPROCESS.into()),
        fuel_used: 0,
        locations: Vec::new(),
        sessions_opened: 0,
        sessions_reused: 0,
    });
    assert!(resolve(&rig, &denied, vec![spec()], true).await.is_empty());
    assert!(rig.breaches().is_empty());

    let rig = Rig::new(Some(Rig::blessed()));
    let hanging = FakeResolve::hanging();
    assert!(resolve(&rig, &hanging, vec![spec()], true).await.is_empty());
    assert!(rig.breaches()[0].reason.contains("budget"));

    let rig = Rig::new(Some(Rig::blessed()));
    let escaping = FakeResolve::answering(ok_outcome(vec![
        ("src/greet.ts", 1, 17),
        ("../../outside.ts", 1, 1),
    ]));
    assert!(
        resolve(&rig, &escaping, vec![spec()], true)
            .await
            .is_empty()
    );
    assert!(
        rig.breaches()[0]
            .reason
            .contains("malformed resolver location")
    );

    let rig = Rig::new(Some(Rig::blessed()));
    let zero = FakeResolve::answering(ok_outcome(vec![("src/greet.ts", 0, 17)]));
    assert!(resolve(&rig, &zero, vec![spec()], true).await.is_empty());
    assert_eq!(rig.breaches().len(), 1);

    // A dead lane degrades service without blaming the plugin.
    let rig = Rig::new(Some(Rig::blessed()));
    let dead = FakeResolve {
        answer: Mutex::new(None),
        hang: false,
        invokes: Mutex::new(Vec::new()),
    };
    assert!(resolve(&rig, &dead, vec![spec()], true).await.is_empty());
    assert!(rig.breaches().is_empty());
}
