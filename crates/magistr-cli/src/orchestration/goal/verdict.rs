//! What the judge answered.

use serde::{Deserialize, Serialize};

/// The judge's answer, in exactly the three shapes Claude Code's own
/// `/goal` evaluator uses — `{ok:true, reason}`,
/// `{ok:false, reason}` and `{ok:false, impossible:true, reason}` — so
/// a delegated mode's verdict and magistr's own report through one type.
///
/// A verdict is an *opinion*: it never completes a chunk by itself. See
/// [`Decision::reconcile`](super::Decision::reconcile) for what the loop
/// does with it against the real gates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    /// Whether the judge holds the acceptance to be met.
    pub ok: bool,
    /// Why, in the judge's words — shown on the stage either way.
    #[serde(default)]
    pub reason: String,
    /// The judge holds the acceptance cannot be met as written; the
    /// loop stops and asks a human rather than iterating again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub impossible: bool,
}

impl Verdict {
    /// The JSON schema a constrained model call is held to — the
    /// evaluator's own: `ok` and `reason` required, `impossible`
    /// optional, nothing else.
    pub const SCHEMA: &'static str = r#"{"type":"object","properties":{"ok":{"type":"boolean"},"reason":{"type":"string"},"impossible":{"type":"boolean"}},"required":["ok","reason"],"additionalProperties":false}"#;

    /// The acceptance is met.
    #[must_use]
    pub fn met(reason: impl Into<String>) -> Self {
        Self {
            ok: true,
            reason: reason.into(),
            impossible: false,
        }
    }

    /// Not yet — iterate again.
    #[must_use]
    pub fn not_yet(reason: impl Into<String>) -> Self {
        Self {
            ok: false,
            reason: reason.into(),
            impossible: false,
        }
    }

    /// Cannot be met as written — stop and ask.
    #[must_use]
    pub fn impossible(reason: impl Into<String>) -> Self {
        Self {
            ok: false,
            reason: reason.into(),
            impossible: true,
        }
    }

    /// Read a verdict off a model's text: the first JSON object in it,
    /// with or without a ```` ```json ```` fence. Anything that does not
    /// parse is *not yet*, carrying the raw text as its reason — a
    /// chatty or broken judge can only ever say "iterate again", never
    /// "done" and never "give up".
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let trimmed = text.trim();
        let candidate = trimmed
            .find('{')
            .and_then(|start| trimmed.rfind('}').map(|end| &trimmed[start..=end]))
            .unwrap_or(trimmed);
        let mut parsed: Self = match serde_json::from_str(candidate) {
            Ok(verdict) => verdict,
            Err(_) => {
                return Self::not_yet(format!("the judge did not answer in shape: {trimmed}"));
            }
        };
        if parsed.impossible {
            // `impossible` is a refinement of "not ok": a verdict that
            // says both met and impossible is malformed, read as not yet.
            parsed.ok = false;
        }
        parsed
    }
}

#[cfg(test)]
#[path = "verdict.tests.rs"]
mod tests;
