use std::time::Duration;

use super::{
    EffectiveCapabilities, HostClock, LanguageServerSpec, PluginComponent, PluginGuestPolicy,
    PluginHost, PluginManifest, ResolvedEdge,
};
use crate::{sandbox::CAP_SUBPROCESS, worker::WorkerGuestEnd};

/// Breach expiry: the bounded, lazy retry window.
const RETRY_AFTER: Duration = Duration::from_secs(60);
/// Per-request ceiling: past this, the symbolic answer ships without us.
const BUDGET_DEFAULT: Duration = Duration::from_secs(2);

/// The resolver upgrade lane: the blessed, subprocess-granted resolver plugin
/// consulted lazily (never below a warm index) and never awaited past its
/// budget. Every failure mode — absent, un-blessed, un-granted, circuit
/// open, slow, crashed, malformed — answers an empty edge list, and the
/// caller keeps its symbolic answer.
pub struct PluginResolver<'a> {
    host: &'a PluginHost<'a>,
    root: String,
    specs: Vec<LanguageServerSpec>,
    budget: Duration,
}

impl<'a> PluginResolver<'a> {
    /// The bundled resolver's inventory name.
    pub const PLUGIN_NAME: &'static str = "workbench.lsp-resolver";

    /// `root` is the workspace root the guest reports locations against.
    pub fn new(
        host: &'a PluginHost<'a>,
        root: impl Into<String>,
        specs: Vec<LanguageServerSpec>,
    ) -> Self {
        Self {
            host,
            root: root.into(),
            specs,
            budget: BUDGET_DEFAULT,
        }
    }

    #[must_use]
    pub fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = budget;
        self
    }

    /// Resolver-confirmed reference edges for `symbol`, or nothing.
    pub async fn resolve_references(&self, symbol: &str, index_warm: bool) -> Vec<ResolvedEdge> {
        if self.specs.is_empty() || !index_warm {
            return Vec::new();
        }
        let Some((manifest, component, capabilities)) = self.find_plugin() else {
            return Vec::new();
        };
        if self.circuit_open(&component.sha256) {
            return Vec::new();
        }
        let invoke = self.host.lane().resolve_references(
            component.bytes,
            symbol.to_owned(),
            capabilities,
            PluginGuestPolicy::for_manifest(&manifest),
            self.specs.iter().map(Into::into).collect(),
            self.root.clone(),
        );
        let outcome = match tokio::time::timeout(self.budget, invoke).await {
            Err(_) => {
                // Slowness is a health signal, not a wait.
                self.breach(
                    &component.sha256,
                    "resolver exceeded its per-request budget",
                );
                return Vec::new();
            }
            // A dead lane degrades service; it is not the plugin's breach.
            Ok(Err(_)) => return Vec::new(),
            Ok(Ok(outcome)) => outcome,
        };
        if outcome.end != WorkerGuestEnd::Ok {
            if !matches!(outcome.end, WorkerGuestEnd::CapDenied(_)) {
                let reason =
                    super::PluginInvokeOutcome::ended(outcome.end, outcome.fuel_used).reason();
                self.breach(&component.sha256, &reason);
            }
            return Vec::new();
        }
        let mut edges = Vec::with_capacity(outcome.locations.len());
        for location in outcome.locations {
            match self.admit(&location.path) {
                Some(path) if location.line >= 1 && location.column >= 1 => {
                    edges.push(ResolvedEdge {
                        path,
                        line: location.line,
                        column: location.column,
                    })
                }
                _ => {
                    // One malformed or out-of-jail location poisons the answer.
                    self.breach(
                        &component.sha256,
                        &format!("malformed resolver location {:?}", location.path),
                    );
                    return Vec::new();
                }
            }
        }
        edges
    }

    /// The bundled resolver: installed, blessed and subprocess-granted.
    fn find_plugin(&self) -> Option<(PluginManifest, PluginComponent, Vec<String>)> {
        let discovery = self.host.discovery();
        let record = discovery.inventory().ok()?.into_iter().find(|record| {
            record.name == Self::PLUGIN_NAME && record.dir.is_some() && record.is_blessed()
        })?;
        let dir = record.dir.clone()?;
        let manifest = PluginManifest::parse(&discovery.read_manifest(&dir).ok()?)
            .ok()?
            .manifest;
        let capabilities = EffectiveCapabilities::resolve(&record, &manifest).effective;
        if !capabilities.iter().any(|cap| cap == CAP_SUBPROCESS) {
            return None;
        }
        let component = PluginComponent::resolve(discovery.policy, &dir, &manifest.component_path)?;
        Some((manifest, component, capabilities))
    }

    /// Breach active (not yet expired) under these exact component bytes?
    fn circuit_open(&self, component_hash: &str) -> bool {
        let Some(now) = HostClock::parse(&self.host.now()) else {
            return false;
        };
        self.host.health().read().iter().any(|breach| {
            breach.plugin == Self::PLUGIN_NAME
                && breach.component_hash == component_hash
                && HostClock::parse(&breach.at)
                    .is_some_and(|at| now.saturating_sub(at) < RETRY_AFTER.as_secs())
        })
    }

    fn breach(&self, component_hash: &str, reason: &str) {
        let mut notes = Vec::new();
        self.host
            .record_breach(Self::PLUGIN_NAME, component_hash, reason, &mut notes);
    }

    /// Admit one guest-reported path through the workspace jail and
    /// relativize it against the admitted root (symlinked roots included).
    fn admit(&self, path: &str) -> Option<String> {
        if path.is_empty() {
            return None;
        }
        let policy = self.host.discovery().policy;
        let admitted = policy.check_read(path).ok()?;
        let root = policy.check_read(".").ok()?;
        let relative = admitted.strip_prefix(&root).ok()?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        (!relative.is_empty() && !relative.starts_with("..")).then_some(relative)
    }
}

#[cfg(test)]
#[path = "resolver.tests.rs"]
mod tests;
