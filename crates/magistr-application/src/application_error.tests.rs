use super::*;
use crate::test_support::{command, run};

#[test]
fn unknown_effects_and_bad_acknowledgements_require_reconciliation() {
    let request = command(&run("project", "run"));
    let unknown = ApplicationError::UnknownOutcome {
        attempt: request.attempt.clone(),
    };
    assert_eq!(unknown.recovery(), RecoveryAction::ReconcileOutcome);
    assert_eq!(
        ApplicationError::InvalidResponse {
            reason: "lost correlation".into()
        }
        .recovery(),
        RecoveryAction::ReconcileOutcome
    );
    assert_ne!(
        unknown.recovery(),
        ApplicationError::PersistenceFailure {
            operation: "load before dispatch".into()
        }
        .recovery()
    );
}

#[test]
fn revision_capability_and_provider_failures_retain_distinct_recovery() {
    let session = SessionId::new(run("project", "run"), "session").expect("session");
    let cases = [
        (
            ApplicationError::RevisionConflict {
                expected: Revision(7),
                actual: Revision(8),
            },
            RecoveryAction::RefreshRevision,
        ),
        (
            ApplicationError::CapabilityDenied {
                capability: Capability::Prompt,
                reason: "policy".into(),
            },
            RecoveryAction::ReviewAuthorization,
        ),
        (
            ApplicationError::CapabilityUnavailable {
                capability: Capability::Prompt,
            },
            RecoveryAction::SelectAvailableCapability,
        ),
        (
            ApplicationError::ProviderDisconnected { session },
            RecoveryAction::ReconnectProvider,
        ),
    ];
    for (error, recovery) in cases {
        assert_eq!(error.recovery(), recovery);
    }
}
