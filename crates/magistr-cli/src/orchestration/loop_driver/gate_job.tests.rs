use std::time::Duration;

use magistr_cockpit_runtime::Jobs;
use tokio::sync::mpsc::unbounded_channel;

use super::*;

async fn next(posted: &mut tokio::sync::mpsc::UnboundedReceiver<Msg>) -> Msg {
    tokio::time::timeout(Duration::from_secs(10), posted.recv())
        .await
        .expect("a message in time")
        .expect("the inbox is open")
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_gates_bytes_reach_the_inbox_as_frames_before_its_outcome() {
    let (inbox, mut posted) = Inbox::channel();
    let jobs = Jobs::new(inbox.clone());
    let (report, mut outcomes) = unbounded_channel();
    let id = jobs.spawn_job(gate_job(
        "check".to_owned(),
        "printf '\\033[32mok\\033[0m\\n'".to_owned(),
        std::env::current_dir().expect("a working directory"),
        report,
        inbox,
        CancellationToken::new(),
    ));

    let outcome = tokio::time::timeout(Duration::from_secs(10), outcomes.recv())
        .await
        .expect("an outcome in time")
        .expect("the report channel is open");
    assert_eq!(outcome.name, "check");
    assert!(outcome.passed());
    assert_eq!(outcome.output, "ok\n", "the record keeps the plain text");

    // On the inbox: the gate's raw bytes as frames keyed by the gate,
    // every one of them before the job's own result.
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        match next(&mut posted).await {
            Msg::Engine(EngineEvent::Event {
                event:
                    magistr_engine::NativeEvent::Magistr(magistr_engine::MagistrFrame::GateOutput(
                        output,
                    )),
                delivery: magistr_engine::Delivery::Local,
            }) => {
                assert_eq!(output.gate, "check");
                bytes.extend(output.bytes);
            }
            Msg::Job { id: done, output } => {
                assert_eq!(done, id);
                assert!(matches!(output, JobOutput::Text(_)), "{output:?}");
                break;
            }
            other => panic!("not a gate's message: {other:?}"),
        }
    }
    assert!(
        bytes.windows(5).any(|w| w == b"\x1b[32m"),
        "the pane gets the colour: {bytes:?}"
    );
    assert!(bytes.ends_with(b"ok\x1b[0m\r\n"), "{bytes:?}");
}
