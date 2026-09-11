use serde::{Deserialize, Serialize};

/// Level of symbol enrichment on an otherwise live workspace tree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MapDetail {
    Skeleton,
    #[default]
    Outline,
    Full,
}
