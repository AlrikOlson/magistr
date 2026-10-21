//! Bounded batches with a fast iteration mode and explicit diagnostic collection.
use super::gate::GateSpec;
use serde_json::{Value, json};
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    task::Poll,
};

/// Keep independent gates in flight, stop scheduling after a required failure,
/// and preserve deterministic result order. The process scheduler owns conflicts.
pub(super) async fn run_batch<F, Fut>(
    specs: Vec<GateSpec>,
    limit: usize,
    diagnostic: bool,
    mut run: F,
) -> Vec<Value>
where
    F: FnMut(GateSpec) -> Fut,
    Fut: Future<Output = Value>,
{
    let mut pending: Vec<(usize, Pin<Box<Fut>>)> = Vec::new();
    let mut next = 0;
    let mut failed = false;
    let mut results = vec![Value::Null; specs.len()];
    while next < specs.len() || !pending.is_empty() {
        while next < specs.len() && pending.len() < limit.clamp(1, 16) && (!failed || diagnostic) {
            pending.push((next, Box::pin(run(specs[next].clone()))));
            next += 1;
        }
        if pending.is_empty() {
            break;
        }
        let (position, value) = poll_fn(|context| {
            for (position, (_, future)) in pending.iter_mut().enumerate() {
                if let Poll::Ready(value) = future.as_mut().poll(context) {
                    return Poll::Ready((position, value));
                }
            }
            Poll::Pending
        })
        .await;
        let (index, _) = pending.remove(position);
        failed |= value["required"].as_bool().unwrap_or(true) && value["passed"] != true;
        results[index] = value;
    }
    for (index, spec) in specs.iter().enumerate().skip(next) {
        results[index] = json!({"gate":spec.name,"command":spec.command,"required":spec.required,
            "passed":false,"verdict":"not run","recorded":false,"note":"An earlier required check failed. Diagnose it before running the remaining checks."});
    }
    results
}

#[cfg(test)]
#[path = "gate_batch.tests.rs"]
mod tests;
