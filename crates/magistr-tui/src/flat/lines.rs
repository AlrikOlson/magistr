//! The session as flat lines.

use super::flat_line::FlatLine;
use super::plain::plain;
use super::role::Role;
use magistr_cockpit::agent::{ToolStatus, ToolVerb};
use magistr_cockpit::cockpit::{Model, TurnState};
use magistr_cockpit::turn::{Line, LineKind, Voice};

/// The whole session as linear text: one opening line, the mode banner,
/// then the turn's **source** lines — not the wrapped rows, which is
/// what leaves every glyph column behind.
///
/// Each line carries whether it has settled, and the rule is per kind:
///
/// * **prose** grows only while it is the last line, so every earlier
///   one is final;
/// * a **tool** or an **edit** is found by id and updated in place, so
///   it is final only once its status is terminal — being last has
///   nothing to do with it;
/// * a **gate**, an **event**, a **note**, an **error** and an **image**
///   are final where they were pushed.
///
/// And above all of it: a turn that is no longer
/// [`Streaming`](TurnState::Streaming) is settled whole. That is what
/// stops a call the user cancelled — pending for ever — from wedging
/// everything behind it, and what lets the last prose line of a finished
/// turn out.
///
/// A gate's details and an edit's diff follow their head as
/// **continuations** (no role, indented). The cockpit's own text goes
/// through [`plain`]; the agent's prose, its tool titles and a diff body
/// are content and arrive as they were written.
#[must_use]
pub fn lines(model: &Model) -> Vec<FlatLine> {
    let mut out = vec![FlatLine::settled(Some(Role::Session), opening(model))];
    if let Some(banner) = model.banner() {
        out.push(FlatLine::settled(Some(Role::Note), plain(banner)));
    }
    let turn = model.turn();
    let streaming = model.turn_state() == TurnState::Streaming;
    let last = turn.lines().len().checked_sub(1);
    for (index, line) in turn.lines().iter().enumerate() {
        let settled = !streaming || is_settled(line.kind(), Some(index) == last);
        push(&mut out, line, settled, streaming);
    }
    out
}

/// The session's own first line: the project, and the mode named so a
/// listener knows why this reads as text.
fn opening(model: &Model) -> String {
    model.title().map_or_else(
        || "magistr cockpit (flat mode)".to_owned(),
        |title| format!("{} (flat mode)", plain(title)),
    )
}

/// Whether nothing can change this line any more, given that the turn is
/// still streaming.
fn is_settled(kind: &LineKind, is_last: bool) -> bool {
    match kind {
        LineKind::Prose(_) => !is_last,
        LineKind::Tool { status, .. } | LineKind::Edit { status, .. } => is_final(*status),
        _ => true,
    }
}

/// Whether a tool call can still report anything.
const fn is_final(status: ToolStatus) -> bool {
    matches!(status, ToolStatus::Completed | ToolStatus::Failed)
}

/// One source line, as the one or more flat lines it reads as.
fn push(out: &mut Vec<FlatLine>, line: &Line, settled: bool, streaming: bool) {
    match line.kind() {
        LineKind::Prose(voice) if line.text().is_empty() => {
            out.push(FlatLine::new(None, "", settled));
        }
        LineKind::Prose(voice) => {
            out.push(FlatLine::new(
                Some(voice_role(*voice)),
                line.text(),
                settled,
            ));
        }
        LineKind::Tool {
            status,
            verb,
            duration_ms,
            ..
        } => out.push(FlatLine::new(
            Some(Role::Tool),
            if !streaming && !is_final(*status) {
                format!("{} {} - no result reported", verb.label(), line.text())
            } else {
                tool_head(line.text(), *verb, *status, *duration_ms)
            },
            settled,
        )),
        LineKind::Check {
            name,
            passed,
            exit_code,
            duration_ms,
            ..
        } => {
            out.push(FlatLine::new(
                Some(Role::Gate),
                check_head(name, *passed, *exit_code, *duration_ms),
                settled,
            ));
            body(out, line.text(), settled);
        }
        LineKind::Edit {
            path,
            status,
            undone,
            ..
        } => {
            out.push(FlatLine::new(
                Some(Role::Edit),
                if !streaming && !is_final(*status) {
                    format!("{path} - no result reported")
                } else {
                    edit_head(path, *status, *undone)
                },
                settled,
            ));
            body(out, line.text(), settled);
        }
        LineKind::Image(_) => out.push(FlatLine::new(
            Some(Role::Image),
            image_text(line.text()),
            settled,
        )),
        LineKind::Event => out.push(FlatLine::new(
            Some(Role::Event),
            plain(line.text()),
            settled,
        )),
        LineKind::Note => out.push(FlatLine::new(Some(Role::Note), plain(line.text()), settled)),
        // The stream's error lines carry their own `error: ` prefix,
        // which the label would say a second time.
        LineKind::Error => out.push(FlatLine::new(
            Some(Role::Error),
            plain(undouble(line.text(), "error: ")),
            settled,
        )),
    }
}

/// `text` without a leading `label` a role would repeat.
fn undouble<'a>(text: &'a str, label: &str) -> &'a str {
    text.strip_prefix(label).unwrap_or(text)
}

/// Who a prose line is spoken as.
const fn voice_role(voice: Voice) -> Role {
    match voice {
        Voice::Agent => Role::Agent,
        Voice::Thought => Role::Thought,
        Voice::User => Role::User,
    }
}

/// A line's detail, as continuations under its head. Nothing at all when
/// there is no detail — the flat cockpit has no fold to hide it behind,
/// so an empty one would be an empty line for a reader to announce.
fn body(out: &mut Vec<FlatLine>, text: &str, settled: bool) {
    if text.is_empty() {
        return;
    }
    for line in text.lines() {
        out.push(FlatLine::new(None, line, settled));
    }
}

/// `verb title - status[ in N ms]`.
fn tool_head(title: &str, verb: ToolVerb, status: ToolStatus, duration_ms: Option<u64>) -> String {
    let mut head = format!("{} {title} - {}", verb.label(), status_word(status));
    if let Some(ms) = duration_ms {
        head.push_str(&format!(" in {ms} ms"));
    }
    head
}

/// `name passed|failed[, exit N][ in N ms]`.
fn check_head(
    name: &str,
    passed: bool,
    exit_code: Option<i64>,
    duration_ms: Option<u64>,
) -> String {
    let mut head = format!("{name} {}", if passed { "passed" } else { "failed" });
    if let Some(code) = exit_code {
        head.push_str(&format!(", exit {code}"));
    }
    if let Some(ms) = duration_ms {
        head.push_str(&format!(" in {ms} ms"));
    }
    head
}

/// `path - status[, undone]`.
fn edit_head(path: &str, status: ToolStatus, undone: bool) -> String {
    let mut head = format!("{path} - {}", status_word(status));
    if undone {
        head.push_str(", undone");
    }
    head
}

/// A tool call's status, in one word.
const fn status_word(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Pending => "pending",
        ToolStatus::InProgress => "running",
        ToolStatus::Completed => "done",
        ToolStatus::Failed => "failed",
    }
}

/// What a flat cockpit can say about an image: its alt text, or that it
/// came without one. There are no pixels here to fall back on.
fn image_text(alt: &str) -> String {
    if alt.trim().is_empty() {
        "an image, with no description".to_owned()
    } else {
        alt.to_owned()
    }
}

#[cfg(test)]
#[path = "lines.tests.rs"]
mod tests;
