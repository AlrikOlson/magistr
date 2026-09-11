use serde::{Deserialize, Serialize};

use super::SearchConfidence;

/// One native matched line, with optional reference evidence and context.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub line: u64,
    pub column: u32,
    pub excerpt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<SearchConfidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicates: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_before: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_after: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_test_range: Option<bool>,
}

impl SearchHit {
    pub(super) fn new(line: u64, column: u32, excerpt: String) -> Self {
        Self {
            line,
            column,
            excerpt,
            kind: None,
            confidence: None,
            provenance: None,
            duplicates: None,
            context_before: None,
            context_after: None,
            in_test_range: None,
        }
    }
}
