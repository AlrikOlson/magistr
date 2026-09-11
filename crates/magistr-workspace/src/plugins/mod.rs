//! The installed-plugin inventory: `<state>/plugins.json` records, their
//! source and trust tier, read per call with the retained fail-closed rules.
//! Pure data — no guest ever runs here — so it compiles in every feature lane
//! and there is exactly one parser of the inventory: the map row's plugin
//! listing and the sandboxed extension host both derive from it.
mod installed_plugin;
mod inventory;
mod plugin_source;
mod plugin_tier;

pub use installed_plugin::InstalledPluginRecord;
pub use inventory::PluginInventory;
pub use plugin_source::PluginSource;
pub use plugin_tier::PluginTier;
