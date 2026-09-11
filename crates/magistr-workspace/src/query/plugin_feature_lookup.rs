use std::collections::BTreeMap;

use super::PluginFeature;

/// Promoted plugin features by hit identity, resolved by the caller from the
/// signal cache before ranking — the ranking stays pure. An absent map or
/// absent key contributes zero, so a cold cache is the first-party baseline.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PluginFeatureLookup {
    features: BTreeMap<String, Vec<PluginFeature>>,
}

impl PluginFeatureLookup {
    /// Cache/lookup key for one hit's plugin features.
    #[must_use]
    pub fn key(file: &str, line: u64, column: u32, excerpt: &str) -> String {
        format!("{file}\u{0}{line}\u{0}{column}\u{0}{excerpt}")
    }

    pub fn insert(&mut self, key: String, features: Vec<PluginFeature>) {
        self.features.entry(key).or_default().extend(features);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&[PluginFeature]> {
        self.features.get(key).map(Vec::as_slice)
    }

    /// Σ weight·value over the hit's features; unlisted names weigh zero.
    #[must_use]
    pub fn boost(&self, file: &str, line: u64, column: u32, excerpt: &str) -> f64 {
        self.get(&Self::key(file, line, column, excerpt))
            .map_or(0.0, |features| {
                features
                    .iter()
                    .map(|feature| Self::weight(&feature.name) * feature.value)
                    .sum()
            })
    }

    /// The retained weight table: the ONLY path by which a signal's value
    /// reaches a score. A name not listed here weighs zero even when promoted.
    #[must_use]
    pub fn weight(name: &str) -> f64 {
        match name {
            "ranking-visibility/exported" => 0.5,
            _ => 0.0,
        }
    }
}
