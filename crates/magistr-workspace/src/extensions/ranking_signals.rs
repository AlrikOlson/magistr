use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{
    EffectiveCapabilities, ExtensionLane, HostClock, PluginComponent, PluginGuestPolicy,
    PluginHealth, PluginHealthRecord, PluginManifest, RankingSignalStatus, discovery::Discovery,
};
use crate::{
    query::{PluginFeature, PluginFeatureLookup},
    worker::{WorkerGuestEnd, WorkerRankingFeature, WorkerRankingHit},
    workspace_policy::WorkspacePolicy,
};

/// Reject |value| beyond this at the host boundary.
const MAX_FEATURE_MAGNITUDE: f64 = 1e6;
/// Feature-cache entries kept per signal.
const CACHE_CAPACITY: usize = 50_000;
/// Shadow-log size cap; oldest entries evicted on breach.
const SHADOW_LOG_CAP_BYTES: u64 = 5 * 1024 * 1024;

/// The ranking-signal runtime: discovery of `ranking-` plugins, the shared
/// breaker, the persisted per-signal promotion store, the size-capped shadow
/// log, the in-memory feature cache and the post-response shadow scorer.
/// The sandbox is never on the search response path: shadow scoring is
/// fire-and-forget after ranking, and promoted signals apply only through
/// the cache's synchronous lookup — a miss contributes zero.
///
/// Promotion is a human decision. The persisted store is the user surface;
/// `promote`/`demote` exist for that surface and for tests. Promotion binds
/// to the exact component hash — new bytes re-earn promotion in shadow.
pub struct RankingSignals {
    inner: Arc<Inner>,
}

struct Inner {
    lane: Arc<dyn ExtensionLane>,
    health: PluginHealth,
    promotions_path: PathBuf,
    shadow_log_path: PathBuf,
    clock: Box<dyn Fn() -> String + Send + Sync>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    signals: Vec<Signal>,
    boot_statuses: Vec<RankingSignalStatus>,
    promotions: HashMap<String, Promotion>,
    caches: HashMap<String, FeatureCache>,
    merged: PluginFeatureLookup,
}

struct Signal {
    plugin: String,
    pkg: String,
    tier: String,
    bytes: Arc<Vec<u8>>,
    sha256: String,
    capabilities: Vec<String>,
    policy: PluginGuestPolicy,
    healthy: bool,
    outcome: String,
    detail: String,
}

#[derive(Clone)]
struct Promotion {
    component_hash: String,
    promoted: bool,
    at: String,
}

#[derive(Default)]
struct FeatureCache {
    order: VecDeque<String>,
    entries: HashMap<String, Vec<PluginFeature>>,
}

impl FeatureCache {
    fn store(&mut self, key: String, features: Vec<PluginFeature>) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, features);
        while self.entries.len() > CACHE_CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

/// Namespace and validate one guest's contributions for one hit. Malformed
/// entries are dropped per-contribution; a spoofed namespace never passes.
#[must_use]
pub(crate) fn sanitize_contributions(
    pkg: &str,
    features: &[WorkerRankingFeature],
) -> Vec<PluginFeature> {
    features
        .iter()
        .filter(|feature| {
            !feature.name.is_empty()
                && !feature.name.contains('/')
                && feature.value.is_finite()
                && feature.value.abs() <= MAX_FEATURE_MAGNITUDE
        })
        .map(|feature| PluginFeature {
            name: format!("{pkg}/{}", feature.name),
            value: feature.value,
        })
        .collect()
}

fn hit_key(hit: &WorkerRankingHit) -> String {
    PluginFeatureLookup::key(&hit.file, u64::from(hit.line), hit.column, &hit.excerpt)
}

impl RankingSignals {
    /// Inventory pkg-name convention marking a plugin as a ranking signal.
    pub const PKG_PREFIX: &'static str = "ranking-";

    /// `state_dir` is the admitted `.magistr/workspace` directory.
    #[must_use]
    pub fn new(state_dir: &Path, lane: Arc<dyn ExtensionLane>) -> Self {
        Self {
            inner: Arc::new(Inner {
                lane,
                health: PluginHealth::new(state_dir),
                promotions_path: state_dir.join("ranking-promotions.json"),
                shadow_log_path: state_dir.join("ranking-shadow.jsonl"),
                clock: Box::new(HostClock::now),
                state: Mutex::new(State::default()),
            }),
        }
    }

    #[must_use]
    pub fn with_clock(self, clock: impl Fn() -> String + Send + Sync + 'static) -> Self {
        let inner = Arc::try_unwrap(self.inner).unwrap_or_else(|_| {
            unreachable!("with_clock is called before any clone of the runtime")
        });
        Self {
            inner: Arc::new(Inner {
                clock: Box::new(clock),
                ..inner
            }),
        }
    }

    /// Discover, breaker-check and load promotion state. Never fails; every
    /// signal answers a status row. Cold caches keep the baseline until the
    /// first shadow pass warms them.
    pub fn boot(&self, policy: &WorkspacePolicy, state_dir: &Path) -> Vec<RankingSignalStatus> {
        let discovery = Discovery {
            policy,
            state_dir: state_dir.to_path_buf(),
        };
        let Ok(inventory) = discovery.inventory() else {
            return Vec::new();
        };
        self.load_promotions();
        let mut statuses = Vec::new();
        for record in inventory {
            if !record.pkg().starts_with(Self::PKG_PREFIX) {
                continue;
            }
            let tier = record
                .tier
                .map_or("sideload", |tier| tier.as_str())
                .to_owned();
            let base = |outcome: &str, detail: String| RankingSignalStatus {
                plugin: record.name.clone(),
                tier: tier.clone(),
                promoted: false,
                healthy: false,
                outcome: outcome.into(),
                detail,
                cached_hits: 0,
            };
            let Some(dir) = record.dir.as_deref() else {
                statuses.push(base(
                    "missing-component",
                    "inventory-only record: no plugin directory".into(),
                ));
                continue;
            };
            let manifest = match discovery
                .read_manifest(dir)
                .map_err(|e| vec![e])
                .and_then(|s| PluginManifest::parse(&s).map(|parsed| parsed.manifest))
            {
                Ok(manifest) => manifest,
                Err(errors) => {
                    statuses.push(base("manifest-unreadable", errors.join("; ")));
                    continue;
                }
            };
            let Some(component) = PluginComponent::resolve(policy, dir, &manifest.component_path)
            else {
                statuses.push(base(
                    "missing-component",
                    format!(
                        "component {} is missing or outside the workspace",
                        manifest.component_path
                    ),
                ));
                continue;
            };
            if let Some(breach) = self.inner.health.breach(&record.name, &component.sha256) {
                statuses.push(base(
                    "breakered",
                    format!(
                        "unhealthy since {} ({}); a changed component clears the breaker",
                        breach.at, breach.reason
                    ),
                ));
                continue;
            }
            let promoted = self.is_promoted(&record.name, &component.sha256);
            let mut state = self.lock();
            state.signals.push(Signal {
                plugin: record.name.clone(),
                pkg: record.pkg().to_owned(),
                tier: tier.clone(),
                bytes: Arc::new(component.bytes),
                sha256: component.sha256,
                capabilities: EffectiveCapabilities::resolve(&record, &manifest).effective,
                policy: PluginGuestPolicy::for_manifest(&manifest),
                healthy: true,
                outcome: "ok".into(),
                detail: String::new(),
            });
            drop(state);
            statuses.push(RankingSignalStatus {
                promoted,
                healthy: true,
                ..base("ok", String::new())
            });
        }
        let mut state = self.lock();
        state.boot_statuses = statuses.clone();
        rebuild_merged(&mut state);
        statuses
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn load_promotions(&self) {
        let Ok(raw) = std::fs::read_to_string(&self.inner.promotions_path) else {
            return;
        };
        // A corrupt store means every signal is shadow — the safe default.
        let Ok(parsed) = serde_json::from_str::<Value>(&raw) else {
            return;
        };
        let mut state = self.lock();
        for entry in parsed
            .get("promotions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (Some(name), Some(hash)) = (
                entry.get("name").and_then(Value::as_str),
                entry.get("componentHash").and_then(Value::as_str),
            ) else {
                continue;
            };
            state.promotions.insert(
                name.to_owned(),
                Promotion {
                    component_hash: hash.to_owned(),
                    promoted: entry.get("promoted").and_then(Value::as_bool) == Some(true),
                    at: entry
                        .get("at")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                },
            );
        }
    }

    /// Promotion binds to the exact artifact: hash mismatch reads as shadow.
    fn is_promoted(&self, plugin: &str, sha256: &str) -> bool {
        self.lock()
            .promotions
            .get(plugin)
            .is_some_and(|record| record.promoted && record.component_hash == sha256)
    }

    /// Promote one installed signal (explicit user action). Unknown signals
    /// are refused — persisted state never changes for them.
    pub fn promote(&self, name: &str) -> Result<String, String> {
        self.set_promotion(name, true)
    }

    /// Demote one signal back to shadow.
    pub fn demote(&self, name: &str) -> Result<String, String> {
        self.set_promotion(name, false)
    }

    fn set_promotion(&self, name: &str, promoted: bool) -> Result<String, String> {
        let mut state = self.lock();
        let Some((plugin, sha256)) = state
            .signals
            .iter()
            .find(|s| s.plugin == name || s.pkg == name)
            .map(|s| (s.plugin.clone(), s.sha256.clone()))
        else {
            return Err(format!("unknown ranking signal {name:?}"));
        };
        state.promotions.insert(
            plugin.clone(),
            Promotion {
                component_hash: sha256,
                promoted,
                at: (self.inner.clock)(),
            },
        );
        let promotions: Vec<Value> = state
            .promotions
            .iter()
            .map(|(name, record)| {
                json!({"name": name, "componentHash": record.component_hash, "promoted": record.promoted, "at": record.at})
            })
            .collect();
        let text = serde_json::to_string_pretty(&json!({ "promotions": promotions }))
            .map_err(|e| format!("promotion not persisted: {e}"))?;
        std::fs::write(&self.inner.promotions_path, format!("{text}\n"))
            .map_err(|e| format!("promotion not persisted: {e}"))?;
        rebuild_merged(&mut state);
        Ok(plugin)
    }

    /// The promoted-lane lookup: a synchronous snapshot, never an await.
    /// `None` when nothing is promoted, healthy and warm — the scorer then
    /// runs the exact baseline path.
    #[must_use]
    pub fn promoted_features(&self) -> Option<PluginFeatureLookup> {
        let state = self.lock();
        (!state.merged.is_empty()).then(|| state.merged.clone())
    }

    /// The shadow-lane hook: schedules batched scoring + logging on the
    /// current runtime and returns immediately. Nothing awaits it.
    pub fn after_search(
        &self,
        query: &str,
        hits: Vec<WorkerRankingHit>,
        served_order: Vec<String>,
    ) {
        if hits.is_empty() || !self.lock().signals.iter().any(|s| s.healthy) {
            return;
        }
        let inner = Arc::clone(&self.inner);
        let query = query.to_owned();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                score_and_log(&inner, &query, hits, served_order).await;
            });
        }
    }

    /// The shadow pass itself, awaitable (tests and deterministic callers).
    pub async fn score_and_log(
        &self,
        query: &str,
        hits: Vec<WorkerRankingHit>,
        served_order: Vec<String>,
    ) {
        score_and_log(&self.inner, query, hits, served_order).await;
    }

    /// Per-signal state rows.
    #[must_use]
    pub fn statuses(&self) -> Vec<RankingSignalStatus> {
        let state = self.lock();
        if state.signals.is_empty() {
            return state.boot_statuses.clone();
        }
        state
            .boot_statuses
            .iter()
            .filter(|status| !state.signals.iter().any(|s| s.plugin == status.plugin))
            .cloned()
            .chain(state.signals.iter().map(|s| RankingSignalStatus {
                plugin: s.plugin.clone(),
                tier: s.tier.clone(),
                promoted: promoted_in(&state, s),
                healthy: s.healthy,
                outcome: s.outcome.clone(),
                detail: s.detail.clone(),
                cached_hits: state.caches.get(&s.pkg).map_or(0, |c| c.entries.len()),
            }))
            .collect()
    }

    /// Status rows as schema-free notes.
    #[must_use]
    pub fn status_notes(&self) -> Vec<String> {
        self.statuses()
            .iter()
            .map(|s| {
                format!(
                    "ranking signal {}: tier {}, {}, {}, {} cached hit(s)",
                    s.plugin,
                    s.tier,
                    if s.promoted { "PROMOTED" } else { "shadow" },
                    if s.healthy {
                        "healthy".to_owned()
                    } else {
                        format!("unhealthy ({}: {})", s.outcome, s.detail)
                    },
                    s.cached_hits
                )
            })
            .collect()
    }
}

fn promoted_in(state: &State, signal: &Signal) -> bool {
    state
        .promotions
        .get(&signal.plugin)
        .is_some_and(|record| record.promoted && record.component_hash == signal.sha256)
}

/// Rebuild the merged promoted∩healthy view (rare: boot, promote, breach).
fn rebuild_merged(state: &mut State) {
    let mut merged = PluginFeatureLookup::default();
    for signal in &state.signals {
        if !signal.healthy || !promoted_in(state, signal) {
            continue;
        }
        if let Some(cache) = state.caches.get(&signal.pkg) {
            for (key, features) in &cache.entries {
                merged.insert(key.clone(), features.clone());
            }
        }
    }
    state.merged = merged;
}

fn breach(inner: &Inner, state: &mut State, index: usize, reason: &str) {
    let signal = &mut state.signals[index];
    signal.healthy = false;
    signal.outcome = "breakered".into();
    signal.detail = reason.to_owned();
    let record = PluginHealthRecord {
        plugin: signal.plugin.clone(),
        component_hash: signal.sha256.clone(),
        reason: reason.to_owned(),
        at: (inner.clock)(),
    };
    rebuild_merged(state);
    // Health-record IO must never fail scoring.
    let _ = inner.health.append(record);
}

async fn score_and_log(
    inner: &Inner,
    query: &str,
    hits: Vec<WorkerRankingHit>,
    served_order: Vec<String>,
) {
    let mut contributions = serde_json::Map::new();
    let mut merged_dirty = false;
    let count = inner.state.lock().map(|s| s.signals.len()).unwrap_or(0);
    for index in 0..count {
        let (pkg, bytes, capabilities, policy, missing) = {
            let mut state = inner
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(signal) = state.signals.get(index) else {
                break;
            };
            if !signal.healthy {
                continue;
            }
            let pkg = signal.pkg.clone();
            let cache = state.caches.entry(pkg.clone()).or_default();
            let missing: Vec<WorkerRankingHit> = hits
                .iter()
                .filter(|hit| !cache.entries.contains_key(&hit_key(hit)))
                .cloned()
                .collect();
            let signal = &state.signals[index];
            (
                pkg,
                Arc::clone(&signal.bytes),
                signal.capabilities.clone(),
                signal.policy,
                missing,
            )
        };
        if !missing.is_empty() {
            let outcome = inner
                .lane
                .contribute_ranking(bytes.to_vec(), missing.clone(), capabilities, policy)
                .await;
            let mut state = inner
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match outcome {
                Err(error) => {
                    breach(inner, &mut state, index, &error.message);
                    continue;
                }
                Ok(outcome) => {
                    // Partial lifts are kept (parallel prefix); a non-ok end breaches.
                    let cache = state.caches.entry(pkg.clone()).or_default();
                    for (hit, features) in missing.iter().zip(&outcome.contributions) {
                        cache.store(hit_key(hit), sanitize_contributions(&pkg, features));
                    }
                    if outcome.end != WorkerGuestEnd::Ok {
                        let reason =
                            super::PluginInvokeOutcome::ended(outcome.end, outcome.fuel_used)
                                .reason();
                        breach(inner, &mut state, index, &reason);
                    } else if promoted_in(&state, &state.signals[index]) {
                        merged_dirty = true;
                    }
                }
            }
        }
        let state = inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut rows = serde_json::Map::new();
        if let Some(cache) = state.caches.get(&pkg) {
            for hit in &hits {
                let key = hit_key(hit);
                if let Some(features) = cache.entries.get(&key).filter(|f| !f.is_empty()) {
                    rows.insert(key, serde_json::to_value(features).unwrap_or(Value::Null));
                }
            }
        }
        contributions.insert(pkg, Value::Object(rows));
    }
    let applied: Vec<String> = {
        let mut state = inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if merged_dirty {
            rebuild_merged(&mut state);
        }
        state
            .signals
            .iter()
            .filter(|s| s.healthy && promoted_in(&state, s))
            .map(|s| s.pkg.clone())
            .collect()
    };
    let query_hash = format!("{:x}", Sha256::digest(query.as_bytes()));
    append_shadow_log(
        inner,
        &json!({
            "at": (inner.clock)(),
            "queryHash": &query_hash[..16],
            "servedOrder": served_order,
            "applied": applied,
            "contributions": contributions,
        }),
    );
}

/// Append one entry, evicting the oldest half past the size cap. Logging
/// must never fail a search.
fn append_shadow_log(inner: &Inner, entry: &Value) {
    let line = entry.to_string();
    let path = &inner.shadow_log_path;
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let result = if size + line.len() as u64 + 1 > SHADOW_LOG_CAP_BYTES {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = existing.split('\n').filter(|l| !l.is_empty()).collect();
        let kept = &lines[lines.len().div_ceil(2)..];
        let mut text = kept.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&line);
        text.push('\n');
        std::fs::write(path, text)
    } else {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| writeln!(file, "{line}"))
    };
    let _ = result;
}

#[cfg(test)]
#[path = "ranking_signals.tests.rs"]
mod tests;
