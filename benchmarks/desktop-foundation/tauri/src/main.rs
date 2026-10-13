//! Rust fixture delivery for the bounded native-window comparison.
use magistr_foundation_workload::{Event, Fixture, StreamCancellation, StreamRun};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::OpenOptions, io::Write, path::PathBuf, sync::Mutex, thread::JoinHandle, time::Duration,
};
use tauri::{State, ipc::Channel};

struct Output(PathBuf);
#[derive(Default)]
struct ActiveRun(Mutex<Option<(StreamCancellation, JoinHandle<()>)>>);

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Record {
    Ready,
    Snapshot,
    Metrics,
    Journey,
}
#[derive(Serialize)]
struct Batch {
    events: Vec<Event>,
    elapsed_us: u64,
    finished: bool,
    dropped: u64,
}

#[tauri::command]
fn fixture() -> Fixture {
    Fixture::new()
}

#[tauri::command]
fn record(kind: Record, value: Value, output: State<'_, Output>) -> Result<(), String> {
    let name = match kind {
        Record::Ready => "ready",
        Record::Snapshot => "snapshot",
        Record::Metrics => "metrics",
        Record::Journey => "journey",
    };
    let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(output.0.join(format!("{name}.json")))
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())
}

#[tauri::command]
fn stream(channel: Channel<Batch>, active: State<'_, ActiveRun>) -> Result<(), String> {
    let mut slot = active.0.lock().map_err(|e| e.to_string())?;
    if let Some((_, thread)) = slot.as_ref()
        && !thread.is_finished()
    {
        return Err("Work is already running".into());
    }
    if let Some((_, previous)) = slot.take() {
        previous
            .join()
            .map_err(|_| "The previous workload failed")?;
    }
    let run = StreamRun::start(1_250, Duration::from_millis(8));
    let cancellation = run.cancellation();
    let thread = std::thread::spawn(move || {
        loop {
            let first = run.receiver.recv_timeout(Duration::from_millis(8));
            let finished = matches!(first, Err(std::sync::mpsc::RecvTimeoutError::Disconnected));
            let mut events: Vec<_> = first.into_iter().collect();
            events.extend(run.receiver.try_iter().take(255));
            let batch = Batch {
                events,
                elapsed_us: u64::try_from(run.started.elapsed().as_micros()).unwrap_or(u64::MAX),
                finished,
                dropped: run.dropped(),
            };
            if channel.send(batch).is_err() || finished {
                break;
            }
        }
    });
    *slot = Some((cancellation, thread));
    Ok(())
}

#[tauri::command]
fn stop(active: State<'_, ActiveRun>) -> Result<(), String> {
    let run = active.0.lock().map_err(|e| e.to_string())?.take();
    if let Some((cancellation, thread)) = run {
        cancellation.stop();
        thread
            .join()
            .map_err(|_| "The workload failed while stopping")?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("Pass a disposable output directory for the foundation fixture")?;
    std::fs::create_dir_all(&output)?;
    tauri::Builder::default()
        .manage(Output(output))
        .manage(ActiveRun::default())
        .invoke_handler(tauri::generate_handler![fixture, record, stream, stop])
        .run(tauri::generate_context!())?;
    Ok(())
}
