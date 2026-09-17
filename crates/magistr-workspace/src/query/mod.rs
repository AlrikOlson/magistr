//! Native read/search/map use cases. Production tool dispatch and owner cutover are separate.
pub(crate) mod budget;
mod map;
mod map_cargo;
mod map_cargo_summary;
mod map_cargo_targets;
mod map_crate_role;
mod map_dependency;
mod map_dependent;
mod map_detail;
mod map_enrichment;
mod map_entry_kind;
mod map_entry_point;
mod map_entry_points;
mod map_import_graph;
mod map_import_module;
mod map_index;
mod map_input;
mod map_plugin;
mod map_row;
mod map_symbol;
mod map_tree;
pub use map_cargo_summary::MapCargoSummary;
pub use map_crate_role::MapCrateRole;
pub use map_dependency::MapDependency;
pub use map_dependent::MapDependent;
pub use map_detail::MapDetail;
pub use map_entry_kind::MapEntryKind;
pub use map_entry_point::MapEntryPoint;
pub use map_import_graph::MapImportGraph;
pub use map_import_module::MapImportModule;
pub use map_index::MapIndex;
pub use map_input::MapInput;
pub use map_plugin::MapPlugin;
pub use map_row::MapRow;
pub use map_symbol::MapSymbol;
mod service;
mod source_access;
mod source_entry;
pub(crate) use source_entry::SourceEntry;
mod source_observation;
mod source_read;
mod worker_error;
pub use service::WorkspaceQueries;
pub(crate) use worker_error::worker_error;
mod plugin_feature;
mod plugin_feature_lookup;
pub use plugin_feature::PluginFeature;
pub use plugin_feature_lookup::PluginFeatureLookup;
mod ranking;
mod ranking_candidate;
/// Test seam over the private ranking classifier.
#[cfg(all(test, feature = "sandbox"))]
pub(crate) fn hit_contexts_for_tests(
    query: &str,
    groups: &[SearchGroup],
    cap: usize,
) -> Vec<crate::worker::WorkerRankingHit> {
    ranking::Ranking::hit_contexts(query, groups, cap)
}
mod read;
mod read_input;
mod read_range;
mod read_row;
mod read_segment;
mod read_selection;
mod read_surround;
mod read_symbol_ref;
mod read_symbols;
#[cfg(feature = "sandbox")]
mod search_resolver;
pub(crate) use read_symbols::SymbolAddress;
mod read_unit;
mod search;
mod search_confidence;
mod search_group;
mod search_hit;
mod search_include;
mod search_input;
mod search_mode;
mod search_references;
mod search_scope;
pub use read_input::ReadInput;
pub use read_range::ReadRange;
pub use read_row::ReadRow;
pub use read_segment::ReadSegment;
pub use read_surround::ReadSurround;
pub use read_symbol_ref::ReadSymbolRef;
pub use read_unit::ReadUnit;
pub use search_confidence::SearchConfidence;
pub use search_group::SearchGroup;
pub use search_hit::SearchHit;
pub use search_include::SearchInclude;
pub use search_input::SearchInput;
pub use search_mode::SearchMode;
pub(crate) use search_scope::SearchScope;
