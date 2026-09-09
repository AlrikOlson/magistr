use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::journal_version::JournalVersion;
use super::shared_context::SharedContext;
use super::worker_admission::WorkerAdmission;
use super::{PeerBriefing, WorkerPermit};

use super::{
    AgreementError, AuthorityAction, AuthorityOutcome, InstructionLease, PeerIdentity, PlanBody,
    PlanReference, PlanRevision, PlanTask, Reconciliation, ReconciliationAction,
    ReconciliationObservation, ReconciliationState, RunContract,
};

/// Task objections and fenced instruction authority above the reconciliation protocol.
/// The host owns authentication, metering, protected storage and atomic operation checks.
#[derive(Debug)]
pub struct TaskAuthority {
    reconciliation: Reconciliation,
    objections: BTreeMap<String, Objection>,
    used_ids: BTreeSet<String>,
    leases: BTreeMap<String, InstructionLease>,
    generation: u64,
    workers: WorkerAdmission,
    shared_context: Option<SharedContext>,
    journal: Journal,
}

#[derive(Debug)]
struct Objection {
    action: AuthorityAction,
    affected: BTreeSet<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema_version: JournalVersion,
    seed: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    briefing: Option<Vec<u8>>,
    entries: Vec<Entry>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    observation: ReconciliationObservation,
    action: AuthorityAction,
}

impl TaskAuthority {
    pub fn new(
        contract: RunContract,
        body: PlanBody,
        observation: ReconciliationObservation,
    ) -> Result<Self, AgreementError> {
        let mut value = Self::from_seed(
            PlanRevision::seal(contract, body, 1)?,
            JournalVersion::StartCapacity,
        )?;
        value.apply(observation, AuthorityAction::observe())?;
        Ok(value)
    }

    /// Start the shared-evidence protocol. The legacy new constructor remains ungated.
    pub fn with_briefing(
        briefing: PeerBriefing,
        observation: ReconciliationObservation,
    ) -> Result<Self, AgreementError> {
        let revision = briefing.revision();
        let seed = PlanRevision::from_json(&revision.to_json()?, revision.contract())?;
        let mut value = Self::from_seed(seed, JournalVersion::SharedBriefing)?;
        value.journal.briefing = Some(briefing.to_json()?);
        value.shared_context = Some(SharedContext::new(briefing));
        value.apply(observation, AuthorityAction::observe())?;
        Ok(value)
    }

    /// Read common bytes for a fixed session. Reading alone records no delivery.
    /// This historical accessor grants no permission to start a model call.
    pub fn briefing_for(
        &self,
        authenticated_session: &PeerIdentity,
    ) -> Result<&[u8], AgreementError> {
        self.require_shared_context()?.packet(authenticated_session)
    }

    fn require_shared_context(&self) -> Result<&SharedContext, AgreementError> {
        self.shared_context
            .as_ref()
            .ok_or_else(|| AgreementError::Invalid("run has no shared briefing".into()))
    }

    fn from_seed(
        seed: PlanRevision,
        schema_version: JournalVersion,
    ) -> Result<Self, AgreementError> {
        if seed.reference().revision != 1 {
            return Err(AgreementError::Invalid(
                "authority seed must be revision 1".into(),
            ));
        }
        let seed_bytes = seed.to_json()?;
        let mut reconciliation = Reconciliation::from_seed(seed, schema_version)?;
        reconciliation.apply(
            ReconciliationObservation {
                now_unix_ms: 0,
                total_tokens: 0,
                worker_attempts: 0,
            },
            ReconciliationAction::Observe,
        )?;
        Ok(Self {
            reconciliation,
            objections: BTreeMap::new(),
            used_ids: BTreeSet::new(),
            leases: BTreeMap::new(),
            generation: 0,
            workers: WorkerAdmission::default(),
            shared_context: None,
            journal: Journal {
                schema_version,
                seed: seed_bytes,
                briefing: None,
                entries: Vec::new(),
            },
        })
    }

    /// Read-only access prevents protocol changes from bypassing lease revocation.
    #[must_use]
    pub fn reconciliation(&self) -> &Reconciliation {
        &self.reconciliation
    }

    /// Historical objection closure, not a task readiness or dispatch decision.
    #[must_use]
    pub fn paused_tasks(&self) -> BTreeSet<String> {
        self.objections
            .values()
            .flat_map(|o| o.affected.iter().cloned())
            .collect()
    }

    /// Active objections, including their fixed author, original revision and reason.
    pub fn objections(&self) -> impl Iterator<Item = &AuthorityAction> {
        self.objections.values().map(|objection| &objection.action)
    }

    /// Invalid inputs leave state unchanged. Reached limits instead record an Observe
    /// and return Stopped; the requested action was not executed. Checkpoint that stop.
    /// RetireWorker is the exception: host termination receipts remain recordable
    /// after a stop and return Updated without restoring any authority.
    pub fn apply(
        &mut self,
        observation: ReconciliationObservation,
        mut action: AuthorityAction,
    ) -> Result<AuthorityOutcome, AgreementError> {
        if !matches!(self.reconciliation.state(), ReconciliationState::Stopped(_))
            && self.reconciliation.limit_stop(observation).is_some()
            && !matches!(action, AuthorityAction::RetireWorker { .. })
        {
            action = AuthorityAction::observe();
        }
        self.validate_action(observation, &action)?;
        let before = self.reconciliation.state();
        let reference = self.reconciliation.revision().reference();
        let protocol = match &action {
            AuthorityAction::Protocol { action } => action.clone(),
            _ => ReconciliationAction::Observe,
        };
        let state = self.reconciliation.apply(observation, protocol)?;
        if state != ReconciliationState::Agreed
            || reference != self.reconciliation.revision().reference()
        {
            self.leases.clear();
        }
        self.workers.observe(observation.worker_attempts);
        if reference != self.reconciliation.revision().reference() {
            self.workers.invalidate_all();
        }
        if before != ReconciliationState::Agreed && state == ReconciliationState::Agreed {
            // Both fixed peers explicitly accepted the replacement; neither can do this alone.
            self.objections.clear();
        }
        self.leases
            .retain(|_, lease| lease.expires_unix_ms > observation.now_unix_ms);
        let outcome = if matches!(action, AuthorityAction::RetireWorker { .. }) {
            self.execute_action(&action)
        } else if let ReconciliationState::Stopped(reason) = state {
            AuthorityOutcome::Stopped(reason)
        } else {
            self.execute_action(&action)
        };
        self.journal.entries.push(Entry {
            observation,
            action,
        });
        Ok(outcome)
    }

    fn require_peer_plan(
        &self,
        peer: &PeerIdentity,
        plan: &PlanReference,
    ) -> Result<(), AgreementError> {
        if plan != &self.reconciliation.revision().reference() {
            return Err(AgreementError::StaleApproval);
        }
        if !self
            .reconciliation
            .revision()
            .contract()
            .peers
            .contains(peer)
        {
            return Err(AgreementError::UnknownPeer);
        }
        if self.reconciliation.state() != ReconciliationState::Agreed {
            return Err(AgreementError::Invalid(
                "task authority requires dual agreement".into(),
            ));
        }
        Ok(())
    }

    fn task(&self, id: &str) -> Result<&PlanTask, AgreementError> {
        self.reconciliation
            .revision()
            .body()
            .tasks
            .iter()
            .find(|task| task.id == id)
            .ok_or_else(|| AgreementError::Invalid(format!("unknown authority task {id}")))
    }

    fn validate_action(
        &self,
        observation: ReconciliationObservation,
        action: &AuthorityAction,
    ) -> Result<(), AgreementError> {
        match action {
            AuthorityAction::AdmitWorker {
                peer,
                plan,
                task,
                lease_generation,
                ..
            } => {
                self.require_worker_lease(observation, peer, plan, task, *lease_generation)?;
                self.workers.validate_admission(
                    action,
                    self.reconciliation.revision(),
                    observation.worker_attempts,
                )?;
            }
            AuthorityAction::AcceptWorker { peer, id, receipt } => {
                super::require_text("worker acceptance receipt", receipt)?;
                let permit = self.workers.active(id)?;
                let lease = permit.lease();
                self.require_worker_lease(
                    observation,
                    peer,
                    lease.plan(),
                    lease.task(),
                    lease.generation(),
                )?;
            }
            AuthorityAction::RetireWorker { id, receipt } => {
                super::require_text("worker termination receipt", receipt)?;
                self.workers.active(id)?;
            }
            AuthorityAction::DeliverBriefing { peer, briefing } => {
                self.require_shared_context()?
                    .validate(peer, briefing, false)?;
            }
            AuthorityAction::AcknowledgeBriefing { peer, briefing } => {
                self.require_shared_context()?
                    .validate(peer, briefing, true)?;
            }
            AuthorityAction::Protocol {
                action: ReconciliationAction::Propose { .. },
            } => {
                if let Some(shared) = &self.shared_context {
                    shared.require_ready()?;
                }
            }
            AuthorityAction::Protocol { .. } => {}
            AuthorityAction::Object {
                peer,
                plan,
                id,
                tasks,
                reason,
            } => {
                self.require_peer_plan(peer, plan)?;
                super::require_text("objection id", id)?;
                super::require_text("objection reason", reason)?;
                super::require_list("objection tasks", tasks)?;
                let mut seen = BTreeSet::new();
                for task in tasks {
                    self.task(task)?;
                    if !seen.insert(task) {
                        return Err(AgreementError::Invalid("duplicate objection task".into()));
                    }
                }
                if self.used_ids.contains(id)
                    && self.objections.get(id).is_none_or(|o| &o.action != action)
                {
                    return Err(AgreementError::Invalid("objection id already used".into()));
                }
            }
            AuthorityAction::Resolve { peer, plan, id } => {
                self.require_peer_plan(peer, plan)?;
                let own = self.objections.get(id).is_some_and(|o| {
                    matches!(&o.action, AuthorityAction::Object { peer: author, .. } if author == peer)
                });
                if !own {
                    return Err(AgreementError::Invalid(
                        "only the objector may resolve an active objection".into(),
                    ));
                }
            }
            AuthorityAction::IssueLease {
                peer,
                plan,
                task,
                expires_unix_ms,
            } => {
                self.require_peer_plan(peer, plan)?;
                if self.task(task)?.owner != peer.peer || self.paused_tasks().contains(task) {
                    return Err(AgreementError::Invalid(
                        "task is paused or has a different owner".into(),
                    ));
                }
                if *expires_unix_ms <= observation.now_unix_ms
                    || *expires_unix_ms
                        > self
                            .reconciliation
                            .revision()
                            .contract()
                            .limits
                            .deadline_unix_ms
                {
                    return Err(AgreementError::Invalid(
                        "lease expiry must be future and within the run deadline".into(),
                    ));
                }
                self.generation.checked_add(1).ok_or_else(|| {
                    AgreementError::Invalid("instruction generation exhausted".into())
                })?;
            }
        }
        Ok(())
    }

    fn require_worker_lease(
        &self,
        observation: ReconciliationObservation,
        peer: &PeerIdentity,
        plan: &PlanReference,
        task: &str,
        generation: u64,
    ) -> Result<(), AgreementError> {
        self.require_peer_plan(peer, plan)?;
        if self.leases.get(task).is_none_or(|lease| {
            lease.owner() != peer
                || lease.plan() != plan
                || lease.generation() != generation
                || lease.expires_unix_ms() <= observation.now_unix_ms
        }) {
            return Err(AgreementError::Invalid(
                "worker instruction lease is not current".into(),
            ));
        }
        Ok(())
    }

    fn closure(&self, tasks: &[String]) -> BTreeSet<String> {
        let mut affected: BTreeSet<_> = tasks.iter().cloned().collect();
        loop {
            let before = affected.len();
            for task in &self.reconciliation.revision().body().tasks {
                if task
                    .dependencies
                    .iter()
                    .any(|dependency| affected.contains(dependency))
                {
                    affected.insert(task.id.clone());
                }
            }
            if before == affected.len() {
                return affected;
            }
        }
    }

    fn execute_action(&mut self, action: &AuthorityAction) -> AuthorityOutcome {
        match action {
            AuthorityAction::AdmitWorker { task, .. } => {
                let lease = self
                    .leases
                    .get(task)
                    .expect("validated worker lease remains current")
                    .clone();
                return self
                    .workers
                    .admit(action, lease)
                    .map_or(AuthorityOutcome::AlreadyAdmitted, |permit| {
                        AuthorityOutcome::Worker(Box::new(permit))
                    });
            }
            AuthorityAction::AcceptWorker { id, receipt, .. } => {
                self.workers.settle(id, receipt, true)
            }
            AuthorityAction::RetireWorker { id, receipt } => {
                self.workers.settle(id, receipt, false)
            }
            AuthorityAction::DeliverBriefing { peer, .. } => {
                self.shared_context
                    .as_mut()
                    .expect("validated shared briefing")
                    .record_delivery(peer.peer);
            }
            AuthorityAction::AcknowledgeBriefing { peer, .. } => {
                self.shared_context
                    .as_mut()
                    .expect("validated shared briefing")
                    .record_acknowledgement(peer.peer);
            }
            AuthorityAction::Protocol { .. } => {}
            AuthorityAction::Object { id, tasks, .. } => {
                if !self.used_ids.contains(id) {
                    let affected = self.closure(tasks);
                    self.leases.retain(|task, _| !affected.contains(task));
                    self.workers.invalidate(&affected);
                    self.objections.insert(
                        id.clone(),
                        Objection {
                            action: action.clone(),
                            affected,
                        },
                    );
                    self.used_ids.insert(id.clone());
                }
            }
            AuthorityAction::Resolve { id, .. } => {
                self.objections.remove(id);
            }
            AuthorityAction::IssueLease {
                peer,
                plan,
                task,
                expires_unix_ms,
            } => {
                self.generation += 1; // checked before any mutation
                let lease = InstructionLease {
                    plan: plan.clone(),
                    task: task.clone(),
                    owner: peer.clone(),
                    generation: self.generation,
                    expires_unix_ms: *expires_unix_ms,
                };
                self.leases.insert(task.clone(), lease.clone());
                return AuthorityOutcome::Lease(Box::new(lease));
            }
        }
        AuthorityOutcome::Updated
    }

    /// Record fresh time/usage even if validation fails, then check the current fence.
    /// The host must serialize this check with its instruction operation and invalidations.
    /// Success does not classify instruction text or authorize a worker start.
    pub fn validate_lease(
        &mut self,
        observation: ReconciliationObservation,
        authenticated_session: &PeerIdentity,
        lease: &InstructionLease,
    ) -> Result<(), AgreementError> {
        self.apply(observation, AuthorityAction::observe())?;
        if self.reconciliation.state() != ReconciliationState::Agreed
            || authenticated_session != &lease.owner
            || self.leases.get(&lease.task) != Some(lease)
        {
            return Err(AgreementError::Invalid(
                "instruction lease is not current for this owner".into(),
            ));
        }
        Ok(())
    }

    /// Check current worker authority at a receiving operation, recording fresh usage first.
    /// This does not authorize a second launch for an already admitted request.
    pub fn validate_worker(
        &mut self,
        observation: ReconciliationObservation,
        authenticated_session: &PeerIdentity,
        permit: &WorkerPermit,
    ) -> Result<(), AgreementError> {
        self.validate_lease(observation, authenticated_session, permit.lease())?;
        if self.workers.active(permit.id())? != permit {
            return Err(AgreementError::Invalid(
                "worker permit is not current".into(),
            ));
        }
        Ok(())
    }

    /// Historical unsettled attempts, including those whose authority was revoked.
    /// Reconcile actual runtime termination before replacing these workers.
    pub fn unsettled_workers(&self) -> impl Iterator<Item = &WorkerPermit> {
        self.workers.unsettled()
    }

    #[must_use]
    pub fn worker_receipt(&self, id: &str) -> Option<&str> {
        self.workers.receipt(id)
    }

    /// Checkpoint for protected host storage. The journal is not an authenticated proof.
    pub fn to_json(&self) -> Result<Vec<u8>, AgreementError> {
        Ok(serde_json::to_vec(&self.journal)?)
    }

    /// Validate by replay, observe fresh whole-run limits, then discard all live leases.
    /// The host must prevent rollback and concurrent authorities over the same run.
    pub fn from_json(
        bytes: &[u8],
        expected_contract: &RunContract,
        observation: ReconciliationObservation,
    ) -> Result<Self, AgreementError> {
        let journal: Journal = serde_json::from_slice(bytes)?;
        if journal
            .entries
            .first()
            .is_none_or(|entry| entry.action != AuthorityAction::observe())
        {
            return Err(AgreementError::Invalid(
                "unsupported or incomplete authority journal".into(),
            ));
        }
        let mut value = Self::from_seed(
            PlanRevision::from_json(&journal.seed, expected_contract)?,
            journal.schema_version,
        )?;
        match (journal.schema_version, journal.briefing) {
            (JournalVersion::SharedBriefing, Some(bytes)) => {
                let briefing = PeerBriefing::from_json(&bytes, expected_contract)?;
                if briefing.revision().reference() != value.reconciliation.revision().reference() {
                    return Err(AgreementError::DigestMismatch);
                }
                value.journal.briefing = Some(briefing.to_json()?);
                value.shared_context = Some(SharedContext::new(briefing));
            }
            (JournalVersion::LegacyTerminal | JournalVersion::StartCapacity, None) => {}
            _ => {
                return Err(AgreementError::Invalid(
                    "briefing does not match authority journal version".into(),
                ));
            }
        }
        for entry in journal.entries {
            value.apply(entry.observation, entry.action.clone())?;
            if value.journal.entries.last() != Some(&entry) {
                return Err(AgreementError::Invalid(
                    "noncanonical authority journal action".into(),
                ));
            }
        }
        value.apply(observation, AuthorityAction::observe())?;
        value.leases.clear();
        Ok(value)
    }
}
