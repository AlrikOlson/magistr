use serde::{Deserialize, Serialize};

use super::journal_version::JournalVersion;

use super::{
    AgreementError, PeerIdentity, PlanAgreement, PlanBody, PlanReference, PlanRevision,
    ReconciliationAction, ReconciliationObservation, ReconciliationState, ReconciliationStop,
    RunContract,
};

/// Bounded proposal/review protocol. The trusted host owns authentication, clocks,
/// usage metering and durable storage. This aggregate never authorizes dispatch.
#[derive(Debug)]
pub struct Reconciliation {
    agreement: PlanAgreement,
    proposals: [Option<PlanBody>; 2],
    round: u32,
    state: ReconciliationState,
    observation: ReconciliationObservation,
    journal: Journal,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema_version: JournalVersion,
    seed: Vec<u8>,
    entries: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    observation: ReconciliationObservation,
    action: ReconciliationAction,
}

impl Reconciliation {
    pub fn new(
        contract: RunContract,
        body: PlanBody,
        observation: ReconciliationObservation,
    ) -> Result<Self, AgreementError> {
        let mut value = Self::from_seed(
            PlanRevision::seal(contract, body, 1)?,
            JournalVersion::StartCapacity,
        )?;
        value.apply(observation, ReconciliationAction::Observe)?;
        Ok(value)
    }

    pub(super) fn from_seed(
        seed: PlanRevision,
        schema_version: JournalVersion,
    ) -> Result<Self, AgreementError> {
        if seed.reference().revision != 1 {
            return Err(AgreementError::Invalid(
                "reconciliation seed must be revision 1".into(),
            ));
        }
        let bytes = seed.to_json()?;
        Ok(Self {
            agreement: PlanAgreement::from_revision(seed),
            proposals: [None, None],
            round: 1,
            state: ReconciliationState::Collecting,
            observation: ReconciliationObservation {
                now_unix_ms: 0,
                total_tokens: 0,
                worker_attempts: 0,
            },
            journal: Journal {
                schema_version,
                seed: bytes,
                entries: Vec::new(),
            },
        })
    }

    #[must_use]
    pub fn revision(&self) -> &PlanRevision {
        self.agreement.revision()
    }

    /// Historical state as of observation(). Apply a fresh Observe before using it.
    /// Agreed still carries no worker authority, leases or objection clearance.
    #[must_use]
    pub fn state(&self) -> ReconciliationState {
        self.state
    }

    #[must_use]
    pub fn round(&self) -> u32 {
        self.round
    }

    #[must_use]
    pub fn observation(&self) -> ReconciliationObservation {
        self.observation
    }

    /// Withhold the pair until both fixed peers have submitted independently.
    #[must_use]
    pub fn proposals(&self) -> Option<[&PlanBody; 2]> {
        Some([self.proposals[0].as_ref()?, self.proposals[1].as_ref()?])
    }

    /// Validate and record one input. Errors leave the aggregate unchanged.
    /// A reached limit records a stop INSTEAD of executing the action and returns
    /// Stopped, not an error. The host must checkpoint it before further work.
    /// Rejected model output still consumes real tokens: report those through
    /// Observe or the next input's cumulative reading, never subtract them.
    pub fn apply(
        &mut self,
        observation: ReconciliationObservation,
        action: ReconciliationAction,
    ) -> Result<ReconciliationState, AgreementError> {
        if observation.now_unix_ms < self.observation.now_unix_ms
            || observation.total_tokens < self.observation.total_tokens
            || observation.worker_attempts < self.observation.worker_attempts
        {
            return Err(AgreementError::Invalid(
                "run observations must not decrease".into(),
            ));
        }
        if matches!(self.state, ReconciliationState::Stopped(_)) {
            if action != ReconciliationAction::Observe {
                return Err(AgreementError::Invalid("reconciliation has stopped".into()));
            }
        } else if let Some(reason) = self.limit_stop(observation) {
            self.stop(reason);
            // Persist the observation that stopped the run, not an unprocessed vote.
            self.record(observation, ReconciliationAction::Observe);
            return Ok(self.state);
        } else {
            self.apply_action(&action)?;
        }
        self.record(observation, action);
        Ok(self.state)
    }

    fn record(&mut self, observation: ReconciliationObservation, action: ReconciliationAction) {
        self.observation = observation;
        self.journal.entries.push(Entry {
            observation,
            action,
        });
    }

    pub(super) fn limit_stop(
        &self,
        observation: ReconciliationObservation,
    ) -> Option<ReconciliationStop> {
        let limits = &self.revision().contract().limits;
        if observation.now_unix_ms >= limits.deadline_unix_ms {
            Some(ReconciliationStop::Deadline)
        } else if observation.total_tokens >= limits.max_total_tokens {
            Some(ReconciliationStop::TokensExhausted)
        } else if self
            .journal
            .schema_version
            .stops_for_attempts(observation.worker_attempts, limits.max_attempts)
        {
            Some(ReconciliationStop::AttemptsExhausted)
        } else {
            None
        }
    }

    fn stop(&mut self, reason: ReconciliationStop) {
        self.agreement.revoke_approvals();
        self.state = ReconciliationState::Stopped(reason);
    }

    fn peer_index(&self, peer: &PeerIdentity) -> Result<usize, AgreementError> {
        self.revision()
            .contract()
            .peers
            .iter()
            .position(|fixed| fixed == peer)
            .ok_or(AgreementError::UnknownPeer)
    }

    fn require_reference(&self, reference: &PlanReference) -> Result<(), AgreementError> {
        if reference != &self.revision().reference() {
            return Err(AgreementError::StaleApproval);
        }
        Ok(())
    }

    fn require_body(&self, body: &PlanBody) -> Result<(), AgreementError> {
        body.validate(self.revision().contract())?;
        if body.context != self.revision().body().context {
            return Err(AgreementError::Invalid(
                "proposals and candidates must share the full context".into(),
            ));
        }
        Ok(())
    }

    fn require_review(&self) -> Result<(), AgreementError> {
        if !matches!(
            self.state,
            ReconciliationState::Reviewing | ReconciliationState::Agreed
        ) {
            return Err(AgreementError::Invalid(
                "no candidate is under review".into(),
            ));
        }
        Ok(())
    }

    fn apply_action(&mut self, action: &ReconciliationAction) -> Result<(), AgreementError> {
        match action {
            ReconciliationAction::Observe => {}
            ReconciliationAction::Propose { peer, base, body } => {
                let index = self.peer_index(peer)?;
                self.require_reference(base)?;
                self.require_body(body)?;
                if self.state != ReconciliationState::Collecting {
                    return Err(AgreementError::Invalid("proposal round is closed".into()));
                }
                if let Some(previous) = &self.proposals[index] {
                    if previous != body.as_ref() {
                        return Err(AgreementError::Invalid(
                            "peer already proposed different content".into(),
                        ));
                    }
                } else {
                    self.proposals[index] = Some(body.as_ref().clone());
                }
            }
            ReconciliationAction::Reconcile { base, body } => {
                self.require_reference(base)?;
                self.require_body(body)?;
                if self.state != ReconciliationState::Collecting || self.proposals().is_none() {
                    return Err(AgreementError::Invalid(
                        "both proposals are required before reconciliation".into(),
                    ));
                }
                self.agreement.revise(body.as_ref().clone())?;
                self.state = ReconciliationState::Reviewing;
            }
            ReconciliationAction::Approve { peer, plan } => {
                self.require_review()?;
                self.agreement.approve(peer, plan)?;
                if self.agreement.has_dual_approval() {
                    self.state = ReconciliationState::Agreed;
                }
            }
            ReconciliationAction::Disagree { peer, plan, reason } => {
                self.require_review()?;
                self.peer_index(peer)?;
                self.require_reference(plan)?;
                super::require_text("disagreement reason", reason)?;
                if self.round == self.revision().contract().limits.max_reconciliation_rounds {
                    self.stop(ReconciliationStop::RoundsExhausted);
                } else {
                    // Revision invalidates stale proposals/votes even before a new candidate.
                    self.agreement.revise(self.revision().body().clone())?;
                    self.round += 1; // strictly below the u32 contract ceiling
                    self.proposals = [None, None];
                    self.state = ReconciliationState::Collecting;
                }
            }
            ReconciliationAction::PeerUnavailable { peer } => {
                self.peer_index(peer)?;
                self.stop(ReconciliationStop::PeerUnavailable(peer.peer));
            }
        }
        Ok(())
    }

    /// Checkpoint bytes for protected host storage; votes are trusted journal events,
    /// not signed proofs. The host must persist atomically and prevent rollback.
    pub fn to_json(&self) -> Result<Vec<u8>, AgreementError> {
        Ok(serde_json::to_vec(&self.journal)?)
    }

    /// Rebuild every invariant by replay, then check fresh whole-run time and usage.
    /// This does not authenticate the journal or detect a valid older checkpoint.
    pub fn from_json(
        bytes: &[u8],
        expected_contract: &RunContract,
        observation: ReconciliationObservation,
    ) -> Result<Self, AgreementError> {
        let journal: Journal = serde_json::from_slice(bytes)?;
        if !matches!(
            journal.entries.first().map(|e| &e.action),
            Some(ReconciliationAction::Observe)
        ) {
            return Err(AgreementError::Invalid(
                "unsupported or incomplete reconciliation journal".into(),
            ));
        }
        let seed = PlanRevision::from_json(&journal.seed, expected_contract)?;
        let mut value = Self::from_seed(seed, journal.schema_version)?;
        for entry in journal.entries {
            // A live limit-reaching input is persisted as Observe. Such a journal
            // must already be canonical; never silently repair stored actions.
            if entry.action != ReconciliationAction::Observe
                && value.limit_stop(entry.observation).is_some()
            {
                return Err(AgreementError::Invalid(
                    "journal action at a reached limit must be an observation".into(),
                ));
            }
            value.apply(entry.observation, entry.action)?;
        }
        value.apply(observation, ReconciliationAction::Observe)?;
        Ok(value)
    }
}
