use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
fn specs() -> Vec<GateSpec> {
    (0..4)
        .map(|i| GateSpec {
            name: i.to_string(),
            command: "check".into(),
            required: true,
        })
        .collect()
}
#[tokio::test]
async fn bounded_batches_overlap_and_keep_deterministic_order() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = active.clone();
    let maximum = Arc::new(AtomicUsize::new(0));
    let observed = maximum.clone();
    let results = run_batch(specs(), 2, false, move |spec| {
        let active = active.clone();
        let maximum = maximum.clone();
        async move {
            let now = active.fetch_add(1, Ordering::SeqCst) + 1;
            maximum.fetch_max(now, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            active.fetch_sub(1, Ordering::SeqCst);
            json!({"gate":spec.name,"required":true,"passed":true})
        }
    })
    .await;
    assert_eq!(observed.load(Ordering::SeqCst), 2);
    assert_eq!(peak.load(Ordering::SeqCst), 0);
    assert_eq!(
        results
            .iter()
            .map(|row| row["gate"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["0", "1", "2", "3"]
    );
}
#[tokio::test]
async fn iteration_stops_scheduling_and_diagnostic_collects_every_result() {
    for diagnostic in [false, true] {
        let results = run_batch(specs(), 1, diagnostic, |spec| async move {
            json!({"gate":spec.name,"required":true,"passed":false,"verdict":"failed"})
        })
        .await;
        assert_eq!(
            results
                .iter()
                .filter(|row| row["verdict"] == "failed")
                .count(),
            if diagnostic { 4 } else { 1 }
        );
        assert_eq!(
            results[3]["verdict"],
            if diagnostic { "failed" } else { "not run" }
        );
    }
}
