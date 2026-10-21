//! Measure actual cold/warm source observation, including tree selection.
use magistr_workspace::snapshot::{SelectionPolicy, SourceSnapshot};
use std::path::PathBuf;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    let policy = SelectionPolicy::default().with_sources(args.collect());
    let mut previous = None;
    for run in 1..=3 {
        let started = Instant::now();
        let snapshot = SourceSnapshot::capture(&root, policy.clone())?;
        let elapsed_ms = started.elapsed().as_millis();
        let digest = snapshot.manifest.digest();
        if let Some(previous) = &previous {
            assert_eq!(previous, &digest, "source changed");
        }
        previous = Some(digest);
        println!(
            "{}",
            serde_json::json!({
                "run": run, "elapsed_ms": elapsed_ms, "files": snapshot.manifest.entries.len(),
                "bytes": snapshot.manifest.entries.values().map(|entry| entry.size).sum::<u64>()
            })
        );
    }
    Ok(())
}
