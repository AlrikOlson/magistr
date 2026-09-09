use serde_json::json;

use super::*;

struct Port {
    pins: Vec<(u32, bool)>,
    fail: bool,
}

impl EnginePort for Port {
    fn pin(&mut self, step_number: u32, pinned: bool) -> Result<(), String> {
        if self.fail {
            return Err("no such step".to_owned());
        }
        self.pins.push((step_number, pinned));
        Ok(())
    }

    fn checkpoint(&mut self) -> Result<serde_json::Value, String> {
        if self.fail {
            return Err("engine gone".to_owned());
        }
        Ok(json!({"open_hypotheses": []}))
    }

    fn start_chunk(&mut self, id: &str) -> Result<serde_json::Value, String> {
        if self.fail {
            return Err("no such chunk".to_owned());
        }
        Ok(json!({"id": id, "status": "in_progress"}))
    }

    fn complete_chunk(&mut self, id: &str) -> Result<serde_json::Value, String> {
        if self.fail {
            return Err("no such chunk".to_owned());
        }
        Ok(json!({"id": id, "status": "done"}))
    }

    fn resolve_proposal(
        &mut self,
        id: &str,
        _kind: crate::cockpit::ProposalKind,
        _accept: bool,
    ) -> Result<serde_json::Value, String> {
        if self.fail {
            return Err("no open proposal".to_owned());
        }
        Ok(json!({"id": id, "status": "pending"}))
    }

    fn clear_blocker(&mut self, id: &str) -> Result<serde_json::Value, String> {
        if self.fail {
            return Err("no blocker to clear".to_owned());
        }
        Ok(json!({"id": id, "status": "pending"}))
    }

    fn promote_signal(&mut self, id: &str) -> Result<serde_json::Value, String> {
        if self.fail {
            return Err("not researched".to_owned());
        }
        Ok(json!({"id": id, "status": "promoted"}))
    }

    fn ignore_signal(&mut self, id: &str) -> Result<serde_json::Value, String> {
        if self.fail {
            return Err("no such signal".to_owned());
        }
        Ok(json!({"id": id, "status": "dismissed"}))
    }

    fn snooze_signal(&mut self, id: &str, _minutes: u32) -> Result<serde_json::Value, String> {
        if self.fail {
            return Err("no such signal".to_owned());
        }
        Ok(json!({"id": id, "snooze_until": "soon"}))
    }
}

#[test]
fn chunk_and_signal_acts_answer_as_synthesized_frames() {
    let mut port = Port {
        pins: Vec::new(),
        fail: false,
    };
    let EngineEvent::Frame(frame) = apply(
        &mut port,
        &EngineCmd::StartChunk {
            id: "auth".to_owned(),
        },
    ) else {
        panic!("a start answers with a frame");
    };
    assert_eq!(
        (frame.family.as_str(), frame.kind.as_str()),
        ("roadmap", "chunk_changed")
    );
    assert_eq!(frame.payload["chunk"]["status"], "in_progress");

    let EngineEvent::Frame(frame) = apply(
        &mut port,
        &EngineCmd::CompleteChunk {
            id: "auth".to_owned(),
        },
    ) else {
        panic!("a completion answers with a frame");
    };
    assert_eq!(frame.kind, "chunk_completed");

    let EngineEvent::Frame(frame) = apply(
        &mut port,
        &EngineCmd::IgnoreSignal {
            id: "s1".to_owned(),
        },
    ) else {
        panic!("an ignore answers with a frame");
    };
    assert_eq!(
        (frame.family.as_str(), frame.kind.as_str()),
        ("signal", "signal_dismissed")
    );
    assert_eq!(frame.payload["signal"]["id"], "s1");
}

#[test]
fn a_chunk_refusal_names_the_act_and_the_kind() {
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
                chunk: "auth".to_owned(),
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
fn a_cleared_blocker_answers_with_the_chunk_changed_frame_the_dock_folds() {
    let mut port = Port {
        pins: Vec::new(),
        fail: false,
    };
    let EngineEvent::Frame(frame) = apply(
        &mut port,
        &EngineCmd::ClearBlocker {
            chunk: "auth".to_owned(),
        },
    ) else {
        panic!("a clear answers with a frame");
    };
    assert_eq!(
        (frame.family.as_str(), frame.kind.as_str()),
        ("roadmap", "chunk_changed")
    );
    assert_eq!(frame.payload["chunk"]["id"], "auth");
}

#[test]
fn a_blocker_the_engine_will_not_clear_names_the_chunk() {
    let mut port = Port {
        pins: Vec::new(),
        fail: true,
    };
    assert_eq!(
        apply(
            &mut port,
            &EngineCmd::ClearBlocker {
                chunk: "auth".to_owned(),
            }
        ),
        EngineEvent::PortFailed {
            cmd: EngineCmd::ClearBlocker {
                chunk: "auth".to_owned(),
            },
            what: "clear the blocker on auth".to_owned(),
            error: "no blocker to clear".to_owned(),
        }
    );
}

#[test]
fn chunk_and_signal_refusals_carry_the_originating_command() {
    let mut port = Port {
        pins: Vec::new(),
        fail: true,
    };
    for (cmd, what, error) in [
        (
            EngineCmd::StartChunk {
                id: "auth".to_owned(),
            },
            "start auth",
            "no such chunk",
        ),
        (
            EngineCmd::CompleteChunk {
                id: "auth".to_owned(),
            },
            "complete auth",
            "no such chunk",
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
