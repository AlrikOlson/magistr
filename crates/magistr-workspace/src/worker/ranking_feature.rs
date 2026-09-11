use serde::{Deserialize, Serialize};

/// One named feature value lifted from a signal's `contribute` answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerRankingFeature {
    pub name: String,
    pub value: f64,
}

impl From<crate::sandbox::LiftedFeature> for WorkerRankingFeature {
    fn from(feature: crate::sandbox::LiftedFeature) -> Self {
        Self {
            name: feature.name,
            value: feature.value,
        }
    }
}
