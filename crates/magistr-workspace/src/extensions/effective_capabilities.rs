use super::{InstalledPluginRecord, PluginManifest, grant_covers};
use crate::sandbox::is_subprocess_import;

/// The capability set one plugin run gets: declared ∩ granted (default deny),
/// with every subprocess stamp additionally reserved to tier "blessed". A
/// grant a human was talked into giving a sideload is ignored with a note,
/// so the guest is refused before instantiation rather than ever linking the
/// interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveCapabilities {
    pub effective: Vec<String>,
    pub notes: Vec<String>,
}

impl EffectiveCapabilities {
    #[must_use]
    pub fn resolve(record: &InstalledPluginRecord, manifest: &PluginManifest) -> Self {
        let grants = record.grants.as_deref().unwrap_or_default();
        let mut notes = Vec::new();
        let effective = manifest
            .imports
            .iter()
            .filter(|import| grants.iter().any(|grant| grant_covers(grant, import)))
            .filter(|import| {
                if is_subprocess_import(import) && !record.is_blessed() {
                    notes.push(format!(
                        "plugin {}: the subprocess capability is reserved to blessed plugins — grant ignored (tier: {}, SDD §8.3)",
                        record.name,
                        record.tier.map_or("untiered", |tier| tier.as_str())
                    ));
                    return false;
                }
                true
            })
            .cloned()
            .collect();
        Self { effective, notes }
    }
}
