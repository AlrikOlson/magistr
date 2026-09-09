//! Goal delivery and recognition over ACP.
//!
//! An agent can expose goals through an advertised extension or a standard
//! `goal` command. Magistr's ACP agent uses command discovery; clients send
//! `/{name} {objective}` after finding `goal` in `available_commands_update`.
//!
//! Extension clients validate `initialize.result._meta.goal`: version 1, an
//! underscore-prefixed `controlMethod`, and support for the requested action.
//! The advertised method accepts `sessionId`, `action` (`set` or `clear`),
//! and an `objective` for `set`. An idle agent may start a full turn while
//! handling this request, so delivery must not block the UI.
//!
//! A `session_info_update` can report a snapshot in `_meta.goal`, or `null`
//! when cleared. Only `objective` is required by [`GoalSnapshot`]; additional
//! progress fields are optional. A matching objective confirms delivery and
//! recognition, not acceptance, progress, or completion. Callers must obtain
//! completion evidence separately.
//!
//! [`AgentGoal::compile_objective`] canonicalizes and caps the condition at
//! 500 characters. Conditions should name checks whose output appears in the
//! transcript, since a transcript-only evaluator cannot inspect files or run
//! commands. Clear a delegated goal before falling back to another loop.

use std::fmt::Write as _;

use agent_client_protocol::schema::v1::{Meta, SessionUpdate};
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// The extension's vocabulary, as one namespace: the constants the
/// advertisement is checked against, the request and turn a delivery
/// composes, the echo it is confirmed by, and the objective it carries.
/// See the module's doc for the specification this implements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentGoal;

impl AgentGoal {
    /// The `_meta` key the extension is advertised and reported under.
    pub const META_KEY: &'static str = "goal";
    /// The advertisement version this client knows.
    pub const VERSION: u64 = 1;
    /// The command's name, as the standard command list carries it —
    /// what [`Self::find_command`] looks for, never what is sent.
    pub const COMMAND: &'static str = "goal";
    /// The `set` action.
    pub const SET: &'static str = "set";
    /// The `clear` action.
    pub const CLEAR: &'static str = "clear";
    /// The agent's cap on a condition after canonicalisation.
    pub const OBJECTIVE_CAP: usize = 500;

    /// One extension's advertisement out of a `_meta` — `initialize`'s,
    /// today; the same read serves `steering` and `jetbrains.air`, so no
    /// registry is needed. `None` when the key is absent or the value
    /// does not parse as `T`.
    #[must_use]
    pub fn capability<T: DeserializeOwned>(meta: Option<&Meta>, key: &str) -> Option<T> {
        let value = meta?.get(key)?;
        serde_json::from_value(value.clone()).ok()
    }

    /// [`Self::capability`] over a session update's `_meta` — a
    /// `session_info_update` carries the goal snapshot there. `None` for
    /// any other update, an absent key, or a value that is not a `T`
    /// (`null` included: the agent reports a cleared goal as `null`).
    #[must_use]
    pub fn update_capability<T: DeserializeOwned>(update: &SessionUpdate, key: &str) -> Option<T> {
        match update {
            SessionUpdate::SessionInfoUpdate(info) => Self::capability(info.meta.as_ref(), key),
            _ => None,
        }
    }

    /// The goal advertisement out of `initialize._meta`, **validated**:
    /// a known version, a control method that starts with `_`. An
    /// advertisement that is missing, unparsable or structurally off is
    /// `None`, and the extension is not used.
    #[must_use]
    pub(crate) fn advertisement(meta: Option<&Meta>) -> Option<GoalAdvertisement> {
        let advertised: GoalAdvertisement = Self::capability(meta, Self::META_KEY)?;
        (advertised.version == Self::VERSION && advertised.control_method.starts_with('_'))
            .then_some(advertised)
    }

    /// The goal command's name among the advertised command names —
    /// found by name, so nothing hardcodes the literal `/goal`. `None`
    /// when the agent lists no such command.
    #[must_use]
    pub fn find_command<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<String> {
        names
            .into_iter()
            .find(|name| *name == Self::COMMAND)
            .map(str::to_owned)
    }

    /// The command names a session update advertises, when it is an
    /// `available_commands_update`.
    #[must_use]
    pub fn command_names(update: &SessionUpdate) -> Option<Vec<String>> {
        match update {
            SessionUpdate::AvailableCommandsUpdate(commands) => Some(
                commands
                    .available_commands
                    .iter()
                    .map(|command| command.name.clone())
                    .collect(),
            ),
            _ => None,
        }
    }

    /// The turn that sets the goal through the command: `/{name}
    /// {objective}`. This grammar is a **convention** — the advertised
    /// command declares no argument hint (`input: null`), so the form is
    /// assumed, not discovered; the echo ([`Self::echo`]) is what catches
    /// it if it is ever wrong.
    #[must_use]
    pub fn command_turn(name: &str, objective: &str) -> String {
        format!("/{name} {objective}")
    }

    /// The turn that clears the goal through the command — the same
    /// convention with the `clear` word the agent parses.
    #[must_use]
    pub fn command_clear_turn(name: &str) -> String {
        Self::command_turn(name, Self::CLEAR)
    }

    /// The `set` request's params.
    #[must_use]
    pub fn set_params(session_id: &str, objective: &str) -> serde_json::Value {
        serde_json::json!({
            "sessionId": session_id,
            "action": Self::SET,
            "objective": objective,
        })
    }

    /// The `clear` request's params.
    #[must_use]
    pub fn clear_params(session_id: &str) -> serde_json::Value {
        serde_json::json!({ "sessionId": session_id, "action": Self::CLEAR })
    }

    /// Whether `update` is the optimistic echo of `objective`: a
    /// `session_info_update` whose `_meta.goal.objective` is exactly what
    /// was sent. This proves **delivery and recognition** — the agent
    /// parsed the goal out of the text the delivery caused — and it
    /// proves neither acceptance nor progress; no runtime snapshot
    /// follows it (see the module doc).
    #[must_use]
    pub fn echo(update: &SessionUpdate, objective: &str) -> bool {
        Self::update_capability::<GoalSnapshot>(update, Self::META_KEY)
            .is_some_and(|snapshot| snapshot.objective == objective)
    }

    /// The condition a delegated iteration hands the agent, compiled
    /// for its evaluator: it names the chunk, points at the project's
    /// own `/next` workflow, and — because the evaluator reads the
    /// transcript only — tells the agent to run the gates and print
    /// their verdict. Canonicalised (whitespace collapsed) and held
    /// under [`Self::OBJECTIVE_CAP`] by trimming the title first, so the
    /// agent never refuses it as too long. `chunk` is `(id, title)`, or
    /// `None` on a project with no roadmap chunk to name.
    #[must_use]
    pub fn compile_objective(chunk: Option<(&str, &str)>) -> String {
        let subject = match chunk {
            Some((id, _)) => format!("roadmap chunk `{id}`"),
            None => "the next roadmap item".to_owned(),
        };
        let mut out = format!("Done means: {subject} is complete");
        let title = chunk.map(|(_, title)| canonical(title)).unwrap_or_default();
        let tail = " — run the /next workflow for it until every acceptance criterion \
                    roadmap_get lists is met, run `magistr validate` and print its verdict \
                    line in the transcript, every gate passes, and the work is committed.";
        let room = Self::OBJECTIVE_CAP.saturating_sub(out.chars().count() + tail.chars().count());
        if !title.is_empty() && room > 8 {
            let title = if title.chars().count() > room - 4 {
                let mut short: String = title.chars().take(room - 5).collect();
                short.push('…');
                short
            } else {
                title
            };
            let _ = write!(out, " ({title})");
        }
        out.push_str(tail);
        canonical(&out)
    }
}

/// The advertisement on `initialize._meta.goal`, as parsed — unknown
/// fields ignored, so a wider future advertisement still reads.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GoalAdvertisement {
    /// The extension's version.
    pub version: u64,
    /// The `_`-prefixed method the goal is controlled through.
    pub control_method: String,
    /// The actions that method accepts.
    #[serde(default)]
    pub actions: Vec<String>,
}

impl GoalAdvertisement {
    /// The control method, when the advertisement lists `action` — the
    /// string a request is sent with, taken from here and nowhere else.
    #[must_use]
    pub fn method_for(&self, action: &str) -> Option<&str> {
        self.actions
            .iter()
            .any(|listed| listed == action)
            .then_some(self.control_method.as_str())
    }
}

/// The snapshot on `session_info_update._meta.goal`, read for the one
/// field a client may rely on. Everything else the agent declares
/// (`status`, `controlMethod`, and the never-emitted `iterations`,
/// `lastReason`, timestamps and budgets) is deliberately not modelled:
/// unknown fields are ignored, and nothing here can ask for them.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct GoalSnapshot {
    /// The condition the agent is working toward.
    pub objective: String,
}

/// Whitespace collapsed to single spaces, trimmed — the agent's own
/// canonicalisation before it measures the cap.
fn canonical(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "agent_goal.tests.rs"]
mod tests;
