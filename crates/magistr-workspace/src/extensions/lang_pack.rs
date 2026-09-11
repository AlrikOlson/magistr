use super::{
    EffectiveCapabilities, InstalledPluginRecord, LangPackStatus, PluginComponent,
    PluginGuestPolicy, PluginHost, PluginManifest,
};
use crate::worker::{WorkerGuestEnd, WorkerLanguageLane};

/// The lang-pack boot lane: discovers installed packs, reads each carrier in
/// the worker's sandbox lane and registers the language in the worker's
/// dynamic registry. The trust tier decides the execution lane there: a
/// blessed pack may promote to native execution, everything else runs its
/// grammar in the WASM store. A failing pack is breakered and its language
/// simply stays lexical-only. The worker owns the registry, so a worker
/// restart forgets it; boot may re-run at any time (idempotent per language).
pub struct LangPackBoot<'a> {
    host: &'a PluginHost<'a>,
}

impl<'a> LangPackBoot<'a> {
    /// Inventory pkg-name convention marking a plugin as a lang pack.
    pub const PKG_PREFIX: &'static str = "lang-pack-";

    #[must_use]
    pub fn new(host: &'a PluginHost<'a>) -> Self {
        Self { host }
    }

    /// Boot every installed lang pack; each answers a status row.
    pub async fn boot(&self) -> Vec<LangPackStatus> {
        let Ok(inventory) = self.host.discovery().inventory() else {
            return Vec::new();
        };
        let mut statuses = Vec::new();
        for record in inventory {
            if record.pkg().starts_with(Self::PKG_PREFIX) {
                statuses.push(self.boot_one(&record).await);
            }
        }
        statuses
    }

    async fn boot_one(&self, record: &InstalledPluginRecord) -> LangPackStatus {
        let plugin = record.name.clone();
        let refused = |outcome: &str, detail: String, notes: Vec<String>| LangPackStatus {
            plugin: plugin.clone(),
            ok: false,
            outcome: outcome.into(),
            detail,
            language: None,
            extensions: None,
            lane: None,
            queries_ok: None,
            notes,
        };
        let Some(dir) = record.dir.as_deref() else {
            return refused(
                "missing-component",
                "inventory-only record: no plugin directory".into(),
                Vec::new(),
            );
        };
        let source = match self.host.discovery().read_manifest(dir) {
            Ok(source) => source,
            Err(error) => return refused("manifest-unreadable", error, Vec::new()),
        };
        let manifest = match PluginManifest::parse(&source) {
            Ok(parsed) => parsed.manifest,
            Err(errors) => return refused("manifest-unreadable", errors.join("; "), Vec::new()),
        };
        let Some(component) =
            PluginComponent::resolve(self.host.discovery().policy, dir, &manifest.component_path)
        else {
            return refused(
                "missing-component",
                format!(
                    "component {} is missing or outside the workspace",
                    manifest.component_path
                ),
                Vec::new(),
            );
        };
        if let Some(breach) = self.host.health().breach(&plugin, &component.sha256) {
            return refused(
                "breakered",
                format!(
                    "unhealthy since {} ({}); a changed component clears the breaker",
                    breach.at, breach.reason
                ),
                Vec::new(),
            );
        }
        let capabilities = EffectiveCapabilities::resolve(record, &manifest);
        let mut notes = capabilities.notes;
        let outcome = self
            .host
            .lane()
            .load_lang_pack(
                component.bytes,
                capabilities.effective,
                PluginGuestPolicy::for_manifest(&manifest),
                record.is_blessed(),
            )
            .await;
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                self.host
                    .record_breach(&plugin, &component.sha256, &error.message, &mut notes);
                return refused("registration-refused", error.message, notes);
            }
        };
        let (Some(registration), WorkerGuestEnd::Ok) = (outcome.registration, &outcome.end) else {
            let reason = super::PluginInvokeOutcome::ended(outcome.end, outcome.fuel_used);
            let detail = if reason.detail().is_empty() {
                reason.kind().to_owned()
            } else {
                reason.detail()
            };
            self.host
                .record_breach(&plugin, &component.sha256, &detail, &mut notes);
            return refused(reason.kind(), detail, notes);
        };
        notes.extend(registration.notes);
        LangPackStatus {
            plugin,
            ok: true,
            outcome: "ok".into(),
            detail: String::new(),
            language: Some(registration.id),
            extensions: Some(registration.extensions),
            lane: Some(
                match registration.lane {
                    WorkerLanguageLane::Native => "native",
                    WorkerLanguageLane::Wasm => "wasm",
                }
                .into(),
            ),
            queries_ok: Some(registration.queries_ok),
            notes,
        }
    }
}

#[cfg(test)]
#[path = "lang_pack.tests.rs"]
mod tests;
