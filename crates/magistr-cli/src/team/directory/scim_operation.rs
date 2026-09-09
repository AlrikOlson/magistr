//! One operation of a SCIM `PATCH` request.

use serde::{Deserialize, Serialize};

/// A single `Operations` entry of an RFC 7644 §3.5.2 PATCH body.
///
/// `path` is optional because both providers that matter send it both
/// ways: Okta writes `{"op":"replace","path":"active","value":false}`
/// and Entra writes `{"op":"replace","value":{"active":false}}`. A
/// reader that handles only one of them handles only half its
/// customers, which is why [`target`](Self::target) resolves the two
/// shapes into the same pair before anything acts on them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScimOperation {
    /// `add`, `replace` or `remove`.
    pub op: String,
    /// The attribute path, when the provider names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The new value.
    #[serde(default)]
    pub value: serde_json::Value,
}

impl ScimOperation {
    /// Whether this operation removes rather than sets.
    #[must_use]
    pub fn is_remove(&self) -> bool {
        self.op.eq_ignore_ascii_case("remove")
    }

    /// The `(path, value)` pairs this operation asks for.
    ///
    /// A pathed operation is one pair. A path-less one is an object
    /// whose keys are the paths, so it is as many pairs as it has
    /// members — which is exactly how the two provider dialects become
    /// one thing to apply.
    #[must_use]
    pub fn targets(&self) -> Vec<(String, &serde_json::Value)> {
        match self
            .path
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            Some(path) => vec![(path.to_string(), &self.value)],
            None => self
                .value
                .as_object()
                .map(|map| map.iter().map(|(k, v)| (k.clone(), v)).collect())
                .unwrap_or_default(),
        }
    }
}
