use crate::plan::domain::{FeatureCatalog, Focus, FocusState};

use super::FeatureInspection;

/// A saved team focus and the current plan observed under one owner lock.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FocusRead {
    pub team: String,
    pub focus: Option<Focus>,
    pub focus_state: FocusState,
    pub frontier: FeatureInspection,
    pub features: FeatureCatalog,
    pub persistent: bool,
}
