use super::*;
use crate::checks::CheckTarget;
use crate::{
    Capability, CommandReceipt, Revision,
    test_support::{command, ready, run},
};
use alloc::boxed::Box;
use magistr_domain::gate::{GateKind, GateOutcome, GateStatus};

struct FakeChecks {
    calls: usize,
    result: Result<CheckExecution, ApplicationError>,
}

impl CheckPort for FakeChecks {
    fn run_check(&mut self, _: RunCheck) -> crate::PortFuture<'_, CheckExecution> {
        self.calls += 1;
        Box::pin(core::future::ready(self.result.clone()))
    }
}

fn request() -> RunCheck {
    let current = run("project", "run");
    RunCheck {
        command: command(&current),
        gate: CheckTarget::standard(GateKind::Test),
        task: crate::workflow::TaskReference::new(current, "verification").unwrap(),
    }
}

fn execution(request: &RunCheck) -> CheckExecution {
    CheckExecution {
        receipt: CommandReceipt {
            attempt: request.command.attempt.clone(),
            revision: Revision(8),
        },
        gate: request.gate.clone(),
        outcome: GateOutcome {
            kind: request.gate.kind(),
            status: GateStatus::Passed,
            output: None,
            duration_ms: Some(12),
        },
        recording: CheckRecording::Unavailable {
            reason: "no active native verification task".into(),
        },
    }
}

#[test]
fn task_scope_is_checked_before_execution() {
    let mut request = request();
    let scope = ProjectScope::new(request.command.run().project().clone());
    let mut port = FakeChecks {
        calls: 0,
        result: Ok(execution(&request)),
    };
    request.task =
        crate::workflow::TaskReference::new(run("project", "other"), "verification").unwrap();
    assert!(matches!(
        ready(Checks::new(&scope, &mut port).run(request)),
        Err(ApplicationError::ScopeMismatch { .. })
    ));
    assert_eq!(port.calls, 0);
}

#[test]
fn passed_but_unrecorded_is_preserved_as_missing_evidence() {
    let request = request();
    let scope = ProjectScope::new(request.command.run().project().clone());
    let mut port = FakeChecks {
        calls: 0,
        result: Ok(execution(&request)),
    };
    let output = ready(Checks::new(&scope, &mut port).run(request)).unwrap();
    assert!(output.outcome.passed());
    assert!(matches!(
        output.recording,
        CheckRecording::Unavailable { .. }
    ));
    assert_eq!(port.calls, 1);
}

#[test]
fn gate_and_terminal_state_must_match_the_request() {
    for (kind, status) in [
        (GateKind::Lint, GateStatus::Passed),
        (GateKind::Test, GateStatus::Running),
    ] {
        let request = request();
        let scope = ProjectScope::new(request.command.run().project().clone());
        let mut output = execution(&request);
        output.outcome.kind = kind;
        output.outcome.status = status;
        let mut port = FakeChecks {
            calls: 0,
            result: Ok(output),
        };
        assert!(matches!(
            ready(Checks::new(&scope, &mut port).run(request)),
            Err(ApplicationError::InvalidResponse { .. })
        ));
    }
}

#[test]
fn capability_denial_is_forwarded_without_retrying() {
    let request = request();
    let scope = ProjectScope::new(request.command.run().project().clone());
    let error = ApplicationError::CapabilityDenied {
        capability: Capability::RunChecks,
        reason: "read-only owner".into(),
    };
    let mut port = FakeChecks {
        calls: 0,
        result: Err(error.clone()),
    };
    assert_eq!(
        ready(Checks::new(&scope, &mut port).run(request)),
        Err(error)
    );
    assert_eq!(port.calls, 1);
}

#[test]
fn gate_name_correlation_preserves_custom_and_archetype_identity() {
    for name in ["architecture-index", "test:app-typescript"] {
        let mut request = request();
        request.gate = CheckTarget::new(GateKind::Test, name).unwrap();
        let scope = ProjectScope::new(request.command.run().project().clone());
        let mut output = execution(&request);
        output.gate = CheckTarget::standard(GateKind::Test);
        let mut port = FakeChecks {
            calls: 0,
            result: Ok(output),
        };
        assert!(matches!(
            ready(Checks::new(&scope, &mut port).run(request)),
            Err(ApplicationError::InvalidResponse { .. })
        ));
    }
}

#[test]
fn recording_must_reference_the_requested_task() {
    let request = request();
    let scope = ProjectScope::new(request.command.run().project().clone());
    let mut output = execution(&request);
    output.recording = CheckRecording::Recorded {
        task: crate::workflow::TaskReference::new(request.command.run().clone(), "unrelated-task")
            .unwrap(),
        check_key: "check-1".into(),
    };
    let mut port = FakeChecks {
        calls: 0,
        result: Ok(output),
    };
    assert!(matches!(
        ready(Checks::new(&scope, &mut port).run(request)),
        Err(ApplicationError::InvalidResponse { .. })
    ));
}
