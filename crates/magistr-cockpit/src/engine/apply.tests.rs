use magistr_workflow_contracts::{Delivery, NativeEvent, PlanFrame, SignalFrame};
use magistr_workflow_contracts::{Signal, SignalStatus, Task, TaskStatus};
use serde_json::json;

use super::*;
use magistr_workflow_contracts::{ReadSource, signal::SignalPromotion};

fn observed<T>(value: T) -> Observed<T> {
    Observed {
        value,
        source: ReadSource::Memory,
    }
}
use crate::harness::fixtures;

struct Port {
    pins: Vec<(u32, bool)>,
    fail: bool,
}

fn task(id: &str, status: TaskStatus) -> Task {
    let mut task = fixtures::task(id);
    task.status = status;
    task
}

fn signal(id: &str, status: SignalStatus) -> Signal {
    let mut signal = fixtures::signal(id);
    signal.status = status;
    signal
}

impl EnginePort for Port {
    fn pin(
        &mut self,
        step_number: u32,
        pinned: bool,
    ) -> Result<Observed<bool>, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("no such step"));
        }
        self.pins.push((step_number, pinned));
        Ok(observed(false))
    }

    fn checkpoint(&mut self) -> Result<serde_json::Value, String> {
        if self.fail {
            return Err("engine gone".into());
        }
        Ok(json!({"open_hypotheses": []}))
    }

    fn start_task(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("no such task"));
        }
        Ok(observed(task(id, TaskStatus::InProgress)))
    }

    fn complete_task(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("no such task"));
        }
        Ok(observed(task(id, TaskStatus::Done)))
    }

    fn resolve_proposal(
        &mut self,
        id: &str,
        _kind: crate::cockpit::ProposalKind,
        _accept: bool,
    ) -> Result<Observed<Task>, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("no open proposal"));
        }
        Ok(observed(task(id, TaskStatus::ToDo)))
    }

    fn clear_blocker(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected(
                "no waiting reason to clear",
            ));
        }
        Ok(observed(task(id, TaskStatus::ToDo)))
    }

    fn promote_signal(&mut self, id: &str) -> Result<SignalPromotion, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("not researched"));
        }
        Ok(SignalPromotion {
            signal_id: id.into(),
            task_id: "new".into(),
            created: true,
            signal: observed(signal(id, SignalStatus::Promoted)),
            task: observed(task("new", TaskStatus::Idea)),
        })
    }

    fn ignore_signal(&mut self, id: &str) -> Result<Observed<Signal>, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("no such signal"));
        }
        Ok(observed(signal(id, SignalStatus::Dismissed)))
    }

    fn snooze_signal(
        &mut self,
        id: &str,
        _minutes: u32,
    ) -> Result<Observed<Signal>, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("no such signal"));
        }
        let mut signal = signal(id, SignalStatus::Researched);
        signal.snooze_until = Some("soon".to_owned());
        Ok(observed(signal))
    }
}

/// The event a successful act answers with — the typed frame the engines
/// would have announced, delivered locally.
fn event_of(port: &mut Port, cmd: &EngineCmd) -> NativeEvent {
    match apply(port, cmd) {
        EngineEvent::Event {
            event,
            delivery: Delivery::Local,
        } => event,
        other => panic!("{cmd:?} answered {other:?}, not a local event"),
    }
}

#[test]
fn task_and_signal_acts_answer_as_the_typed_events_the_engines_would_announce() {
    let mut port = Port {
        pins: Vec::new(),
        fail: false,
    };
    assert_eq!(
        event_of(
            &mut port,
            &EngineCmd::StartTask {
                id: "auth".to_owned(),
            },
        ),
        NativeEvent::Plan(PlanFrame::TaskChanged {
            task: task("auth", TaskStatus::InProgress),
        })
    );
    assert_eq!(
        event_of(
            &mut port,
            &EngineCmd::CompleteTask {
                id: "auth".to_owned(),
            },
        ),
        NativeEvent::Plan(PlanFrame::TaskCompleted {
            task: task("auth", TaskStatus::Done),
        })
    );
    assert_eq!(
        event_of(
            &mut port,
            &EngineCmd::IgnoreSignal {
                id: "s1".to_owned(),
            },
        ),
        NativeEvent::Signal(SignalFrame::SignalDismissed {
            signal: signal("s1", SignalStatus::Dismissed),
        })
    );
    assert_eq!(
        event_of(
            &mut port,
            &EngineCmd::PromoteSignal {
                id: "s2".to_owned(),
            },
        ),
        NativeEvent::Signal(SignalFrame::SignalChanged {
            signal: signal("s2", SignalStatus::Promoted),
        })
    );
    assert!(matches!(
        event_of(
            &mut port,
            &EngineCmd::SnoozeSignal {
                id: "s3".to_owned(),
                minutes: 5,
            },
        ),
        NativeEvent::Signal(SignalFrame::SignalChanged { signal })
            if signal.snooze_until.as_deref() == Some("soon")
    ));
}

#[test]
fn a_task_refusal_names_the_act_and_the_kind() {
    use crate::cockpit::ProposalKind;

    let mut port = Port {
        pins: Vec::new(),
        fail: true,
    };
    for (kind, label) in [
        (ProposalKind::Title, "title"),
        (ProposalKind::Status, "status"),
        (ProposalKind::Reprioritize, "priority"),
    ] {
        for (accept, act) in [(true, "accept"), (false, "reject")] {
            let cmd = EngineCmd::ResolveProposal {
                task: "auth".to_owned(),
                kind,
                accept,
            };
            assert_eq!(
                apply(&mut port, &cmd),
                EngineEvent::PortFailed {
                    cmd,
                    what: format!("{act} {label} proposal on auth"),
                    error: "no open proposal".to_owned(),
                }
            );
        }
    }
}

#[test]
fn a_pin_reaches_the_port_and_answers_with_the_requested_flag() {
    let mut port = Port {
        pins: Vec::new(),
        fail: false,
    };
    let event = apply(
        &mut port,
        &EngineCmd::Pin {
            step_number: 7,
            pinned: true,
        },
    );
    assert_eq!(port.pins, vec![(7, true)]);
    assert_eq!(
        event,
        EngineEvent::Pinned {
            step_number: 7,
            pinned: true,
        }
    );
}

#[test]
fn a_checkpoint_renders_the_report_to_lines() {
    let mut port = Port {
        pins: Vec::new(),
        fail: false,
    };
    let EngineEvent::Checkpoint { lines } = apply(&mut port, &EngineCmd::Checkpoint) else {
        panic!("a checkpoint answers with its report");
    };
    assert!(lines.iter().any(|line| line.contains("open_hypotheses")));
}

#[test]
fn a_refusal_names_what_was_asked() {
    let mut port = Port {
        pins: Vec::new(),
        fail: true,
    };
    assert_eq!(
        apply(
            &mut port,
            &EngineCmd::Pin {
                step_number: 9,
                pinned: false,
            }
        ),
        EngineEvent::PortFailed {
            cmd: EngineCmd::Pin {
                step_number: 9,
                pinned: false,
            },
            what: "pin 9".to_owned(),
            error: "no such step".to_owned(),
        }
    );
    assert_eq!(
        apply(&mut port, &EngineCmd::Checkpoint),
        EngineEvent::PortFailed {
            cmd: EngineCmd::Checkpoint,
            what: "checkpoint".to_owned(),
            error: "engine gone".to_owned(),
        }
    );
}

#[test]
fn a_cleared_blocker_answers_with_the_task_changed_event_the_dock_folds() {
    let mut port = Port {
        pins: Vec::new(),
        fail: false,
    };
    assert_eq!(
        event_of(
            &mut port,
            &EngineCmd::ClearBlocker {
                task: "auth".to_owned(),
            },
        ),
        NativeEvent::Plan(PlanFrame::TaskChanged {
            task: task("auth", TaskStatus::ToDo),
        })
    );
}

#[test]
fn a_blocker_the_engine_will_not_clear_names_the_task() {
    let mut port = Port {
        pins: Vec::new(),
        fail: true,
    };
    assert_eq!(
        apply(
            &mut port,
            &EngineCmd::ClearBlocker {
                task: "auth".to_owned(),
            }
        ),
        EngineEvent::PortFailed {
            cmd: EngineCmd::ClearBlocker {
                task: "auth".to_owned(),
            },
            what: "clear the blocker on auth".to_owned(),
            error: "no waiting reason to clear".to_owned(),
        }
    );
}

#[test]
fn task_and_signal_refusals_carry_the_originating_command() {
    let mut port = Port {
        pins: Vec::new(),
        fail: true,
    };
    for (cmd, what, error) in [
        (
            EngineCmd::StartTask {
                id: "auth".to_owned(),
            },
            "start auth",
            "no such task",
        ),
        (
            EngineCmd::CompleteTask {
                id: "auth".to_owned(),
            },
            "complete auth",
            "no such task",
        ),
        (
            EngineCmd::PromoteSignal {
                id: "s1".to_owned(),
            },
            "promote s1",
            "not researched",
        ),
        (
            EngineCmd::IgnoreSignal {
                id: "s2".to_owned(),
            },
            "ignore s2",
            "no such signal",
        ),
        (
            EngineCmd::SnoozeSignal {
                id: "s3".to_owned(),
                minutes: 15,
            },
            "snooze s3",
            "no such signal",
        ),
    ] {
        assert_eq!(
            apply(&mut port, &cmd),
            EngineEvent::PortFailed {
                cmd,
                what: what.to_owned(),
                error: error.to_owned(),
            }
        );
    }
}
