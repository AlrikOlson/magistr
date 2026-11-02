//! Typed cross-references between `decision_*` and `run_*` entities.
//!
//! In-process, all cross-references are values of [`CrossRef`]. The wire
//! contract used by tool inputs and persisted traces is a `prefix:value`
//! string. Old clients impose no constraint on changing this representation.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::StoredRefWords;

pub type StepNumber = u32;
pub type StepId = String;
pub type ProgressId = u32;
pub type CheckName = String;
pub type TaskId = String;
pub type RequestId = String;
/// A tracker provider key, e.g. `"github"`, `"linear"`, `"jira"`. Normalized to
/// lowercase — provider keys are ours to name, so they may be case-folded.
pub type ProviderId = String;
/// An id minted by an external tracker. Case- and punctuation-preserving: a
/// GitHub GraphQL node id is base64 and a Jira key is uppercase, so unlike the
/// provider this is never folded.
pub type ExternalId = String;

/// Separates provider from external id inside an `ext:` cross-ref value.
const EXTERNAL_SEP: char = '/';

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum CrossRef {
    Note(StepNumber),
    RunStep(StepId),
    RunProgress(ProgressId),
    RunCheck(CheckName),
    /// A plan task: the long-horizon item a run goal
    /// realizes and a decision note shapes. Wire form `task:<slug>`.
    PlanTask(TaskId),
    /// A stakeholder request: the opportunity a plan task was
    /// promoted from. Wire form `request:<id>`. Promotion writes this onto the
    /// task and the reciprocal `task:<slug>` onto the request.
    RequestRef(RequestId),
    /// A work item in an EXTERNAL tracker — the twin a plan task was
    /// projected onto (tracker-port-seam). Wire form
    /// `ext:<provider>/<external_id>`, e.g. `ext:github/1234`,
    /// `ext:linear/ENG-42`, `ext:jira/PROJ-7`.
    ///
    /// This is the one cross-ref whose referent lives outside the system, which
    /// is exactly why it belongs here rather than in a provider module: the
    /// provenance graph gains an edge to a tracker without any core type
    /// learning what a tracker *is*. Adding a provider adds a string, not a
    /// variant.
    ///
    /// Two-part because a bare external id is meaningless — issue `1234` exists
    /// in every tracker on earth. `provider` is normalized to lowercase;
    /// `external_id` is preserved byte-for-byte (see [`ExternalId`]).
    External {
        provider: ProviderId,
        external_id: ExternalId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    MissingDelimiter,
    UnknownPrefix(String),
    InvalidNumber(String),
    EmptyValue,
    /// An `ext:` ref whose value has no `provider/external_id` split.
    MissingProviderSeparator,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDelimiter => write!(f, "cross-ref string must contain ':'"),
            Self::UnknownPrefix(p) => write!(
                f,
                "unknown cross-ref prefix '{p}' (expected decision|step|progress|check|task|request|ext)"
            ),
            Self::InvalidNumber(s) => write!(f, "expected integer, got '{s}'"),
            Self::EmptyValue => write!(f, "cross-ref value is empty"),
            Self::MissingProviderSeparator => write!(
                f,
                "external cross-ref must be 'ext:<provider>{EXTERNAL_SEP}<external_id>'"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

impl CrossRef {
    /// Build an [`CrossRef::External`] with the provider normalized the same way
    /// parsing normalizes it, so a hand-built ref and a parsed one are equal.
    /// Prefer this over the struct literal.
    #[must_use]
    pub fn external(provider: &str, external_id: &str) -> Self {
        Self::External {
            provider: provider.trim().to_ascii_lowercase(),
            external_id: external_id.trim().to_string(),
        }
    }

    /// Serialize to the wire string form: `decision:42`, `step:auth-refactor`,
    /// `progress:5`, `check:cargo-test`, `task:phase-26c`,
    /// `ext:github/1234`.
    pub fn to_wire(&self) -> String {
        match self {
            Self::Note(n) => format!("decision:{n}"),
            Self::RunStep(id) => format!("step:{id}"),
            Self::RunProgress(n) => format!("progress:{n}"),
            Self::RunCheck(name) => format!("check:{name}"),
            Self::PlanTask(id) => format!("task:{id}"),
            Self::RequestRef(id) => format!("request:{id}"),
            Self::External {
                provider,
                external_id,
            } => format!("ext:{provider}{EXTERNAL_SEP}{external_id}"),
        }
    }

    /// Parse from the wire string form.
    pub fn from_wire(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let (prefix, value) = s.split_once(':').ok_or(ParseError::MissingDelimiter)?;
        let value = value.trim();
        if value.is_empty() {
            return Err(ParseError::EmptyValue);
        }
        match prefix.trim().to_ascii_lowercase().as_str() {
            "decision" => value
                .parse::<u32>()
                .map(Self::Note)
                .map_err(|_| ParseError::InvalidNumber(value.to_string())),
            "step" => Ok(Self::RunStep(value.to_string())),
            "progress" => value
                .parse::<u32>()
                .map(Self::RunProgress)
                .map_err(|_| ParseError::InvalidNumber(value.to_string())),
            "check" => Ok(Self::RunCheck(value.to_string())),
            "task" => Ok(Self::PlanTask(value.to_string())),
            "request" => Ok(Self::RequestRef(value.to_string())),
            // Split on the FIRST separator only: everything after it is the
            // external id, so `ext:github/owner/repo#12` keeps its slashes and a
            // base64 GraphQL node id survives intact.
            "ext" | "external" => {
                let (provider, external_id) = value
                    .split_once(EXTERNAL_SEP)
                    .ok_or(ParseError::MissingProviderSeparator)?;
                // Normalize through the constructor so a parsed ref and a
                // hand-built one are byte-identical, then reject empty halves.
                match Self::external(provider, external_id) {
                    Self::External {
                        provider,
                        external_id,
                    } if provider.is_empty() || external_id.is_empty() => {
                        Err(ParseError::EmptyValue)
                    }
                    r => Ok(r),
                }
            }
            other => Err(ParseError::UnknownPrefix(other.to_string())),
        }
    }
}

impl CrossRef {
    /// Rewrite every stored reference in `value` from `words` to the current
    /// vocabulary. Each whole reference string is mapped once, so the
    /// glossary's order holds without chaining: a run step's `task:` becomes
    /// `step:` and a plan task's `chunk:` becomes `task:` in the same pass.
    /// Prose that merely starts with a prefix keeps its words. The caller
    /// decides `words` from the document's version and converts once.
    /// Returns whether anything changed.
    pub fn upgrade_stored_refs(value: &mut serde_json::Value, words: StoredRefWords) -> bool {
        match value {
            serde_json::Value::String(text) => match Self::upgraded_ref(text, words) {
                Some(upgraded) => {
                    *text = upgraded;
                    true
                }
                None => false,
            },
            serde_json::Value::Array(items) => items.iter_mut().fold(false, |changed, item| {
                Self::upgrade_stored_refs(item, words) | changed
            }),
            serde_json::Value::Object(fields) => {
                fields.values_mut().fold(false, |changed, item| {
                    Self::upgrade_stored_refs(item, words) | changed
                })
            }
            _ => false,
        }
    }

    fn upgraded_ref(text: &str, words: StoredRefWords) -> Option<String> {
        let (prefix, rest) = text.split_once(':')?;
        if rest.is_empty() || rest.chars().any(char::is_whitespace) {
            return None;
        }
        let numeric = rest.chars().all(|c| c.is_ascii_digit());
        match (words, prefix) {
            (_, "signal") => Some(format!("request:{rest}")),
            (StoredRefWords::BeforeRequestWords, _) => None,
            (StoredRefWords::BeforeDecisionWords, "chunk") => None,
            (_, "chunk") => Some(format!("task:{rest}")),
            (_, "think") if numeric => Some(format!("decision:{rest}")),
            (StoredRefWords::BeforeRunWords, "step") if numeric => Some(format!("decision:{rest}")),
            (StoredRefWords::BeforeRunWords, "task") => Some(format!("step:{rest}")),
            (StoredRefWords::BeforeRunWords, "action") if numeric => {
                Some(format!("progress:{rest}"))
            }
            _ => None,
        }
    }
}

impl fmt::Display for CrossRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_wire())
    }
}

impl From<CrossRef> for String {
    fn from(r: CrossRef) -> Self {
        r.to_wire()
    }
}

impl TryFrom<String> for CrossRef {
    type Error = ParseError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_wire(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(r: CrossRef) {
        let s = r.to_wire();
        let back = CrossRef::from_wire(&s).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn round_trip_note() {
        round_trip(CrossRef::Note(42));
    }

    #[test]
    fn round_trip_run_task() {
        round_trip(CrossRef::RunStep("auth-refactor".to_string()));
    }

    #[test]
    fn round_trip_run_action() {
        round_trip(CrossRef::RunProgress(7));
    }

    #[test]
    fn round_trip_check_step() {
        round_trip(CrossRef::RunCheck("cargo-test".to_string()));
    }

    #[test]
    fn round_trip_plan_task() {
        round_trip(CrossRef::PlanTask("phase-26c".to_string()));
    }

    #[test]
    fn round_trip_request_ref() {
        round_trip(CrossRef::RequestRef("f1c9a4e2".to_string()));
    }

    #[test]
    fn request_wire_form() {
        assert_eq!(
            CrossRef::RequestRef("abc123".to_string()).to_wire(),
            "request:abc123"
        );
        assert_eq!(
            CrossRef::from_wire("request:abc123").unwrap(),
            CrossRef::RequestRef("abc123".to_string())
        );
    }

    #[test]
    fn task_wire_form() {
        assert_eq!(
            CrossRef::PlanTask("phase-1".to_string()).to_wire(),
            "task:phase-1"
        );
        assert_eq!(
            CrossRef::from_wire("task:phase-1").unwrap(),
            CrossRef::PlanTask("phase-1".to_string())
        );
    }

    #[test]
    fn round_trip_external() {
        round_trip(CrossRef::external("github", "1234"));
        round_trip(CrossRef::external("linear", "ENG-42"));
        round_trip(CrossRef::external("jira", "PROJ-7"));
    }

    #[test]
    fn external_wire_form() {
        assert_eq!(
            CrossRef::external("github", "1234").to_wire(),
            "ext:github/1234"
        );
        assert_eq!(
            CrossRef::from_wire("ext:github/1234").unwrap(),
            CrossRef::external("github", "1234")
        );
        // `external` is accepted as a long-form alias, like `step` for `decision`.
        assert_eq!(
            CrossRef::from_wire("external:github/1234").unwrap(),
            CrossRef::external("github", "1234")
        );
    }

    /// The asymmetry that matters: the provider key is ours to name so it folds
    /// to lowercase, but an external id belongs to the tracker and must survive
    /// byte-for-byte — GitHub GraphQL node ids are base64 and Jira keys are
    /// uppercase, so folding either would produce a ref that resolves to
    /// nothing.
    #[test]
    fn external_normalizes_provider_but_never_the_id() {
        let r = CrossRef::from_wire("ext:GitHub/MDU6SXNzdWUx").unwrap();
        assert_eq!(
            r,
            CrossRef::External {
                provider: "github".into(),
                external_id: "MDU6SXNzdWUx".into(),
            }
        );
        assert_eq!(r.to_wire(), "ext:github/MDU6SXNzdWUx");
    }

    /// Only the FIRST separator splits, so ids containing `/` or `:` survive —
    /// `owner/repo#12` is a legitimate GitHub-shaped id and base64 node ids can
    /// contain `/` too. If this regressed, every such ref would silently point
    /// at a truncated item.
    #[test]
    fn external_id_may_contain_separators() {
        let r = CrossRef::from_wire("ext:github/AlrikOlson/magistr#12").unwrap();
        assert_eq!(r, CrossRef::external("github", "AlrikOlson/magistr#12"));
        round_trip(r);

        let r = CrossRef::from_wire("ext:jira/https://x.atlassian.net/browse/AB-1").unwrap();
        assert_eq!(
            r,
            CrossRef::external("jira", "https://x.atlassian.net/browse/AB-1")
        );
        round_trip(r);
    }

    #[test]
    fn external_without_separator_rejected() {
        let e = CrossRef::from_wire("ext:github").unwrap_err();
        assert_eq!(e, ParseError::MissingProviderSeparator);
    }

    #[test]
    fn external_with_empty_half_rejected() {
        assert_eq!(
            CrossRef::from_wire("ext:/1234").unwrap_err(),
            ParseError::EmptyValue
        );
        assert_eq!(
            CrossRef::from_wire("ext:github/").unwrap_err(),
            ParseError::EmptyValue
        );
        assert_eq!(
            CrossRef::from_wire("ext:github/   ").unwrap_err(),
            ParseError::EmptyValue
        );
    }

    #[test]
    fn external_serde_round_trip() {
        let r = CrossRef::external("linear", "ENG-42");
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"ext:linear/ENG-42\"");
        let back: CrossRef = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    /// The additive guarantee: every pre-existing wire form still parses to the
    /// same variant it always did. A stored trace written before `ext:` existed
    /// must load unchanged.
    #[test]
    fn existing_wire_forms_are_unaffected() {
        for (wire, expected) in [
            ("decision:42", CrossRef::Note(42)),
            ("step:auth", CrossRef::RunStep("auth".into())),
            ("progress:5", CrossRef::RunProgress(5)),
            ("check:cargo-test", CrossRef::RunCheck("cargo-test".into())),
            ("task:phase-1", CrossRef::PlanTask("phase-1".into())),
            ("request:abc", CrossRef::RequestRef("abc".into())),
        ] {
            assert_eq!(CrossRef::from_wire(wire).unwrap(), expected);
            assert_eq!(expected.to_wire(), wire);
        }
    }

    /// The run family's words replaced `task:` and `action:`, and `step:`
    /// names a run step, never a reasoning step. The old spellings are
    /// refused like any unknown prefix; the store upgrade rewrote them.
    #[test]
    fn retired_prefixes_are_refused() {
        for old in ["chunk:auth", "action:5"] {
            let e = CrossRef::from_wire(old).unwrap_err();
            assert!(matches!(e, ParseError::UnknownPrefix(_)), "{old}");
        }
        assert_eq!(
            CrossRef::from_wire("task:auth").unwrap(),
            CrossRef::PlanTask("auth".into())
        );
        assert_eq!(
            CrossRef::from_wire("step:9").unwrap(),
            CrossRef::RunStep("9".into())
        );
    }

    #[test]
    fn unknown_prefix_rejected() {
        let e = CrossRef::from_wire("nope:1").unwrap_err();
        assert!(matches!(e, ParseError::UnknownPrefix(_)));
    }

    #[test]
    fn missing_delimiter_rejected() {
        let e = CrossRef::from_wire("step-without-colon").unwrap_err();
        assert_eq!(e, ParseError::MissingDelimiter);
    }

    #[test]
    fn empty_value_rejected() {
        let e = CrossRef::from_wire("step: ").unwrap_err();
        assert_eq!(e, ParseError::EmptyValue);
    }

    #[test]
    fn references_saved_before_the_run_words_take_every_new_word_in_one_pass() {
        let mut stored = serde_json::json!({
            "cross_refs": ["task:auth", "action:4", "step:9", "think:2", "chunk:x", "check:lint", "signal:s1"],
            "execution_ref": "task:verify",
            "context": "task: read the notes first",
            "nested": [{"ref": "action:12"}],
        });
        assert!(CrossRef::upgrade_stored_refs(
            &mut stored,
            StoredRefWords::BeforeRunWords
        ));
        assert_eq!(
            stored,
            serde_json::json!({
                "cross_refs": ["step:auth", "progress:4", "decision:9", "decision:2", "task:x", "check:lint", "request:s1"],
                "execution_ref": "step:verify",
                "context": "task: read the notes first",
                "nested": [{"ref": "progress:12"}],
            })
        );
    }

    #[test]
    fn references_saved_with_the_run_words_rename_plan_tasks_and_notes() {
        let mut stored = serde_json::json!({
            "refs": ["step:auth", "progress:4", "think:9", "chunk:x", "task:y"],
            "scope": "chunk:vocab-plan-family",
        });
        assert!(CrossRef::upgrade_stored_refs(
            &mut stored,
            StoredRefWords::BeforePlanWords
        ));
        assert_eq!(
            stored,
            serde_json::json!({
                "refs": ["step:auth", "progress:4", "decision:9", "task:x", "task:y"],
                "scope": "task:vocab-plan-family",
            })
        );
        let once = stored.clone();
        assert!(
            !CrossRef::upgrade_stored_refs(&mut stored, StoredRefWords::BeforePlanWords),
            "a second pass from the run words changes nothing"
        );
        assert_eq!(stored, once);
    }

    #[test]
    fn references_saved_with_the_plan_words_only_rename_notes() {
        let mut stored = serde_json::json!({
            "refs": ["step:auth", "progress:4", "think:9", "task:y", "chunk:kept"],
            "context": "think: about it",
        });
        assert!(CrossRef::upgrade_stored_refs(
            &mut stored,
            StoredRefWords::BeforeDecisionWords
        ));
        assert_eq!(
            stored,
            serde_json::json!({
                "refs": ["step:auth", "progress:4", "decision:9", "task:y", "chunk:kept"],
                "context": "think: about it",
            })
        );
        assert!(!CrossRef::upgrade_stored_refs(
            &mut stored,
            StoredRefWords::BeforeDecisionWords
        ));
    }

    #[test]
    fn references_saved_with_the_decision_words_only_rename_requests() {
        let mut stored = serde_json::json!({
            "refs": ["decision:9", "task:y", "chunk:kept", "think:kept", "signal:s1"],
            "context": "signal: a prose label",
        });
        assert!(CrossRef::upgrade_stored_refs(
            &mut stored,
            StoredRefWords::BeforeRequestWords
        ));
        assert_eq!(
            stored,
            serde_json::json!({
                "refs": ["decision:9", "task:y", "chunk:kept", "think:kept", "request:s1"],
                "context": "signal: a prose label",
            })
        );
        assert!(!CrossRef::upgrade_stored_refs(
            &mut stored,
            StoredRefWords::BeforeRequestWords
        ));
    }

    #[test]
    fn the_retired_request_prefix_is_refused_like_any_unknown_prefix() {
        assert_eq!(
            CrossRef::from_wire("signal:s1"),
            Err(ParseError::UnknownPrefix("signal".into()))
        );
    }

    #[test]
    fn the_retired_note_prefix_is_refused_like_any_unknown_prefix() {
        assert_eq!(
            CrossRef::from_wire("think:5"),
            Err(ParseError::UnknownPrefix("think".into()))
        );
    }

    #[test]
    fn whitespace_tolerated() {
        let r = CrossRef::from_wire("  decision:5  ").unwrap();
        assert_eq!(r, CrossRef::Note(5));
    }

    #[test]
    fn case_insensitive_prefix() {
        let r = CrossRef::from_wire("STEP:foo").unwrap();
        assert_eq!(r, CrossRef::RunStep("foo".to_string()));
    }

    #[test]
    fn serde_round_trip() {
        let r = CrossRef::RunStep("x".to_string());
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, "\"step:x\"");
        let back: CrossRef = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }
}
