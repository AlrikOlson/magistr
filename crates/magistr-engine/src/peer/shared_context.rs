use std::collections::BTreeSet;

use super::{AgreementError, Peer, PeerBriefing, PeerIdentity, PlanReference};

/// Delivery and acknowledgements are journaled by the owning TaskAuthority.
#[derive(Debug)]
pub(super) struct SharedContext {
    briefing: PeerBriefing,
    delivered: BTreeSet<Peer>,
    acknowledged: BTreeSet<Peer>,
}

impl SharedContext {
    pub(super) fn new(briefing: PeerBriefing) -> Self {
        Self {
            briefing,
            delivered: BTreeSet::new(),
            acknowledged: BTreeSet::new(),
        }
    }

    pub(super) fn packet(&self, peer: &PeerIdentity) -> Result<&[u8], AgreementError> {
        self.require_peer(peer)?;
        Ok(self.briefing.bytes())
    }

    fn require_peer(&self, peer: &PeerIdentity) -> Result<(), AgreementError> {
        if !self.briefing.revision().contract().peers.contains(peer) {
            return Err(AgreementError::UnknownPeer);
        }
        Ok(())
    }

    pub(super) fn validate(
        &self,
        peer: &PeerIdentity,
        reference: &PlanReference,
        acknowledge: bool,
    ) -> Result<(), AgreementError> {
        self.require_peer(peer)?;
        if reference != &self.briefing.revision().reference() {
            return Err(AgreementError::StaleApproval);
        }
        if acknowledge && !self.delivered.contains(&peer.peer) {
            return Err(AgreementError::Invalid(
                "briefing must be delivered before acknowledgement".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn record_delivery(&mut self, peer: Peer) {
        self.delivered.insert(peer);
    }
    pub(super) fn record_acknowledgement(&mut self, peer: Peer) {
        self.acknowledged.insert(peer);
    }

    pub(super) fn require_ready(&self) -> Result<(), AgreementError> {
        if self.acknowledged.len() != 2 {
            return Err(AgreementError::Invalid(
                "both peers must acknowledge the shared briefing before proposing".into(),
            ));
        }
        Ok(())
    }
}
