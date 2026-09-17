//! Native plugin coordination (chunk arch-14c8): capability manifests,
//! installation records and grants, the disk-backed circuit breaker, the
//! namespaced plugin-tool host, the conformance kit and the lang-pack,
//! ranking, resolver and runner lanes. Every guest executes through an
//! [`ExtensionLane`]; the production lane is the supervised native worker,
//! so untrusted bytes never load into the application owner's process.
#[cfg(feature = "lang-pack")]
mod admitted_lang_pack;
mod component;
mod discovery;
mod effective_capabilities;
mod grant;
mod guest_policy;
mod health;
mod health_record;
mod host;
mod host_clock;
mod invoke_outcome;
mod lane;
mod lane_future;
#[cfg(feature = "lang-pack")]
mod lang_pack;
#[cfg(feature = "lang-pack")]
mod lang_pack_admission;
#[cfg(feature = "lang-pack")]
mod lang_pack_load_outcome;
#[cfg(feature = "lang-pack")]
mod lang_pack_restore;
#[cfg(feature = "lang-pack")]
mod lang_pack_status;
mod language_server_spec;
mod limits;
mod manifest;
mod parsed_manifest;
mod ranking_contribute_outcome;
mod ranking_signal_status;
mod ranking_signals;
mod reserved;
mod resolved_edge;
mod resolved_plugin;
mod resolver;
mod resolver_lane_outcome;
mod runner;
mod runner_input;
mod runner_lane_outcome;
pub mod tck;
mod tool_decl;
mod tool_listing;
mod wasi_lane_outcome;
mod worker_lane;

#[cfg(feature = "lang-pack")]
pub use admitted_lang_pack::AdmittedLangPack;
pub use component::PluginComponent;
pub use effective_capabilities::EffectiveCapabilities;
pub use grant::grant_covers;
pub use guest_policy::PluginGuestPolicy;
pub use health::PluginHealth;
pub use health_record::PluginHealthRecord;
pub use host::PluginHost;
pub use host_clock::HostClock;
pub use invoke_outcome::PluginInvokeOutcome;
pub use lane::ExtensionLane;
pub use lane_future::LaneFuture;
#[cfg(feature = "lang-pack")]
pub use lang_pack::LangPackBoot;
#[cfg(feature = "lang-pack")]
pub use lang_pack_admission::LangPackAdmission;
#[cfg(feature = "lang-pack")]
pub use lang_pack_load_outcome::LangPackLoadOutcome;
#[cfg(feature = "lang-pack")]
pub use lang_pack_restore::LangPackRestore;
#[cfg(feature = "lang-pack")]
pub use lang_pack_status::LangPackStatus;
pub use language_server_spec::LanguageServerSpec;
pub use limits::PluginLimits;
pub use manifest::PluginManifest;
pub use parsed_manifest::ParsedManifest;
pub use ranking_contribute_outcome::RankingContributeOutcome;
// The inventory records are pure data shared with the map row; they live in
// `crate::plugins` (every feature lane) and keep their extension-side names.
pub use crate::plugins::{InstalledPluginRecord, PluginInventory, PluginSource, PluginTier};
pub use ranking_signal_status::RankingSignalStatus;
pub use ranking_signals::RankingSignals;
pub use reserved::RESERVED_TOOL_NAMES;
pub use resolved_edge::ResolvedEdge;
pub use resolved_plugin::ResolvedPlugin;
pub use resolver::PluginResolver;
pub use resolver_lane_outcome::ResolverLaneOutcome;
pub use runner::RunnerAdapter;
pub use runner_input::RunnerInput;
pub use runner_lane_outcome::RunnerLaneOutcome;
pub use tool_decl::PluginToolDecl;
pub use tool_listing::PluginToolListing;
pub use wasi_lane_outcome::WasiLaneOutcome;
pub use worker_lane::WorkerExtensionLane;
