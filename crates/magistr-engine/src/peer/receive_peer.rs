use super::{
    AgreementError, AuthorityAction, AuthorityOutcome, PeerIdentity, PeerRequest,
    ReconciliationObservation, TaskAuthority,
};

impl TaskAuthority {
    /// Receive one peer message with identity and usage supplied by the trusted host.
    /// The host must authenticate the session independently of these untrusted bytes,
    /// including its exact provider/model/effort, and supply whole-run usage itself.
    ///
    /// Unlike apply, errors can still record the observation and expire leases.
    /// A reached or existing stop returns Stopped without interpreting the message.
    /// Checkpoint even on errors, and persist a successful permit BEFORE a runtime
    /// side effect. Serialize receipt, persistence, dispatch and invalidations through
    /// one authority; this method performs no I/O and authenticates no transport.
    pub fn receive_peer(
        &mut self,
        observation: ReconciliationObservation,
        authenticated_session: &PeerIdentity,
        bytes: &[u8],
    ) -> Result<AuthorityOutcome, AgreementError> {
        let observed = self.apply(observation, AuthorityAction::observe())?;
        if matches!(observed, AuthorityOutcome::Stopped(_)) {
            return Ok(observed);
        }
        if !self
            .reconciliation()
            .revision()
            .contract()
            .peers
            .contains(authenticated_session)
        {
            return Err(AgreementError::UnknownPeer);
        }
        let request = PeerRequest::decode(bytes)?;
        self.apply(
            observation,
            request.into_action(authenticated_session.clone()),
        )
    }
}
