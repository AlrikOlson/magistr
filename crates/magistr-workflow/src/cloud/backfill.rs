//! One-shot back-fill of the existing local corpus to the cloud.
//!
//! Cloud sync is write-through on *mutation* only, so an
//! agent that connects after it already has local history shows nothing in the
//! cloud until it next mutates each record. `magistr sync push` closes
//! that gap: it walks the persisted think/ship/roadmap/signal stores, builds the
//! same envelopes the write-through path builds (via [`crate::cloud::build`]),
//! and `PUT`s each to `/v1/records`.
//!
//! It is **idempotent** (the backend dedups on the per-record idempotency key,
//! returning `200` for a content-identical record) and therefore **resumable**:
//! re-running after a partial push simply re-PUTs everything, and the already-
//! landed records collapse to dedups.
//!
//! Scope mirrors the write-through path **exactly**: think steps, ship *actions*,
//! roadmap chunks, signals. Ship objectives/checks/tasks are not pushed by
//! write-through (no stable wire id) and stay deferred to `saas-cloud-sync-all`
//! — this back-fill does not silently widen that scope.

use crate::cloud::build;
use crate::cloud::client::{CloudClient, PushOutcome};
use crate::cloud::envelope::{Family, UnifiedRecordEnvelope};
use crate::roadmap::domain::Roadmap;
use crate::ship::domain::objective::Objective;
use crate::ship::domain::task::Task;
use crate::signal::domain::Signal;
use crate::think::domain::step::ThinkStep;

/// Per-family record counts in a collected back-fill (the dry-run report).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackfillCounts {
    pub think: usize,
    pub ship: usize,
    pub roadmap: usize,
    pub signal: usize,
}

impl BackfillCounts {
    /// Total records across all four families.
    #[must_use]
    pub fn total(&self) -> usize {
        self.think + self.ship + self.roadmap + self.signal
    }

    /// Derive the per-family breakdown from a built envelope list.
    #[must_use]
    pub fn from_envelopes(envelopes: &[UnifiedRecordEnvelope]) -> Self {
        let mut c = Self::default();
        for e in envelopes {
            match e.family {
                Family::Think => c.think += 1,
                Family::Ship => c.ship += 1,
                Family::Roadmap => c.roadmap += 1,
                Family::Signal => c.signal += 1,
            }
        }
        c
    }
}

/// Build the cloud envelopes for every local record across the four families,
/// reusing the write-through builders. Pure and network-free — the unit-test
/// seam for the whole back-fill. Order is stable: think, ship, roadmap, signal.
pub fn collect_envelopes<'a>(
    tenant: &str,
    steps: impl Iterator<Item = &'a ThinkStep>,
    ship: Option<(&Objective, &[Task])>,
    roadmap: &Roadmap,
    signals: &[Signal],
) -> Result<Vec<UnifiedRecordEnvelope>, magistr_workflow_contracts::ReadError> {
    let mut out = Vec::new();
    out.extend(
        steps
            .map(|s| build::from_step(tenant, s))
            .collect::<Result<Vec<_>, _>>()?,
    );
    // The ship cycle backfills with the SAME cycle-scoped identities as the
    // write-through push (sync-ship-full); without an objective `created_at`
    // there is no cycle key, so ship records are skipped (nothing to anchor).
    if let Some((objective, tasks)) = ship
        && let Some(created) = objective.created_at.as_deref()
    {
        let cycle = build::cycle_key(created);
        out.push(build::from_objective(tenant, &cycle, objective));
        for task in tasks {
            out.push(build::from_task(tenant, &cycle, created, task));
            for (seq, check) in task.checks.iter().enumerate() {
                out.push(build::from_check(tenant, &cycle, &task.id, seq, check));
            }
            out.extend(
                task.actions
                    .iter()
                    .map(|a| build::from_action(tenant, &cycle, a)),
            );
        }
    }
    // Carry each chunk's tracker state, exactly as the write-through push does
    // — a backfill that stripped it would un-bind every projected chunk on the
    // next machine to reconcile.
    out.extend(roadmap.chunks.iter().map(|c| {
        let links: Vec<_> = roadmap
            .links
            .iter()
            .filter(|l| l.chunk_id == c.id)
            .cloned()
            .collect();
        let opt_ins: Vec<_> = roadmap
            .tracker_opt_ins
            .iter()
            .filter(|o| o.chunk_id == c.id)
            .cloned()
            .collect();
        build::from_chunk(tenant, c, &links, &opt_ins)
    }));
    out.extend(signals.iter().map(|s| build::from_signal(tenant, s)));
    if out.iter().any(|envelope| envelope.id.trim().is_empty()) {
        return Err(magistr_workflow_contracts::ReadError::new(
            magistr_workflow_contracts::ReadErrorKind::Identity,
            "A record has no identity. Repair it before uploading.",
        ));
    }
    Ok(out)
}

/// The outcome of pushing a back-fill: how many records were newly created vs
/// deduped on the cloud, plus any per-record failures (identity + reason).
#[derive(Debug, Default)]
pub struct PushSummary {
    pub created: usize,
    pub deduped: usize,
    /// Records the cloud refused because its copy is further along the
    /// lifecycle than ours (`409 illegal_transition` on an unreachable
    /// target). That refusal is the no-clobber guard HOLDING — the cloud
    /// carries a fresher truth, e.g. a signal a human dismissed in the
    /// webapp while the local store still says `new` — so it is reported
    /// here, not in [`Self::failed`], and does not fail the run.
    pub kept: Vec<(String, String)>,
    /// `("<family>/<kind>/<id>", "<error>")` for each record that failed to push.
    pub failed: Vec<(String, String)>,
}

impl PushSummary {
    /// Records pushed without error (created + deduped).
    #[must_use]
    pub fn ok(&self) -> usize {
        self.created + self.deduped
    }
}

/// A short `family/kind/id` label for an envelope (used in failure reports).
fn label(e: &UnifiedRecordEnvelope) -> String {
    format!("{}/{}/{}", e.family.as_str(), e.kind.as_str(), e.id)
}

/// How many pushes are in flight at once. Each record is an independent PUT
/// keyed by its own idempotency key, so ordering between records carries no
/// meaning — the only reason this was ever sequential was small corpus size,
/// and a machine-wide back-fill is ~20k records where sequential means hours.
const PUSH_CONCURRENCY: usize = 12;

/// A `409 illegal_transition` is not a push failure: it means the cloud's
/// copy is further along the record's lifecycle than the local one, and the
/// backend kept the fresher truth. Everything else non-2xx is a real failure.
fn cloud_is_fresher(e: &crate::cloud::client::CloudError) -> bool {
    matches!(
        e,
        crate::cloud::client::CloudError::Status { status: 409, body }
            if body.contains("illegal_transition")
    )
}

/// Push every envelope to the cloud with bounded concurrency
/// (`PUSH_CONCURRENCY` in flight), tallying created/deduped, the records
/// the cloud kept its fresher copy of, and real failures. A single record's
/// failure never aborts the run — every record gets its attempt, and a re-run
/// is safe because already-landed records collapse to dedups.
pub async fn push_all(client: &CloudClient, envelopes: &[UnifiedRecordEnvelope]) -> PushSummary {
    use futures_util::stream::{self, StreamExt};

    let results: Vec<(String, Result<PushOutcome, _>)> = stream::iter(envelopes)
        .map(|envelope| async move { (label(envelope), client.push(envelope).await) })
        .buffer_unordered(PUSH_CONCURRENCY)
        .collect()
        .await;

    let mut summary = PushSummary::default();
    for (label, result) in results {
        match result {
            Ok(PushOutcome::Created) => summary.created += 1,
            Ok(PushOutcome::Deduped) => summary.deduped += 1,
            Err(e) if cloud_is_fresher(&e) => summary.kept.push((label, e.to_string())),
            Err(e) => summary.failed.push((label, e.to_string())),
        }
    }
    summary
}

#[cfg(test)]
#[path = "backfill.tests.rs"]
mod tests;
