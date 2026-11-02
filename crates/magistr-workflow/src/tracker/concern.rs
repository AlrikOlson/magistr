//! Divergence reaches a human — the last step of the conflict story.
//!
//! # Why this is not inside the projector
//!
//! [`project_all`](super::project::project_all) DETECTS divergence and returns it. It does not raise
//! anything, and that separation is deliberate: a detector that also writes is
//! a detector nobody can reason about, and the same line was held for the
//! reconcile sweep in `tracker::sweep`. So the projector reports, the CALLER
//! decides whether those reports become signals, and this module is what the
//! caller uses when it decides yes.
//!
//! The practical consequence: `project_all` still takes no [`SignalEngine`],
//! and a caller that only wants to know what diverged — a dry run, a test, a
//! read-only audit — gets that without anything being written anywhere.
//!
//! # Why a signal rather than a log line
//!
//! Before this, a divergence reached `tracing::warn`. That means it reached a
//! human only if one happened to be watching stderr during a push, which is
//! nobody, most of the time. The signal family already solves this problem:
//! capture, research, surface under earned-interruption discipline, promote to
//! a plan task. Reusing it means a conflict arrives through machinery this
//! project already built and already trusts, instead of a bespoke channel that
//! would need its own inbox, its own triage and its own reasons to be believed.
//!
//! # Both directions of provenance
//!
//! Each concern carries two cross-refs: `task:<id>` and
//! `ext:<provider>/<external_id>`. From the task you can find the conflict;
//! from the ticket you can find the task. A one-directional link would make
//! the concern findable only by someone who already knew where to look.

use crate::plan::engine::PlanEngine;
use crate::signal::SignalEngine;
use crate::signal::domain::SignalKind;
use crate::tracker::TrackerError;
use crate::tracker::project::ProjectionReport;

/// Who a divergence concern is "from". Not a person — the projection itself
/// noticed it, and pretending otherwise would put a human's name on a machine's
/// observation.
const AUTHOR: &str = "tracker-projection";

/// Turn a projection's divergences into concern signals.
///
/// Returns the ids of the signals actually captured, which is fewer than the
/// divergences whenever one was already raised.
///
/// IDEMPOTENT by body and refs: re-projecting an unchanged conflict does not
/// mint a second signal. That matters more than it looks — a projection runs on
/// every push, so a duplicating version would turn one disagreement into a
/// stream of identical concerns and the inbox would be abandoned within a day.
/// A conflict whose VALUES changed produces a different body and therefore a
/// new signal, which is correct: it is a different disagreement.
pub fn emit_divergence_concerns(
    signal: &mut SignalEngine,
    plan: &PlanEngine,
    provider: &str,
    report: &ProjectionReport,
) -> Result<Vec<String>, TrackerError> {
    let mut captured = Vec::new();

    for (task_id, divergence) in &report.divergences {
        // A field the tracker owns diverging is normal, not news. Raising it
        // would be the noise that gets the whole channel ignored.
        if divergence.owner == crate::tracker::ownership::Owner::Theirs {
            continue;
        }

        let Some(link) = plan.tracker_link(task_id, provider) else {
            // No link means no external ref to point at, and a concern that
            // cannot name the ticket it is about is not worth raising.
            continue;
        };

        let task_ref = format!("task:{task_id}");
        let external_ref = format!("ext:{provider}/{}", link.external_id);
        let body = divergence.summary();

        if already_raised(signal, &body, &task_ref, &external_ref) {
            continue;
        }

        let result = signal
            .capture_linked(
                SignalKind::Concern,
                AUTHOR.to_string(),
                body,
                vec![task_ref, external_ref],
            )
            .map_err(|error| {
                let message = format!(
                    "Saved {} concerns; the next concern could not be saved: {error}",
                    captured.len()
                );
                let store = match error {
                    crate::signal::engine::SignalMutationError::Store(error) => Some(error),
                    crate::signal::engine::SignalMutationError::Rejected(_) => None,
                };
                TrackerError::Local { message, store }
            })?;
        captured.push(result.value.id);
    }

    Ok(captured)
}

/// Whether this exact disagreement is already on someone's plate.
///
/// Matches on the body AND both refs rather than the body alone: the same
/// sentence about two different tasks is two different problems.
fn already_raised(signal: &SignalEngine, body: &str, task_ref: &str, external_ref: &str) -> bool {
    signal.signals().signals.iter().any(|s| {
        s.body == body
            && s.refs.iter().any(|r| r == task_ref)
            && s.refs.iter().any(|r| r == external_ref)
    })
}

/// Turn a sweep's remote changes into STATUS proposals on the tasks they
/// belong to.
///
/// The counterpart of [`emit_divergence_concerns`], and caller-invoked for the
/// same reason: `reconcile` detects, this decides, and a caller that only wants
/// to know what moved gets that without anything being written.
///
/// A proposal is NEVER a transition. A ticket closed in the tracker looks
/// exactly like a task that should go done, but a close means the ticket is
/// finished — not that the acceptance criteria were met. Transitioning silently
/// would remove the one moment a human was going to look at the evidence, which
/// is the moment the plan exists to create.
///
/// Returns the task ids that received a proposal.
pub fn propose_status_from_sweep(
    plan: &mut PlanEngine,
    provider: &str,
    report: &crate::tracker::sweep::SweepReport,
) -> Result<Vec<String>, TrackerError> {
    let mut proposed = Vec::new();

    for item in &report.remote {
        let Some(external_id) = item.external_id.as_deref() else {
            continue;
        };
        let Some((task_id, current)) = plan
            .tracker_link_by_external_id(provider, external_id)
            .map(|l| l.task_id.clone())
            .and_then(|id| {
                plan.plan()
                    .tasks
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| (c.id.clone(), c.status))
            })
        else {
            // A ticket we never projected. The sweep reports it; proposing a
            // status for a task that does not exist would be nonsense.
            continue;
        };

        let suggested = status_for(item.state);
        if suggested == current {
            continue;
        }

        let reason = format!(
            "The tracker moved {external_id} to {:?}. The plan still says {current:?}. \
             A tracker close means the ticket is finished, not that the acceptance \
             criteria were met — so this is a suggestion, not a transition.",
            item.state
        );
        let source = format!("ext:{provider}/{external_id}");

        plan.propose_status(&task_id, suggested, reason, source)
            .map_err(|error| proposal_failure(error, proposed.len()))?;
        proposed.push(task_id);
    }

    Ok(proposed)
}

/// Turn a sweep's remote retitles into TITLE proposals on the tasks they
/// belong to — the pull-side door into the durable-concession machinery.
///
/// The sibling of [`propose_status_from_sweep`], and the reason it exists is
/// the shape that sibling cannot see: a human retitles a ticket and the plan
/// never edits that task locally. Push cheap-skips before any I/O (nothing
/// local changed), so reconcile never runs and no Contested divergence is
/// produced. The sweep DOES fetch the retitle as a genuine remote change —
/// this is where that fetch becomes a proposal.
///
/// Two fences, both inherited rather than reimplemented:
///
/// - **Echo**: only `report.remote` is read. The sweep's `reconcile` already
///   ran every fetched item through the echo fence; our own writes coming
///   back land in `echoes`/`drifted` and never reach this loop, so a title
///   proposal cannot be minted from our own projection returning.
/// - **Ownership**: only an [`Owner::Contested`](crate::tracker::ownership::Owner::Contested) title may propose. `Ours`
///   means a remote title divergence is push's business to re-assert, and
///   proposing it here would launder a remote overwrite of a field the plan
///   owns into a polite suggestion. `Theirs` (a table override) means the
///   team decided title divergence is not a conversation at all.
///
/// A proposal is NEVER an edit: `propose_title` fills `title_proposal` and
/// the task's real `title` does not move. Idempotent across repeated sweeps
/// because `propose_title` is — an unchanged suggestion does not restamp
/// `proposed_at`.
///
/// Returns the task ids that received a proposal.
pub fn propose_titles_from_sweep(
    plan: &mut PlanEngine,
    provider: &str,
    report: &crate::tracker::sweep::SweepReport,
    ownership: &crate::tracker::ownership::Ownership,
) -> Result<Vec<String>, TrackerError> {
    use crate::tracker::ownership::{Field, Owner};

    if ownership.owner(Field::Title) != Owner::Contested {
        return Ok(Vec::new());
    }

    let mut proposed = Vec::new();

    for item in &report.remote {
        let Some(external_id) = item.external_id.as_deref() else {
            continue;
        };
        let Some((task_id, current_title)) = plan
            .tracker_link_by_external_id(provider, external_id)
            .map(|l| l.task_id.clone())
            .and_then(|id| {
                plan.plan()
                    .tasks
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| (c.id.clone(), c.title.clone()))
            })
        else {
            // A ticket we never projected. The sweep reports it; proposing a
            // title for a task that does not exist would be nonsense.
            continue;
        };

        if item.title == current_title {
            continue;
        }

        let reason = format!(
            "The tracker retitled {external_id} to \"{}\". The plan still says \
             \"{current_title}\". A remote retitle is a real statement about the \
             plan — but so is ours, so nobody wins silently: this is a suggestion, \
             not an edit.",
            item.title
        );
        let source = format!("ext:{provider}/{external_id}");

        plan.propose_title(&task_id, item.title.clone(), reason, source)
            .map_err(|error| proposal_failure(error, proposed.len()))?;
        proposed.push(task_id);
    }

    Ok(proposed)
}

/// The tracker's canonical state as a plan status.
///
/// Deliberately NOT the inverse of the projector's `state_of`: that maps five
/// task statuses onto four tracker states, so the round trip cannot be a
/// bijection and pretending otherwise would invent transitions nobody asked
/// for. `Todo` maps to `ToDo` rather than `Idea` because a ticket that
/// exists upstream is work someone has already decided to do.
fn status_for(state: crate::tracker::domain::WorkItemState) -> crate::plan::domain::TaskStatus {
    use crate::plan::domain::TaskStatus;
    use crate::tracker::domain::WorkItemState;
    match state {
        WorkItemState::Todo => TaskStatus::ToDo,
        WorkItemState::InProgress => TaskStatus::InProgress,
        WorkItemState::Done => TaskStatus::Done,
        WorkItemState::Cancelled => TaskStatus::Retired,
    }
}

fn proposal_failure(error: crate::plan::engine::PlanMutationError, saved: usize) -> TrackerError {
    let message =
        format!("Saved {saved} suggestions; the next suggestion could not be saved: {error}");
    let store = match error {
        crate::plan::engine::PlanMutationError::Store(error) => Some(error),
        crate::plan::engine::PlanMutationError::Rejected(_) => None,
    };
    TrackerError::Local { message, store }
}

#[cfg(test)]
#[path = "concern.tests.rs"]
mod tests;
