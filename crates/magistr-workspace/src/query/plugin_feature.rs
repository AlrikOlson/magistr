use serde::{Deserialize, Serialize};

/// One namespaced feature value (`<plugin>/<feature>`) a promoted ranking
/// signal contributed for a hit. The first-party combiner decides its weight;
/// a plugin never sets a final score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginFeature {
    pub name: String,
    pub value: f64,
}
