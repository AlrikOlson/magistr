//! The request engine: owns the in-memory [`Requests`] store and mediates every
//! mutation, mirroring `plan::engine::PlanEngine` (a plain struct whose
//! concurrency guard lives at the service layer; builder-style
//! `with_persistence`; persist-on-mutation).
//!
//! Scope: local capture + read + validated status transitions. The
//! local store is a CACHE of the cloud system-of-record; the write-through to
//! the cloud belongs to the cloud client (`SyncTarget::Cloud`), a documented
//! no-op here.
//!
//! Dependency direction: depends on `crate::request::domain` (pure) and
//! `crate::infra` (persistence) — never the reverse (DIP).

mod editing;
mod mutation_error;
mod transaction;
mod upgrade;
pub use mutation_error::RequestMutationError;

use chrono::Utc;

use crate::cloud::client::CloudClient;
use crate::infra::{Persistence, StoreStamp};
use crate::request::broadcast::{Broadcaster, RequestFrame};
use crate::request::domain::{Enrichment, Request, RequestKind, RequestStatus, Requests};
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};

pub struct RequestEngine {
    requests: Requests,
    project_id: String,
    persistence: Option<Persistence>,
    /// Source of the last acknowledged save or adopted read. Observations
    /// validate the stored records independently of this writer's memory.
    stamp: Option<StoreStamp>,
    broadcaster: Option<Broadcaster>,
    /// Optional cloud sync client. When set, every mutation
    /// fire-and-forget pushes the request envelope to the cloud backend. `None`
    /// (default) = no cloud sync.
    cloud: Option<CloudClient>,
}

impl RequestEngine {
    pub fn new(project_id: String) -> Self {
        Self {
            requests: Requests {
                schema_version: magistr_workflow_contracts::REQUESTS_SCHEMA_VERSION,
                project_id: project_id.clone(),
                requests: Vec::new(),
            },
            project_id,
            persistence: None,
            stamp: None,
            broadcaster: None,
            cloud: None,
        }
    }

    /// The project this engine belongs to — the reconcile filter's identity
    /// (sync-project-scope): only records stamped with this id merge in.
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Attach the shared broadcaster so mutations emit `family="request"` frames
    /// through the one socket. Wired by the host's composition root.
    pub fn with_broadcaster(mut self, broadcaster: Broadcaster) -> Self {
        self.set_broadcaster(broadcaster);
        self
    }

    /// Attach the admitted owner's publisher to an already-open engine.
    pub fn set_broadcaster(&mut self, broadcaster: Broadcaster) {
        self.broadcaster = Some(broadcaster);
    }

    /// Attach a cloud client so every mutation fire-and-forget pushes the request
    /// envelope to the cloud backend. Wired by the host's composition root.
    pub fn with_cloud(mut self, client: CloudClient) -> Self {
        self.cloud = Some(client);
        self
    }

    /// Fire-and-forget: fan a mutation frame out to the broadcast socket, then
    /// push the request envelope to the cloud. A broadcast/push error is logged
    /// and dropped — a mutation is never failed by it.
    fn record_event(&self, frame: RequestFrame) {
        if let Some(b) = &self.broadcaster {
            b.emit(&frame);
        }
        self.cloud_push_frame(&frame);
    }

    /// Fire-and-forget cloud push of the frame's request. No-op
    /// without a cloud client OR outside a tokio runtime (so sync unit tests
    /// never panic); a push error is logged and dropped.
    fn cloud_push_frame(&self, frame: &RequestFrame) {
        let Some(client) = &self.cloud else {
            return;
        };
        let request = match frame {
            RequestFrame::RequestCaptured { request }
            | RequestFrame::RequestChanged { request }
            | RequestFrame::RequestDismissed { request } => request,
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let envelope = crate::cloud::build::from_request(&self.project_id, request);
        let client = client.clone();
        handle.spawn(async move {
            if let Err(e) = client.push(&envelope).await {
                tracing::warn!(target: "magistr_workflow::cloud", "request cloud push failed: {e}");
            }
        });
    }

    /// Attach a persistence handle, loading any prior request cache for this
    /// project off disk first (so state accumulates across conversations).
    pub fn with_persistence(mut self, persistence: Persistence) -> Self {
        self.persistence = Some(persistence);
        if let Err(error) = self.reload_from_store() {
            tracing::warn!(%error, "The request inbox could not be loaded");
        }
        self
    }

    /// Read validated stored requests without changing this writer's memory.
    pub fn observe(&self) -> Result<Observed<Requests>, ReadError> {
        let Some(persistence) = self.persistence.as_ref().filter(|p| p.enabled()) else {
            return Ok(Observed {
                value: self.requests.clone(),
                source: ReadSource::Memory,
            });
        };
        let read = persistence.load_stamped::<Requests>(&self.project_id)?;
        let value = read.value.unwrap_or_else(|| Requests {
            schema_version: magistr_workflow_contracts::REQUESTS_SCHEMA_VERSION,
            project_id: self.project_id.clone(),
            requests: Vec::new(),
        });
        validate(&value, &self.project_id)?;
        Ok(Observed {
            value,
            source: read.source,
        })
    }

    /// Validate this inbox's stored records before answering or acting.
    pub fn refresh_if_stale(&mut self) -> Result<bool, String> {
        self.reload_from_store()
    }

    /// Validate and adopt the stored inbox, keeping live wiring intact.
    pub fn reload_from_store(&mut self) -> Result<bool, String> {
        self.refresh_observed().map_err(|error| error.to_string())
    }

    pub(crate) fn refresh_observed(&mut self) -> Result<bool, ReadError> {
        let read = match self.observe() {
            Ok(read) => read,
            Err(error) => {
                self.stamp = None;
                return Err(error);
            }
        };
        let stamp = match read.source {
            ReadSource::Memory => return Ok(false),
            ReadSource::Missing => None,
            ReadSource::File(stamp) => Some(stamp),
        };
        let changed = self.stamp != stamp;
        self.requests = read.value;
        self.stamp = stamp;
        Ok(changed)
    }

    /// Read-only access for the wire layer.
    pub fn requests(&self) -> &Requests {
        &self.requests
    }

    /// Merge a request pulled from the cloud: insert when absent;
    /// replace an existing copy only when the incoming one has strictly
    /// progressed (`domain::request_wins` — the same lifecycle-rank rule as the
    /// disk merge), so a stale cloud copy never rolls back a fresh local
    /// transition (reconcile-recency-guard). A SILENT merge — it
    /// does NOT emit a mutation frame or push back to the cloud; a reconcile
    /// that re-emitted would loop the pull straight back into a push. Persists.
    pub fn upsert_request(
        &mut self,
        request: Request,
    ) -> Result<Observed<bool>, RequestMutationError> {
        self.mutate(|edit| edit.upsert_request(request))
    }

    fn index_of(&self, id: &str) -> Result<usize, String> {
        self.requests
            .requests
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| format!("request '{id}' not found"))
    }

    /// Capture a new request. Mints a UUIDv4 id and an RFC-3339 `created` stamp,
    /// starts it in `New`, and persists. (Cloud write-through happens via the
    /// attached cloud client, when one is wired.)
    pub fn capture(
        &mut self,
        kind: RequestKind,
        from: String,
        body: String,
    ) -> Result<Observed<Request>, RequestMutationError> {
        self.capture_linked(kind, from, body, Vec::new())
    }

    /// [`Self::capture`] carrying the optional structured body — the rich
    /// rendering sidecar the MCP seam steers writers toward. Validated by the
    /// caller; `None` is every legacy writer.
    /// Capture the request and its links as one authored record.
    pub fn capture_linked(
        &mut self,
        kind: RequestKind,
        from: String,
        body: String,
        refs: Vec<String>,
    ) -> Result<Observed<Request>, RequestMutationError> {
        self.mutate(|edit| {
            let id = edit.capture(kind, from, body)?.id.clone();
            for reference in refs {
                edit.link(&id, &reference)?;
            }
            edit.get(&id).cloned()
        })
    }

    pub fn capture_with_content(
        &mut self,
        kind: RequestKind,
        from: String,
        body: String,
        content: Option<crate::content::StructuredContent>,
    ) -> Result<Observed<Request>, RequestMutationError> {
        self.mutate(|edit| {
            edit.capture_with_content(kind, from, body, content)
                .cloned()
        })
    }

    /// Fetch a request by id.
    pub fn get(&self, id: &str) -> Result<&Request, String> {
        let idx = self.index_of(id)?;
        Ok(&self.requests.requests[idx])
    }

    /// Move a request to a new status, validated against the transition table.
    pub fn set_status(
        &mut self,
        id: &str,
        to: RequestStatus,
    ) -> Result<Observed<Request>, RequestMutationError> {
        self.mutate(|edit| edit.set_status(id, to).cloned())
    }

    /// Append an enrichment record. Does not change status on its own — the
    /// status-advancing churn op is [`Self::research`].
    pub fn enrich(
        &mut self,
        id: &str,
        enrichment: Enrichment,
    ) -> Result<Observed<Request>, RequestMutationError> {
        self.mutate(|edit| edit.enrich(id, enrichment).cloned())
    }

    /// Record one round of the agent "churning" on a request: append
    /// a durable [`Enrichment`] (stamped `at = now`), advance the lifecycle
    /// toward `Researched`, and — when a motivating `note` is given —
    /// cross-ref it as `decision:<N>` so the reasoning is auditable from the request.
    ///
    /// Status rules (validated, never backward): `New`/`Triaged` advance to
    /// `Researched`; an already-`Researched` request stays `Researched` (a request
    /// can be re-researched with more enrichment); a `Raised` request keeps its
    /// status (it's already past `Researched`) but still accrues enrichment;
    /// `Promoted`/`Dismissed` are terminal and rejected. `confidence` is clamped
    /// to `[0, 1]`.
    pub fn research(
        &mut self,
        id: &str,
        summary: String,
        confidence: f64,
        sources: Vec<String>,
        note: Option<u32>,
    ) -> Result<Observed<Request>, RequestMutationError> {
        self.mutate(|edit| {
            edit.research(id, summary, confidence, sources, note)
                .cloned()
        })
    }

    /// Attach a validated cross-ref to a request (deduped). Powers `request_link`
    /// and the reciprocal `task:<id>` written by promotion. The ref is
    /// validated + normalized via [`crate::infra::CrossRef`], so only the forms
    /// that type accepts get through: `decision:`, `step:`, `progress:`, `check:`,
    /// `task:`, `request:` and `ext:<provider>/<external_id>`.
    ///
    /// The `ext:` form arrived with the tracker seam and this list did not
    /// follow it for several phases — the behaviour was always right, because
    /// validation delegates, but the comment said otherwise. It is what a
    /// tracker divergence concern uses to name the ticket it is about.
    pub fn link(
        &mut self,
        id: &str,
        cross_ref: &str,
    ) -> Result<Observed<Request>, RequestMutationError> {
        self.mutate(|edit| edit.link(id, cross_ref).cloned())
    }

    /// Mark a request `Promoted`. Only a `Researched` or `Raised` request can be
    /// promoted — a stakeholder opportunity that's been validated becomes a
    /// plan solution. This is a higher-level op than [`Self::set_status`]
    /// (it may skip `Raised`); the bidirectional cross-ref wiring lives at the
    /// service layer. Idempotency (don't re-promote) is enforced there via the
    /// existing `task:` cross-ref.
    pub fn promote(&mut self, id: &str) -> Result<Observed<Request>, RequestMutationError> {
        self.mutate(|edit| edit.promote(id).cloned())
    }

    // ── Organic raising ───────────────────────────────────────────────────

    /// A request's raising confidence — the MAX confidence over its enrichment
    /// trail. A request with no enrichment scores 0.0, so an un-researched request
    /// can never clear a positive threshold (a structural guard, not a check).
    fn raising_confidence(s: &Request) -> f64 {
        s.enrichment
            .iter()
            .map(|e| e.confidence)
            .fold(0.0_f64, f64::max)
    }

    /// Whether `s` is currently snoozed (its `snooze_until` is in the future).
    fn is_snoozed(s: &Request, now: chrono::DateTime<chrono::Utc>) -> bool {
        match &s.snooze_until {
            Some(t) => chrono::DateTime::parse_from_rfc3339(t)
                .map(|dt| dt.with_timezone(&Utc) > now)
                .unwrap_or(false),
            None => false,
        }
    }

    /// Requests ready to raise to the human under earned-interruption
    /// discipline: `status == Researched`, max-enrichment-confidence ≥
    /// `min_confidence`, not already raised, not snoozed, and — when `hints` is
    /// non-empty — relevant (the body or a cross-ref contains a hint substring,
    /// case-insensitive). Relevance is HINT-DRIVEN: the caller supplies the
    /// active files/keywords, since the engine has no editor-state oracle.
    /// Highest-confidence first, capped at `limit`.
    pub fn ready(&self, min_confidence: f64, hints: &[String], limit: usize) -> Vec<&Request> {
        let now = Utc::now();
        let lc_hints: Vec<String> = hints
            .iter()
            .map(|h| h.to_lowercase())
            .filter(|h| !h.is_empty())
            .collect();
        let mut out: Vec<&Request> = self
            .requests
            .requests
            .iter()
            .filter(|s| {
                s.status == RequestStatus::Researched
                    && s.raised_at.is_none()
                    && Self::raising_confidence(s) >= min_confidence
                    && !Self::is_snoozed(s, now)
                    && (lc_hints.is_empty() || {
                        let body = s.body.to_lowercase();
                        lc_hints.iter().any(|h| {
                            body.contains(h) || s.refs.iter().any(|r| r.to_lowercase().contains(h))
                        })
                    })
            })
            .collect();
        out.sort_by(|a, b| {
            Self::raising_confidence(b)
                .partial_cmp(&Self::raising_confidence(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out.truncate(limit);
        out
    }

    /// How many requests are currently ready to raise — the count source for the
    /// `plan_status` ready-request badge. Same gate as
    /// [`Self::ready`] with no relevance hints.
    pub fn ready_count(&self, min_confidence: f64) -> usize {
        self.ready(min_confidence, &[], usize::MAX).len()
    }

    /// Mark a request as raised (raised to the human): `Researched → Raised`
    /// and stamp `raised_at`, so `ready` won't re-raise it.
    pub fn raise(&mut self, id: &str) -> Result<Observed<Request>, RequestMutationError> {
        self.mutate(|edit| edit.raise(id).cloned())
    }

    /// Snooze a request: suppress it from `ready` for `minutes`
    /// (`snooze_until = now + minutes`). Status is unchanged.
    pub fn snooze(
        &mut self,
        id: &str,
        minutes: i64,
    ) -> Result<Observed<Request>, RequestMutationError> {
        self.mutate(|edit| edit.snooze(id, minutes).cloned())
    }

    /// Dismiss a request (validated terminal transition).
    pub fn dismiss(&mut self, id: &str) -> Result<Observed<Request>, RequestMutationError> {
        self.mutate(|edit| edit.dismiss(id).cloned())
    }

    /// A bounded JSON snapshot: counts by status + a capped, newest-first list
    /// of request summaries. Mirrors `PlanEngine::status`'s "never blow the
    /// MCP output limit" discipline.
    pub fn status(&self) -> serde_json::Value {
        serde_json::to_value(crate::request::application::RequestInbox::from(
            self.requests(),
        ))
        .expect("request inbox contains serializable domain fields")
    }
}

fn validate(value: &Requests, project_id: &str) -> Result<(), ReadError> {
    if value.schema_version != magistr_workflow_contracts::REQUESTS_SCHEMA_VERSION {
        return Err(ReadError::new(
            ReadErrorKind::Interrupted,
            "Saved work is waiting for a one-time upgrade. Start Magistr to finish it.",
        ));
    }
    if value.project_id != project_id
        || value
            .requests
            .iter()
            .any(|request| request.project_id.as_deref() != Some(project_id))
    {
        return Err(ReadError::new(
            ReadErrorKind::Identity,
            "The saved requests could not be matched to this project.",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    if value
        .requests
        .iter()
        .any(|request| request.id.trim().is_empty() || !ids.insert(&request.id))
    {
        return Err(ReadError::new(
            ReadErrorKind::Identity,
            "The saved requests contain incomplete or duplicate items.",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::{Domain, Persistence, PersistenceConfig};
    use tempfile::TempDir;

    fn engine() -> RequestEngine {
        RequestEngine::new("proj".into())
    }

    #[test]
    fn capture_mints_id_and_starts_new() {
        let mut e = engine();
        let s = e.capture(RequestKind::Bug, "dana".into(), "it crashes".into());
        assert_eq!(s.as_ref().unwrap().value.status, RequestStatus::New);
        assert!(!s.as_ref().unwrap().value.id.is_empty());
        assert!(!s.as_ref().unwrap().value.created.is_empty());
        assert_eq!(s.as_ref().unwrap().value.kind, RequestKind::Bug);
    }

    #[test]
    fn get_finds_captured_and_errors_on_unknown() {
        let mut e = engine();
        let id = e
            .capture(RequestKind::Suggestion, "x".into(), "do the thing".into())
            .as_ref()
            .unwrap()
            .value
            .id
            .clone();
        assert_eq!(e.get(&id).unwrap().body, "do the thing");
        assert!(e.get("ghost").unwrap_err().contains("not found"));
    }

    #[test]
    fn set_status_enforces_transition_table() {
        let mut e = engine();
        let id = e
            .capture(RequestKind::Concern, "x".into(), "hmm".into())
            .as_ref()
            .unwrap()
            .value
            .id
            .clone();
        e.set_status(&id, RequestStatus::Triaged).unwrap();
        e.set_status(&id, RequestStatus::Researched).unwrap();
        // researched -> new is illegal.
        let err = e.set_status(&id, RequestStatus::New).unwrap_err();
        assert!(err.to_string().contains("illegal transition"));
    }

    #[test]
    fn status_counts_by_lifecycle_state() {
        let mut e = engine();
        e.capture(RequestKind::Bug, "a".into(), "one".into())
            .unwrap();
        let id = e
            .capture(RequestKind::Suggestion, "b".into(), "two".into())
            .as_ref()
            .unwrap()
            .value
            .id
            .clone();
        e.set_status(&id, RequestStatus::Triaged).unwrap();
        let s = e.status();
        assert_eq!(s["counts"]["total"], 2);
        assert_eq!(s["counts"]["new"], 1);
        assert_eq!(s["counts"]["triaged"], 1);
        assert_eq!(s["requests"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn research_appends_enrichment_advances_and_crossrefs_decision() {
        let mut e = engine();
        let id = e
            .capture(RequestKind::Concern, "dana".into(), "perf is bad".into())
            .as_ref()
            .unwrap()
            .value
            .id
            .clone(); // New
        let s = e
            .research(
                &id,
                "profiled it; the export is O(n^2)".into(),
                0.8,
                vec!["sym-plan::export".into()],
                Some(69),
            )
            .unwrap();
        // Advanced New -> Researched.
        assert_eq!(s.value.status, RequestStatus::Researched);
        // Enrichment recorded with stamped fields.
        assert_eq!(s.value.enrichment.len(), 1);
        assert_eq!(
            s.value.enrichment[0].summary,
            "profiled it; the export is O(n^2)"
        );
        assert_eq!(s.value.enrichment[0].confidence, 0.8);
        assert_eq!(s.value.enrichment[0].note, Some(69));
        assert!(!s.value.enrichment[0].at.is_empty());
        // The motivating decision note is cross-ref'd onto the request.
        assert!(s.value.refs.contains(&"decision:69".to_string()));
    }

    #[test]
    fn research_is_idempotent_on_status_and_accrues_enrichment() {
        let mut e = engine();
        let id = e
            .capture(RequestKind::Suggestion, "x".into(), "dark mode".into())
            .as_ref()
            .unwrap()
            .value
            .id
            .clone();
        e.research(&id, "round one".into(), 0.5, vec![], None)
            .unwrap();
        let s = e
            .research(&id, "round two".into(), 0.9, vec![], None)
            .unwrap();
        // Status stays Researched; both enrichments are kept.
        assert_eq!(s.value.status, RequestStatus::Researched);
        assert_eq!(s.value.enrichment.len(), 2);
    }

    #[test]
    fn research_clamps_confidence_and_rejects_terminal() {
        let mut e = engine();
        let id = e
            .capture(RequestKind::Bug, "x".into(), "boom".into())
            .as_ref()
            .unwrap()
            .value
            .id
            .clone();
        // Confidence above 1.0 is clamped.
        let s = e
            .research(&id, "found it".into(), 5.0, vec![], None)
            .unwrap();
        assert_eq!(s.value.enrichment[0].confidence, 1.0);
        // Dismiss it (terminal), then research must be rejected.
        e.set_status(&id, RequestStatus::Dismissed).unwrap();
        let err = e
            .research(&id, "too late".into(), 0.5, vec![], None)
            .unwrap_err();
        assert!(err.to_string().contains("terminal"));
    }

    #[test]
    fn research_enrichment_is_durable_across_restart() {
        let tmp = TempDir::new().unwrap();
        let cfg = PersistenceConfig::from_env()
            .with_data_dir(tmp.path().to_path_buf())
            .enabled(true);
        let id = {
            let mut e = RequestEngine::new("proj".into())
                .with_persistence(Persistence::new(&cfg, Domain::Request));
            let id = e
                .capture(RequestKind::Feedback, "x".into(), "nice".into())
                .as_ref()
                .unwrap()
                .value
                .id
                .clone();
            e.research(&id, "grounded it".into(), 0.7, vec!["url".into()], Some(42))
                .unwrap();
            id
        };
        let e2 = RequestEngine::new("proj".into())
            .with_persistence(Persistence::new(&cfg, Domain::Request));
        let s = e2.get(&id).unwrap();
        assert_eq!(s.status, RequestStatus::Researched);
        assert_eq!(s.enrichment.len(), 1);
        assert_eq!(s.enrichment[0].note, Some(42));
        assert!(s.refs.contains(&"decision:42".to_string()));
    }

    /// Capture a request and research it to `confidence` so it's `Researched`
    /// and raising-eligible. Returns its id.
    fn researched(e: &mut RequestEngine, body: &str, confidence: f64) -> String {
        let id = e
            .capture(RequestKind::Suggestion, "x".into(), body.into())
            .as_ref()
            .unwrap()
            .value
            .id
            .clone();
        e.research(&id, "looked into it".into(), confidence, vec![], None)
            .unwrap();
        id
    }

    #[test]
    fn ready_returns_only_researched_above_threshold() {
        let mut e = engine();
        e.capture(RequestKind::Bug, "x".into(), "raw, unresearched".into())
            .unwrap(); // New
        researched(&mut e, "low confidence", 0.2); // below threshold
        let hi = researched(&mut e, "high confidence", 0.9);
        let ready = e.ready(0.6, &[], 20);
        assert_eq!(ready.len(), 1, "only the researched, above-threshold one");
        assert_eq!(ready[0].id, hi);
    }

    #[test]
    fn ready_filters_by_relevance_hints() {
        let mut e = engine();
        researched(&mut e, "the export is slow", 0.9);
        let auth = researched(&mut e, "auth token refresh bug", 0.9);
        let hits = e.ready(0.6, &["auth".to_string()], 20);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, auth);
    }

    #[test]
    fn raise_marks_and_excludes_from_ready() {
        let mut e = engine();
        let id = researched(&mut e, "raise me", 0.9);
        assert_eq!(e.ready(0.6, &[], 20).len(), 1);
        let s = e.raise(&id).unwrap();
        assert_eq!(s.value.status, RequestStatus::Raised);
        assert!(s.value.raised_at.is_some());
        assert!(
            e.ready(0.6, &[], 20).is_empty(),
            "a raised request is not re-raised"
        );
        // A non-researched request cannot be raised.
        let raw = e
            .capture(RequestKind::Bug, "x".into(), "raw".into())
            .as_ref()
            .unwrap()
            .value
            .id
            .clone();
        assert!(e.raise(&raw).is_err());
    }

    #[test]
    fn snooze_suppresses_until_expiry() {
        let mut e = engine();
        let id = researched(&mut e, "snooze me", 0.9);
        e.snooze(&id, 60).unwrap();
        assert!(
            e.ready(0.6, &[], 20).is_empty(),
            "a snoozed request is hidden"
        );
        // A zero/expired snooze no longer suppresses.
        e.snooze(&id, 0).unwrap();
        assert_eq!(
            e.ready(0.6, &[], 20).len(),
            1,
            "an expired snooze re-raises the request"
        );
    }

    #[test]
    fn dismiss_drops_from_ready() {
        let mut e = engine();
        let id = researched(&mut e, "dismiss me", 0.9);
        let s = e.dismiss(&id).unwrap();
        assert_eq!(s.value.status, RequestStatus::Dismissed);
        assert!(e.ready(0.6, &[], 20).is_empty());
    }

    #[test]
    fn ready_count_counts_only_ready_requests() {
        let mut e = engine();
        researched(&mut e, "ready one", 0.9);
        researched(&mut e, "ready two", 0.7);
        researched(&mut e, "too low", 0.2); // below threshold
        e.capture(RequestKind::Bug, "x".into(), "raw".into())
            .unwrap(); // un-researched
        assert_eq!(e.ready_count(0.6), 2);
    }

    #[test]
    fn persistence_round_trips_across_engine_instances() {
        let tmp = TempDir::new().unwrap();
        let cfg = PersistenceConfig::from_env()
            .with_data_dir(tmp.path().to_path_buf())
            .enabled(true);

        let id = {
            let mut e = RequestEngine::new("proj".into())
                .with_persistence(Persistence::new(&cfg, Domain::Request));
            let id = e
                .capture(RequestKind::Feedback, "dana".into(), "nice".into())
                .as_ref()
                .unwrap()
                .value
                .id
                .clone();
            e.set_status(&id, RequestStatus::Triaged).unwrap();
            id
        };

        // A fresh engine on the same data dir loads the cache from disk.
        let e2 = RequestEngine::new("proj".into())
            .with_persistence(Persistence::new(&cfg, Domain::Request));
        assert_eq!(e2.requests().requests.len(), 1);
        let s = e2.get(&id).unwrap();
        assert_eq!(s.status, RequestStatus::Triaged);
        assert_eq!(s.from, "dana");
    }
}
