use super::*;

#[test]
fn a_chunk_round_trips_through_its_frame() {
    let out = GateOutput {
        gate: "check".to_owned(),
        bytes: b"\x1b[32mCompiling\x1b[0m foo\r\n".to_vec(),
    };
    let frame = out.frame();
    assert_eq!(frame.family, "ship");
    assert_eq!(frame.kind, "gate_output");
    assert_eq!(GateOutput::from_frame(&frame), Some(out));
}

#[test]
fn the_frame_survives_the_socket_line() {
    // What mcp-serve writes is what the engine feed parses back.
    let out = GateOutput {
        gate: "test".to_owned(),
        bytes: vec![0, 255, b'\n'],
    };
    let line = serde_json::to_string(&out.frame().payload).expect("json");
    let parsed = NativeFrame::parse_line(&line).expect("a frame");
    assert_eq!(GateOutput::from_frame(&parsed), Some(out));
}

#[test]
fn other_frames_are_not_gate_output() {
    let check = NativeFrame {
        family: "ship".to_owned(),
        kind: "check_recorded".to_owned(),
        payload: serde_json::json!({"gate": "check", "bytes": [1]}),
    };
    assert_eq!(GateOutput::from_frame(&check), None);
    let wrong_family = NativeFrame {
        family: "roadmap".to_owned(),
        kind: "gate_output".to_owned(),
        payload: serde_json::json!({"gate": "check", "bytes": [1]}),
    };
    assert_eq!(GateOutput::from_frame(&wrong_family), None);
    let malformed = NativeFrame {
        family: "ship".to_owned(),
        kind: "gate_output".to_owned(),
        payload: serde_json::json!({"gate": "check", "bytes": "not an array"}),
    };
    assert_eq!(GateOutput::from_frame(&malformed), None);
}
