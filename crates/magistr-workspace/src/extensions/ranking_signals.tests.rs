use super::*;
use crate::{
    config::ResolveOptions,
    extensions::{LaneFuture, RankingContributeOutcome},
    query::SearchGroup,
};
use std::{fs, sync::Mutex, time::Duration};

const REAL_MANIFEST: &str = include_str!(
    "../../../../subsystems/workspace/reference/plugins/ranking-visibility/plugin.toml"
);

fn hit(file: &str, excerpt: &str, kind_class: &str) -> WorkerRankingHit {
    WorkerRankingHit {
        file: file.into(),
        line: 1,
        column: 1,
        excerpt: excerpt.into(),
        kind_class: kind_class.into(),
        file_class: "source".into(),
    }
}

fn decl_hits() -> Vec<WorkerRankingHit> {
    vec![
        hit("src/a-internal.ts", "function needle() {", "declaration"),
        hit(
            "src/z-exported.ts",
            "export function needle() {",
            "declaration",
        ),
    ]
}

/// The fake guest: exported=1.0 for export/pub-leading declarations.
fn fake_contribute(hits: &[WorkerRankingHit]) -> RankingContributeOutcome {
    RankingContributeOutcome {
        end: WorkerGuestEnd::Ok,
        fuel_used: 1,
        contributions: hits
            .iter()
            .map(|hit| {
                if hit.kind_class == "declaration" {
                    let exported = ["export", "pub", "public"]
                        .iter()
                        .any(|marker| hit.excerpt.trim_start().starts_with(marker));
                    vec![WorkerRankingFeature {
                        name: "exported".into(),
                        value: if exported { 1.0 } else { 0.0 },
                    }]
                } else {
                    Vec::new()
                }
            })
            .collect(),
    }
}

struct FakeContribute {
    mode: Mode,
    calls: Mutex<Vec<Vec<WorkerRankingHit>>>,
    gate: Option<tokio::sync::Notify>,
}

enum Mode {
    Healthy,
    Exploding,
}

impl ExtensionLane for FakeContribute {
    fn contribute_ranking(
        &self,
        _guest: Vec<u8>,
        hits: Vec<WorkerRankingHit>,
        _capabilities: Vec<String>,
        _policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, RankingContributeOutcome> {
        self.calls.lock().unwrap().push(hits.clone());
        Box::pin(async move {
            if let Some(gate) = &self.gate {
                gate.notified().await;
            }
            match self.mode {
                Mode::Healthy => Ok(fake_contribute(&hits)),
                Mode::Exploding => Err(crate::contracts::WorkspaceError::new(
                    crate::contracts::ErrorKind::EngineUnavailable,
                    "guest exploded",
                )),
            }
        })
    }
}

struct Env {
    dir: tempfile::TempDir,
    policy: WorkspacePolicy,
    lane: Arc<FakeContribute>,
}

impl Env {
    fn new(tier: Option<&str>, mode: Mode, gated: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".workbench")).unwrap();
        fs::create_dir_all(root.join("plugins/ranking-visibility")).unwrap();
        fs::write(
            root.join("plugins/ranking-visibility/plugin.toml"),
            REAL_MANIFEST,
        )
        .unwrap();
        fs::write(
            root.join("plugins/ranking-visibility/guest.wasm"),
            b"(component v1)",
        )
        .unwrap();
        let mut record = serde_json::json!({
            "name": "workbench.ranking-visibility",
            "source": "registry",
            "dir": "plugins/ranking-visibility",
        });
        if let Some(tier) = tier {
            record["tier"] = serde_json::json!(tier);
        } else {
            record["source"] = serde_json::json!("sideload");
        }
        fs::write(
            root.join(".workbench/plugins.json"),
            serde_json::json!({ "plugins": [record] }).to_string(),
        )
        .unwrap();
        let policy = WorkspacePolicy::resolve(&ResolveOptions::new(root)).unwrap();
        Self {
            dir,
            policy,
            lane: Arc::new(FakeContribute {
                mode,
                calls: Mutex::new(Vec::new()),
                gate: gated.then(tokio::sync::Notify::new),
            }),
        }
    }

    fn state_dir(&self) -> std::path::PathBuf {
        self.dir.path().join(".workbench")
    }

    fn runtime(&self) -> RankingSignals {
        let signals = RankingSignals::new(&self.state_dir(), self.lane.clone())
            .with_clock(|| "2026-08-22T00:00:00Z".to_owned());
        signals.boot(&self.policy, &self.state_dir());
        signals
    }

    fn shadow_lines(&self) -> Vec<serde_json::Value> {
        fs::read_to_string(self.state_dir().join("ranking-shadow.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn promotions(&self) -> Option<String> {
        fs::read_to_string(self.state_dir().join("ranking-promotions.json")).ok()
    }

    fn rewrite_component(&self) {
        fs::write(
            self.dir
                .path()
                .join("plugins/ranking-visibility/guest.wasm"),
            b"(component v2)",
        )
        .unwrap();
    }
}

fn served(lookup: Option<&crate::query::PluginFeatureLookup>) -> Vec<String> {
    // Two declaration groups tie without plugin features; z sorts last.
    let boost =
        |file: &str, excerpt: &str| lookup.map_or(0.0, |lookup| lookup.boost(file, 1, 1, excerpt));
    let mut files = vec![
        (
            "src/a-internal.ts",
            boost("src/a-internal.ts", "function needle() {"),
        ),
        (
            "src/z-exported.ts",
            boost("src/z-exported.ts", "export function needle() {"),
        ),
    ];
    files.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    files.into_iter().map(|(file, _)| file.to_owned()).collect()
}

async fn until(mut check: impl FnMut() -> bool) {
    for _ in 0..400 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition never settled");
}

#[test]
fn contributions_are_namespaced_per_plugin_and_malformed_values_dropped() {
    let raw = vec![
        WorkerRankingFeature {
            name: "exported".into(),
            value: 1.0,
        },
        WorkerRankingFeature {
            name: "other-plugin/spoof".into(),
            value: 1.0,
        },
        WorkerRankingFeature {
            name: String::new(),
            value: 1.0,
        },
        WorkerRankingFeature {
            name: "nan".into(),
            value: f64::NAN,
        },
        WorkerRankingFeature {
            name: "inf".into(),
            value: f64::INFINITY,
        },
        WorkerRankingFeature {
            name: "huge".into(),
            value: 1e9,
        },
        WorkerRankingFeature {
            name: "negative".into(),
            value: -0.25,
        },
    ];
    assert_eq!(
        sanitize_contributions("ranking-visibility", &raw),
        vec![
            PluginFeature {
                name: "ranking-visibility/exported".into(),
                value: 1.0
            },
            PluginFeature {
                name: "ranking-visibility/negative".into(),
                value: -0.25
            },
        ]
    );
}

#[tokio::test]
async fn a_fresh_install_serves_the_baseline_and_accumulates_the_shadow_log() {
    let env = Env::new(None, Mode::Healthy, false);
    let runtime = env.runtime();
    let statuses = runtime.statuses();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].tier, "sideload");
    assert!(statuses[0].healthy && !statuses[0].promoted);
    assert!(runtime.promoted_features().is_none());
    let baseline = served(None);
    runtime.after_search("needle", decl_hits(), baseline.clone());
    until(|| env.shadow_lines().len() == 1).await;
    let entry = &env.shadow_lines()[0];
    assert_eq!(entry["applied"], serde_json::json!([]));
    assert_eq!(entry["servedOrder"], serde_json::json!(baseline));
    assert_eq!(entry["queryHash"].as_str().unwrap().len(), 16);
    let rows = entry["contributions"]["ranking-visibility"]
        .as_object()
        .unwrap();
    assert!(rows.values().any(|features| {
        features
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["name"] == "ranking-visibility/exported" && f["value"] == 1.0)
    }));
    // Shadow NEVER applies: even with a warm cache.
    assert!(runtime.promoted_features().is_none());
    assert_eq!(runtime.statuses()[0].cached_hits, 2);
    assert!(runtime.status_notes()[0].contains("shadow"));
}

#[tokio::test]
async fn promotion_persists_survives_restart_applies_from_the_warm_cache_and_demotes() {
    let env = Env::new(Some("verified"), Mode::Healthy, false);
    let runtime = env.runtime();
    runtime
        .score_and_log("needle", decl_hits(), Vec::new())
        .await;
    assert_eq!(
        runtime.promote("ranking-visibility").unwrap(),
        "workbench.ranking-visibility"
    );
    let text = env.promotions().unwrap();
    assert!(text.contains("\"promoted\": true"));
    let lookup = runtime.promoted_features().expect("warm promoted cache");
    assert_eq!(
        served(Some(&lookup)),
        vec!["src/z-exported.ts", "src/a-internal.ts"]
    );
    // Replaying the logged contributions reproduces the applied order.
    let entry = &env.shadow_lines()[0];
    let mut replay = crate::query::PluginFeatureLookup::default();
    for (key, features) in entry["contributions"]["ranking-visibility"]
        .as_object()
        .unwrap()
    {
        replay.insert(
            key.clone(),
            serde_json::from_value(features.clone()).unwrap(),
        );
    }
    assert_eq!(served(Some(&replay)), served(Some(&lookup)));

    // Restart: promotion survives; the cache starts cold so the baseline serves.
    let restarted = env.runtime();
    assert!(restarted.statuses()[0].promoted);
    assert!(restarted.promoted_features().is_none());
    restarted.after_search("needle", decl_hits(), Vec::new());
    until(|| restarted.promoted_features().is_some()).await;
    assert!(restarted.status_notes()[0].contains("PROMOTED"));

    assert!(runtime.demote("ranking-visibility").is_ok());
    assert!(runtime.promoted_features().is_none());
    assert!(
        runtime
            .promote("ranking-nonexistent")
            .unwrap_err()
            .contains("unknown ranking signal")
    );
}

#[tokio::test]
async fn a_failing_signal_breaches_degrades_to_baseline_and_goes_quiet() {
    let env = Env::new(Some("verified"), Mode::Exploding, false);
    let runtime = env.runtime();
    runtime.promote("ranking-visibility").unwrap();
    runtime
        .score_and_log("needle", decl_hits(), Vec::new())
        .await;
    let breaches = PluginHealth::new(&env.state_dir()).read();
    assert_eq!(breaches.len(), 1);
    assert!(breaches[0].reason.contains("guest exploded"));
    assert!(runtime.promoted_features().is_none());
    assert!(!runtime.statuses()[0].healthy);
    runtime.after_search("needle", decl_hits(), Vec::new());
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(env.lane.calls.lock().unwrap().len(), 1);
    // A rebooted runtime sees the durable breaker.
    assert_eq!(env.runtime().statuses()[0].outcome, "breakered");
}

#[tokio::test]
async fn a_changed_component_hash_resets_promotion_to_shadow() {
    let env = Env::new(Some("verified"), Mode::Healthy, false);
    let runtime = env.runtime();
    runtime.promote("ranking-visibility").unwrap();
    env.rewrite_component();
    let changed = env.runtime();
    assert!(!changed.statuses()[0].promoted);
    changed
        .score_and_log("needle", decl_hits(), Vec::new())
        .await;
    assert!(changed.promoted_features().is_none());
}

#[tokio::test]
async fn the_response_path_never_awaits_the_sandbox_and_the_log_cap_evicts_oldest() {
    let env = Env::new(Some("verified"), Mode::Healthy, true);
    let runtime = env.runtime();
    runtime.after_search("needle", decl_hits(), Vec::new());
    until(|| env.lane.calls.lock().unwrap().len() == 1).await;
    // The call is pending on the gate; the caller already returned.
    assert!(env.shadow_lines().is_empty());
    env.lane.gate.as_ref().unwrap().notify_one();
    until(|| env.shadow_lines().len() == 1).await;

    let filler = format!("{}\n", serde_json::json!({"pad": "x".repeat(1024)}));
    let mut text = String::new();
    while text.len() < 5 * 1024 * 1024 - 512 {
        text.push_str(&filler);
    }
    fs::write(env.state_dir().join("ranking-shadow.jsonl"), &text).unwrap();
    let before = text.lines().count();
    runtime.after_search("needle", decl_hits(), Vec::new());
    env.lane.gate.as_ref().unwrap().notify_one();
    until(|| env.shadow_lines().len() < before).await;
    let lines = env.shadow_lines();
    assert!(lines.last().unwrap().get("queryHash").is_some());
}

#[test]
fn ranking_hit_contexts_carry_stage_classes_and_a_cap() {
    use crate::query::SearchHit;
    let hit = |line: u64, excerpt: &str, kind: Option<&str>| SearchHit {
        line,
        column: 1,
        excerpt: excerpt.into(),
        kind: kind.map(str::to_owned),
        confidence: None,
        provenance: None,
        duplicates: None,
        context_before: None,
        context_after: None,
        in_test_range: None,
    };
    let groups = vec![
        SearchGroup {
            file: "src/a.ts".into(),
            junk: false,
            total_hits: 2,
            hits: vec![
                hit(1, "export function needle() {", Some("function")),
                hit(3, "needle();", None),
            ],
        },
        SearchGroup {
            file: "vendor/x.js".into(),
            junk: true,
            total_hits: 1,
            hits: vec![hit(1, "needle", None)],
        },
    ];
    let contexts = crate::query::hit_contexts_for_tests("needle", &groups, 10);
    assert_eq!(contexts.len(), 3);
    assert_eq!(contexts[0].kind_class, "declaration");
    assert_eq!(contexts[0].file_class, "source");
    assert_eq!(contexts[1].kind_class, "call");
    assert_eq!(contexts[2].file_class, "vendor");
    assert_eq!(
        crate::query::hit_contexts_for_tests("needle", &groups, 1).len(),
        1
    );
}
