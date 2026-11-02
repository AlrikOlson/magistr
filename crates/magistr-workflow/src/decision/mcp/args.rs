//! Input-argument structs for every MCP tool that takes parameters.
//!
//! These types only exist because `schemars::JsonSchema` derives the
//! tool's `inputSchema` from them. The engine doesn't see these structs
//! directly — by the time engine code runs, the values are already
//! unwrapped into method arguments or merged into [`crate::decision::domain`]
//! types like `Note`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Canonical empty parameters type for zero-arg tools.
///
/// No `///` doc comment because schemars surfaces those as the
/// MCP-schema `description`, leaking implementation details to LLM
/// consumers.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[schemars(description = "")]
pub struct NoArgs {}

#[path = "log_args.rs"]
mod log;
pub use log::LogArgs;

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[schemars(description = "Export options.")]
pub struct ExportArgs {
    /// Named log to read. Omit for the project's default log.
    #[serde(default)]
    pub log_id: Option<String>,
    /// "markdown" (default, badged), "json" (full state), "console"
    /// (ANSI-styled), or "tree" (thread structure as text).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[schemars(description = "Update the projected total note count on the most recent note in place.")]
pub struct ReviseEstimateArgs {
    /// New estimated total note count for the decision log. Must be >= 1.
    /// Coerced forgivingly (int | numeric string); an unparseable/absent value
    /// becomes 0 and the handler returns a soft "must be >= 1" error rather
    /// than a -32602 that would cancel sibling tool calls.
    #[serde(default, deserialize_with = "crate::infra::coerce::lenient_u32")]
    #[schemars(required, !default)]
    pub estimated_total: u32,
    /// Optional human-readable note about why the estimate is changing.
    /// Surfaced back in the response text so the log remains self-explaining.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[schemars(description = "Look up a single recorded note by its note number.")]
pub struct NoteLookupArgs {
    /// The note number to retrieve. Coerced forgivingly (int | numeric string);
    /// an unparseable/absent value becomes 0, which the handler reports as a
    /// soft "not found" rather than a -32602 that would cancel siblings.
    #[serde(default, deserialize_with = "crate::infra::coerce::lenient_u32")]
    #[schemars(required, !default)]
    pub note: u32,
    /// When true, walk the `revised_by` chain forward and return the live
    /// (latest-revision) note instead of the original. Defaults to false so
    /// historical inspection still works.
    #[serde(default, deserialize_with = "crate::infra::coerce::lenient_opt_bool")]
    pub resolve_latest: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[schemars(description = "Case-insensitive substring search across recorded notes.")]
pub struct SearchArgs {
    /// Substring to search for, case-insensitive. Matches across the
    /// `thought`, `outcome`, `context`, `purpose`, and text-form `next_action`
    /// fields of every recorded note (including thread notes). Defaults to ""
    /// when absent; the handler reports an empty query as a soft error.
    #[serde(default)]
    #[schemars(required, !default)]
    pub query: String,
    /// Maximum number of matches to return. Defaults to 10 when omitted.
    #[serde(default, deserialize_with = "crate::infra::coerce::lenient_opt_u32")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[schemars(description = "Log the dependency and revision graph around a note.")]
pub struct ImpactArgs {
    /// The note to analyze. Returns upstream dependencies, downstream
    /// dependents, the revision chain through this note, and any threads
    /// that fork off it. Coerced forgivingly (int | numeric string); an
    /// unparseable/absent value becomes 0 → soft "not found", never a -32602.
    #[serde(default, deserialize_with = "crate::infra::coerce::lenient_u32")]
    #[schemars(required, !default)]
    pub note: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[schemars(description = "Mark an existing thread as active, merged, or abandoned.")]
pub struct ThreadStatusArgs {
    /// The id of the thread (e.g. as returned by the `decision_record` tool's
    /// `thread.id` field when the thread was created). Defaults to "" when
    /// absent; the handler reports an empty id as a soft error.
    #[serde(default)]
    #[schemars(required, !default)]
    pub thread_id: String,
    /// New status. One of: "open", "merged", "abandoned". Defaults to ""
    /// when absent → soft "invalid status" rather than a -32602.
    #[serde(default)]
    #[schemars(required, !default)]
    pub status: String,
    /// Optional note number that synthesized this thread back into the
    /// main reasoning line. Recorded on the thread and surfaced by
    /// `decision_impact`. Only meaningful when `status: "merged"`.
    #[serde(default, deserialize_with = "crate::infra::coerce::lenient_opt_u32")]
    pub merged_into: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[schemars(description = "Pin or unpin a note.")]
pub struct PinArgs {
    /// Note number to pin or unpin. Coerced forgivingly (int | numeric
    /// string); an unparseable/absent value becomes 0 → soft "not found".
    #[serde(default, deserialize_with = "crate::infra::coerce::lenient_u32")]
    #[schemars(required, !default)]
    pub note: u32,
    /// True to pin, false to unpin. Defaults to true.
    #[serde(default, deserialize_with = "crate::infra::coerce::lenient_opt_bool")]
    pub pinned: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[schemars(description = "Status snapshot options.")]
pub struct StatusArgs {
    /// Named log to read. Omit for the project's default log.
    #[serde(default)]
    pub log_id: Option<String>,
    /// When true, include `pinned[]` (full note descriptors) and
    /// `logs[]` (per-log summaries) in the response. Default false
    /// keeps the snapshot compact.
    #[serde(default, deserialize_with = "crate::infra::coerce::lenient_opt_bool")]
    pub verbose: Option<bool>,
}

#[cfg(test)]
mod tests {
    //! These prove the load-bearing claim: tool arguments deserialize
    //! *infallibly*. A wrong-typed or absent field must never produce a serde
    //! error (which the rmcp layer would surface as JSON-RPC -32602 and Claude
    //! Code would cascade-cancel into sibling calls). Each case below would
    //! have errored under the strict derive.
    use super::*;
    use serde_json::json;

    #[test]
    fn note_number_as_string_is_coerced_not_rejected() {
        let a: NoteLookupArgs = serde_json::from_value(json!({"note": "999"})).unwrap();
        assert_eq!(a.note, 999);
    }

    #[test]
    fn absent_required_numeric_defaults_zero() {
        let a: ImpactArgs = serde_json::from_value(json!({})).unwrap();
        assert_eq!(a.note, 0); // handler reports a soft "not found"
        let r: ReviseEstimateArgs = serde_json::from_value(json!({})).unwrap();
        assert_eq!(r.estimated_total, 0); // handler reports soft ">= 1"
    }

    #[test]
    fn pin_accepts_stringy_bool_and_number_note() {
        let a: PinArgs = serde_json::from_value(json!({"note": 3, "pinned": "false"})).unwrap();
        assert_eq!(a.note, 3);
        assert_eq!(a.pinned, Some(false));
    }

    #[test]
    fn thread_status_absent_fields_default_empty() {
        // Empty status → handler's parse() yields a soft "invalid_status".
        let a: ThreadStatusArgs = serde_json::from_value(json!({})).unwrap();
        assert_eq!(a.thread_id, "");
        assert_eq!(a.status, "");
        assert_eq!(a.merged_into, None);
    }

    #[test]
    fn search_limit_as_string_is_coerced() {
        let a: SearchArgs = serde_json::from_value(json!({"query": "x", "limit": "25"})).unwrap();
        assert_eq!(a.limit, Some(25));
    }
}
