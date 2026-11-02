use magistr_workflow_contracts::{Delivery, NativeEvent, PlanFrame, RequestFrame};
use magistr_workflow_contracts::{Request, RequestStatus, Task, TaskStatus};
use serde_json::json;

use super::*;
use magistr_workflow_contracts::{ReadSource, request::RequestPromotion};

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

fn request(id: &str, status: RequestStatus) -> Request {
    let mut request = fixtures::request(id);
    request.status = status;
    request
}

impl EnginePort for Port {
    fn pin(
        &mut self,
        note_number: u32,
        pinned: bool,
    ) -> Result<Observed<bool>, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("no such note"));
        }
        self.pins.push((note_number, pinned));
        Ok(observed(false))
    }

    fn review(&mut self) -> Result<serde_json::Value, String> {
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

    fn promote_request(&mut self, id: &str) -> Result<RequestPromotion, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("not researched"));
        }
        Ok(RequestPromotion {
            request_id: id.into(),
            task_id: "new".into(),
            created: true,
            request: observed(request(id, RequestStatus::Promoted)),
            task: observed(task("new", TaskStatus::Idea)),
        })
    }

    fn dismiss_request(&mut self, id: &str) -> Result<Observed<Request>, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("no such request"));
        }
        Ok(observed(request(id, RequestStatus::Dismissed)))
    }

    fn snooze_request(
        &mut self,
        id: &str,
        _minutes: u32,
    ) -> Result<Observed<Request>, WorkflowMutationError> {
        if self.fail {
            return Err(WorkflowMutationError::rejected("no such request"));
        }
        let mut request = request(id, RequestStatus::Researched);
        request.snooze_until = Some("soon".to_owned());
        Ok(observed(request))
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
fn task_and_request_acts_answer_as_the_typed_events_the_engines_would_announce() {
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
            &EngineCmd::DismissRequest {
                id: "s1".to_owned(),
            },
        ),
        NativeEvent::Request(RequestFrame::RequestDismissed {
            request: request("s1", RequestStatus::Dismissed),
        })
    );
    assert_eq!(
        event_of(
            &mut port,
            &EngineCmd::PromoteRequest {
                id: "s2".to_owned(),
            },
        ),
        NativeEvent::Request(RequestFrame::RequestChanged {
            request: request("s2", RequestStatus::Promoted),
        })
    );
    assert!(matches!(
        event_of(
            &mut port,
            &EngineCmd::SnoozeRequest {
                id: "s3".to_owned(),
                minutes: 5,
            },
        ),
        NativeEvent::Request(RequestFrame::RequestChanged { request })
            if request.snooze_until.as_deref() == Some("soon")
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
            note: 7,
            pinned: true,
        },
    );
    assert_eq!(port.pins, vec![(7, true)]);
    assert_eq!(
        event,
        EngineEvent::Pinned {
            note: 7,
            pinned: true,
        }
    );
}

#[test]
fn a_review_renders_the_report_to_lines() {
    let mut port = Port {
        pins: Vec::new(),
        fail: false,
    };
    let EngineEvent::Review { lines } = apply(&mut port, &EngineCmd::Review) else {
        panic!("a review answers with its report");
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
                note: 9,
                pinned: false,
            }
        ),
        EngineEvent::PortFailed {
            cmd: EngineCmd::Pin {
                note: 9,
                pinned: false,
            },
            what: "pin 9".to_owned(),
            error: "no such note".to_owned(),
        }
    );
    assert_eq!(
        apply(&mut port, &EngineCmd::Review),
        EngineEvent::PortFailed {
            cmd: EngineCmd::Review,
            what: "review".to_owned(),
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
fn task_and_request_refusals_carry_the_originating_command() {
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
            EngineCmd::PromoteRequest {
                id: "s1".to_owned(),
            },
            "promote s1",
            "not researched",
        ),
        (
            EngineCmd::DismissRequest {
                id: "s2".to_owned(),
            },
            "ignore s2",
            "no such request",
        ),
        (
            EngineCmd::SnoozeRequest {
                id: "s3".to_owned(),
                minutes: 15,
            },
            "snooze s3",
            "no such request",
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
