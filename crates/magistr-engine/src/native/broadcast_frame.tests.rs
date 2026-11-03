use super::*;
use crate::native::{CheckBytes, DecisionFrame, MagistrFrame, PlanFrame, RequestFrame, RunFrame};
use crate::native::{EventCursor, NativeEvent, OwnerGeneration};
use crate::workflow::WorkflowPhase;
#[cfg(unix)]
use magistr_workflow::infra::Broadcaster;
use magistr_workflow::infra::Family;
use magistr_workflow::plan::domain::TaskStatus;
use magistr_workflow::request::domain::RequestKind;

#[test]
fn parses_family_and_type() {
    let line = r#"{"family":"plan","type":"task_changed","task":{"id":"a"}}"#;
    let f = NativeFrame::parse_line(line).expect("frame");
    assert_eq!(f.family, "plan");
    assert_eq!(f.kind, "task_changed");
    assert_eq!(f.payload["task"]["id"], "a");
}

#[test]
fn missing_type_is_an_empty_kind() {
    let f = NativeFrame::parse_line("{\"family\":\"run\"}\n").expect("frame");
    assert_eq!(f.family, "run");
    assert_eq!(f.kind, "");
}

#[test]
fn junk_lines_are_dropped() {
    assert!(NativeFrame::parse_line("").is_none());
    assert!(NativeFrame::parse_line("not json").is_none());
    assert!(NativeFrame::parse_line("[1,2]").is_none());
    assert!(NativeFrame::parse_line("{\"type\":\"x\"}").is_none());
    assert!(NativeFrame::parse_line("{\"family\":3}").is_none());
}

fn task(id: &str) -> magistr_workflow::plan::domain::Task {
    magistr_workflow::plan::domain::Task {
        id: id.to_owned(),
        title: format!("Task {id}"),
        name: id.to_owned(),
        status: TaskStatus::InProgress,
        priority: 100,
        description: String::new(),
        content: None,
        notes: String::new(),
        feature: None,
        acceptance: vec![],
        deps: vec![],
        tier: None,
        refs: vec![],
        shared: false,
        reprioritize: None,
        status_proposal: None,
        title_proposal: None,
        retired_reason: None,
        waiting_on: None,
        project_id: None,
        created_at: "2026-09-12T00:00:00Z".to_owned(),
        updated_at: "2026-09-12T00:00:00Z".to_owned(),
    }
}

fn request(id: &str) -> magistr_workflow::request::domain::Request {
    magistr_workflow::request::domain::Request {
        id: id.to_owned(),
        kind: RequestKind::Suggestion,
        from: "alice".to_owned(),
        body: "faster startup".to_owned(),
        content: None,
        created: "2026-09-12T00:00:00Z".to_owned(),
        status: magistr_workflow::request::domain::RequestStatus::New,
        enrichment: vec![],
        refs: vec![],
        raised_at: None,
        snooze_until: None,
        project_id: None,
    }
}

fn note(n: u32) -> magistr_workflow::decision::domain::Note {
    serde_json::from_value(serde_json::json!({
        "note": n,
        "estimated_total": 3,
        "purpose": "open",
        "context": "the edge",
        "thought": "decode once",
        "outcome": "typed",
        "next_action": "fold",
        "rationale": "strings stop here",
    }))
    .expect("note")
}

/// The line a publisher writes is what the edge decodes: every family's
/// typed frame survives the round trip, tags and all, with the cursor the
/// publisher stamped.
#[test]
fn every_family_round_trips_through_the_wire_line() {
    let cases: Vec<(Family, serde_json::Value, NativeEvent)> = vec![
        (
            Family::Decision,
            serde_json::to_value(DecisionFrame::NoteAppended {
                log_id: None,
                note: Box::new(note(4)),
            })
            .unwrap(),
            NativeEvent::Decision(DecisionFrame::NoteAppended {
                log_id: None,
                note: Box::new(note(4)),
            }),
        ),
        (
            Family::Run,
            serde_json::to_value(RunFrame::StepStarted {
                step_id: "t1".to_owned(),
            })
            .unwrap(),
            NativeEvent::Run(RunFrame::StepStarted {
                step_id: "t1".to_owned(),
            }),
        ),
        (
            Family::Plan,
            serde_json::to_value(PlanFrame::TaskChanged { task: task("a") }).unwrap(),
            NativeEvent::Plan(PlanFrame::TaskChanged { task: task("a") }),
        ),
        (
            Family::Request,
            serde_json::to_value(RequestFrame::RequestCaptured {
                request: request("s1"),
            })
            .unwrap(),
            NativeEvent::Request(RequestFrame::RequestCaptured {
                request: request("s1"),
            }),
        ),
        (
            Family::Magistr,
            serde_json::to_value(MagistrFrame::CheckBytes(CheckBytes {
                check: "test".to_owned(),
                bytes: vec![0, 255, b'\n'],
            }))
            .unwrap(),
            NativeEvent::Magistr(MagistrFrame::CheckBytes(CheckBytes {
                check: "test".to_owned(),
                bytes: vec![0, 255, b'\n'],
            })),
        ),
        (
            Family::Magistr,
            serde_json::to_value(MagistrFrame::PhaseSet {
                phase: WorkflowPhase::Verifying,
            })
            .unwrap(),
            NativeEvent::Magistr(MagistrFrame::PhaseSet {
                phase: WorkflowPhase::Verifying,
            }),
        ),
    ];
    for (family, mut payload, expected) in cases {
        // What `Broadcaster::emit` flattens onto the payload.
        let obj = payload.as_object_mut().expect("object");
        obj.insert("family".to_owned(), serde_json::json!(family.as_str()));
        obj.insert("seq".to_owned(), serde_json::json!(12));
        obj.insert("epoch".to_owned(), serde_json::json!(3));
        let line = serde_json::to_string(&payload).unwrap();
        let (event, cursor) = NativeFrame::parse_line(&line)
            .expect("a frame")
            .decode()
            .unwrap_or_else(|| panic!("{family:?} decodes: {line}"));
        assert_eq!(event, expected);
        assert_eq!(event.family(), family);
        assert_eq!(
            cursor,
            Some(EventCursor {
                epoch: OwnerGeneration::admitted(3).unwrap(),
                sequence: 12,
            })
        );
    }
}

/// The very bytes a live broadcaster writes decode, sequence and epoch
/// included — the socket is the edge this decoder is for.
#[cfg(unix)]
#[tokio::test]
async fn a_live_broadcaster_line_decodes_with_its_stamps() {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::net::UnixStream;

    let tmp = tempfile::tempdir().expect("tempdir");
    let sock = tmp.path().join("b.sock");
    let publisher = Broadcaster::spawn(sock.clone())
        .expect("bind")
        .with_epoch(9);
    let stream = UnixStream::connect(&sock).await.expect("connect");
    assert!(
        publisher
            .wait_for_subscribers(1, std::time::Duration::from_secs(5))
            .await
    );
    publisher
        .emit(
            Family::Request,
            &RequestFrame::RequestCaptured {
                request: request("s9"),
            },
        )
        .expect("emit");
    let mut lines = BufReader::new(stream).lines();
    let line = lines.next_line().await.expect("read").expect("a line");
    let (event, cursor) = NativeFrame::parse_line(&line)
        .expect("frame")
        .decode()
        .expect("decodes");
    assert_eq!(
        event,
        NativeEvent::Request(RequestFrame::RequestCaptured {
            request: request("s9")
        })
    );
    assert_eq!(
        cursor,
        Some(EventCursor {
            epoch: OwnerGeneration::admitted(9).unwrap(),
            sequence: 1,
        })
    );
}

#[test]
fn a_publisher_without_stamps_decodes_without_a_cursor() {
    let line = r#"{"family":"run","type":"step_started","step_id":"t1"}"#;
    let (event, cursor) = NativeFrame::parse_line(line).unwrap().decode().unwrap();
    assert_eq!(
        event,
        NativeEvent::Run(RunFrame::StepStarted {
            step_id: "t1".to_owned()
        })
    );
    assert_eq!(cursor, None);
    // Half a stamp is no stamp, and a zero epoch is one no owner minted.
    for partial in [
        r#"{"family":"run","type":"step_started","step_id":"t1","seq":4}"#,
        r#"{"family":"run","type":"step_started","step_id":"t1","epoch":2}"#,
        r#"{"family":"run","type":"step_started","step_id":"t1","epoch":0,"seq":4}"#,
    ] {
        let (_, cursor) = NativeFrame::parse_line(partial).unwrap().decode().unwrap();
        assert_eq!(cursor, None, "{partial}");
    }
}

#[test]
fn unknown_families_kinds_and_malformed_payloads_are_dropped_not_half_read() {
    for line in [
        // A family this build does not know.
        r#"{"family":"workspace","type":"journal","seq":1}"#,
        // A kind the run family does not have (what check output used to
        // ride as).
        r#"{"family":"run","type":"check_bytes","check":"check","bytes":[1]}"#,
        // The right kind with the wrong shape.
        r#"{"family":"magistr","type":"check_bytes","check":"check","bytes":"not an array"}"#,
        r#"{"family":"magistr","type":"phase_set","phase":"nowhere"}"#,
        r#"{"family":"plan","type":"task_changed","task":{"id":"a"}}"#,
        // No kind at all.
        r#"{"family":"run"}"#,
    ] {
        assert!(
            NativeFrame::parse_line(line).unwrap().decode().is_none(),
            "{line}"
        );
    }
}
