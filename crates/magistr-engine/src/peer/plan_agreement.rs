use std::collections::BTreeSet;

use super::{
    AgreementError, Peer, PeerIdentity, PlanBody, PlanReference, PlanRevision, RunContract,
};

/// Ephemeral dual approvals for the current immutable revision.
/// This aggregate does not authenticate runtimes or authorize dispatch.
#[derive(Debug)]
pub struct PlanAgreement {
    revision: PlanRevision,
    approvals: BTreeSet<Peer>,
}

impl PlanAgreement {
    pub fn new(contract: RunContract, body: PlanBody) -> Result<Self, AgreementError> {
        Ok(Self::from_revision(PlanRevision::seal(contract, body, 1)?))
    }

    /// Reopening a validated revision always starts without approvals.
    #[must_use]
    pub fn from_revision(revision: PlanRevision) -> Self {
        Self {
            revision,
            approvals: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn revision(&self) -> &PlanRevision {
        &self.revision
    }

    /// The host MUST supply identity from its authenticated session context.
    /// Never populate it from a model's approval payload. Duplicate votes are idempotent.
    pub fn approve(
        &mut self,
        authenticated_session: &PeerIdentity,
        reference: &PlanReference,
    ) -> Result<(), AgreementError> {
        if reference != &self.revision.reference() {
            return Err(AgreementError::StaleApproval);
        }
        if !self
            .revision
            .contract()
            .peers
            .contains(authenticated_session)
        {
            return Err(AgreementError::UnknownPeer);
        }
        self.approvals.insert(authenticated_session.peer);
        Ok(())
    }

    /// Only answers the vote question. Time, objections, budget and leases are separate.
    #[must_use]
    pub fn has_dual_approval(&self) -> bool {
        self.approvals.contains(&Peer::Astra) && self.approvals.contains(&Peer::Fable)
    }

    pub(super) fn revoke_approvals(&mut self) {
        self.approvals.clear();
    }

    /// Seal the next body under the original contract and invalidate BOTH votes.
    /// Even identical content gets a fresh revision. Invalid edits change nothing.
    pub fn revise(&mut self, body: PlanBody) -> Result<(), AgreementError> {
        let next = self.revision.next(body)?;
        self.revision = next;
        self.approvals.clear();
        Ok(())
    }
}
