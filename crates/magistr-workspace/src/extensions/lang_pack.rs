use super::{
    AdmittedLangPack, EffectiveCapabilities, InstalledPluginRecord, LangPackAdmission,
    LangPackStatus, PluginComponent, PluginGuestPolicy, PluginHost, PluginManifest,
};

/// The lang-pack boot lane: discovers installed packs, reads each carrier in
/// the worker's sandbox lane and registers the language in the worker's
/// dynamic registry. The trust tier decides the execution lane there: a
/// blessed pack may promote to native execution, everything else runs its
/// grammar in the WASM store. A failing pack is breakered and its language
/// simply stays lexical-only. Registration lives in the worker: the owner
/// keeps what `admit` decided and replays it into every incarnation through
/// [`super::LangPackRestore`]; `boot` is the same admission followed by one
/// registration through the lane (idempotent per language).
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

    /// The host-side half of boot, no worker involved: every installed pack
    /// with a readable manifest and component is admitted with its carrier
    /// bytes captured; the rest are refused status rows.
    #[must_use]
    pub fn admit(&self) -> LangPackAdmission {
        let Ok(inventory) = self.host.discovery().inventory() else {
            return LangPackAdmission::default();
        };
        let mut admission = LangPackAdmission::default();
        for record in inventory {
            if !record.pkg().starts_with(Self::PKG_PREFIX) {
                continue;
            }
            match self.admit_one(&record) {
                Ok(pack) => admission.packs.push(pack),
                Err(status) => admission.refused.push(*status),
            }
        }
        admission
    }

    /// Boot every installed lang pack through the host's lane; each answers
    /// a status row.
    pub async fn boot(&self) -> Vec<LangPackStatus> {
        let admission = self.admit();
        let mut statuses = admission.refused;
        for pack in &admission.packs {
            statuses.push(self.register(pack).await);
        }
        statuses
    }

    fn admit_one(
        &self,
        record: &InstalledPluginRecord,
    ) -> Result<AdmittedLangPack, Box<LangPackStatus>> {
        let plugin = record.name.clone();
        let refused = |outcome: &str, detail: String| {
            Box::new(LangPackStatus::refused(
                &plugin,
                outcome,
                detail,
                Vec::new(),
            ))
        };
        let Some(dir) = record.dir.as_deref() else {
            return Err(refused(
                "missing-component",
                "inventory-only record: no plugin directory".into(),
            ));
        };
        let source = self
            .host
            .discovery()
            .read_manifest(dir)
            .map_err(|error| refused("manifest-unreadable", error))?;
        let manifest = PluginManifest::parse(&source)
            .map(|parsed| parsed.manifest)
            .map_err(|errors| refused("manifest-unreadable", errors.join("; ")))?;
        let component =
            PluginComponent::resolve(self.host.discovery().policy, dir, &manifest.component_path)
                .ok_or_else(|| {
                refused(
                    "missing-component",
                    format!(
                        "component {} is missing or outside the workspace",
                        manifest.component_path
                    ),
                )
            })?;
        let capabilities = EffectiveCapabilities::resolve(record, &manifest);
        Ok(AdmittedLangPack {
            plugin,
            component,
            capabilities: capabilities.effective,
            notes: capabilities.notes,
            policy: PluginGuestPolicy::for_manifest(&manifest),
            blessed: record.is_blessed(),
        })
    }

    async fn register(&self, pack: &AdmittedLangPack) -> LangPackStatus {
        if let Some(breach) = self
            .host
            .health()
            .breach(&pack.plugin, &pack.component.sha256)
        {
            return LangPackStatus::breakered(pack, &breach);
        }
        let outcome = self
            .host
            .lane()
            .load_lang_pack(
                pack.component.bytes.clone(),
                pack.capabilities.clone(),
                pack.policy,
                pack.blessed,
            )
            .await
            .map_err(|error| error.message);
        LangPackStatus::registered(pack, outcome, self.host.health(), &self.host.now())
    }
}

#[cfg(test)]
#[path = "lang_pack.tests.rs"]
mod tests;
