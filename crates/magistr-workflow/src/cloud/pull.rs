//! Fetch cloud records without holding engine locks, then apply each record
//! through its local persistence owner. Acknowledgements count committed
//! changes only; failures preserve the count already saved and typed write cause.

use std::sync::{Arc, Mutex};

use crate::cloud::client::{CloudClient, CloudError};
use crate::decision::domain::Note;
use crate::decision::engine::core::ReasoningServer;
use crate::plan::PlanEngine;
use crate::plan::domain::{Task, TrackerLink, TrackerOptIn};
use crate::signal::SignalEngine;
use crate::signal::domain::Signal;

/// Fetch the tenant's record envelopes for one family (`GET /v1/records?family=`).
/// Pure transport — no engine access, safe to await without holding any lock.
pub async fn fetch_family(
    client: &CloudClient,
    family: &str,
) -> Result<Vec<serde_json::Value>, CloudError> {
    client.list(Some(family), None).await
}

/// Incremental fetch: only records whose `updated >=` the watermark
/// (cloud-read-amplification). Pure transport.
pub async fn fetch_family_since(
    client: &CloudClient,
    family: &str,
    since: &str,
) -> Result<Vec<serde_json::Value>, CloudError> {
    client.list(Some(family), Some(since)).await
}

/// The newest change-cursor stamp in a batch of envelopes (`updated ?? created`)
/// — the next watermark after a merge. `None` for an empty batch.
#[must_use]
pub fn max_watermark(envelopes: &[serde_json::Value]) -> Option<String> {
    envelopes
        .iter()
        .filter_map(|e| {
            e.get("updated")
                .or_else(|| e.get("created"))
                .and_then(|v| v.as_str())
        })
        .max()
        .map(str::to_owned)
}

/// Tenant membership is insufficient: only the record's project owns it.
fn record_is_local(envelope: &serde_json::Value, project_id: &str) -> bool {
    envelope
        .get("record")
        .and_then(|record| record.get("project_id"))
        .and_then(serde_json::Value::as_str)
        == Some(project_id)
}

fn invalid_record(error: impl std::fmt::Display) -> CloudError {
    CloudError::Local {
        saved: 0,
        message: error.to_string(),
        store: None,
    }
}

/// Decode the whole input before committing any local record.
fn local_records<T: serde::de::DeserializeOwned>(
    envelopes: &[serde_json::Value],
    project_id: &str,
) -> Result<Vec<(T, serde_json::Value)>, CloudError> {
    envelopes
        .iter()
        .filter(|envelope| record_is_local(envelope, project_id))
        .map(|envelope| {
            let record = envelope["record"].clone();
            let value = serde_json::from_value(record.clone()).map_err(invalid_record)?;
            Ok((value, record))
        })
        .collect()
}

#[derive(serde::Deserialize, Default)]
struct TrackerSidecar {
    #[serde(default)]
    links: Vec<TrackerLink>,
    #[serde(default)]
    opt_ins: Vec<TrackerOptIn>,
}

/// Commit each task with its tracker state in one owner transaction.
pub fn apply_plan_records(
    engine: &mut PlanEngine,
    envelopes: &[serde_json::Value],
) -> Result<usize, CloudError> {
    let records = local_records::<Task>(envelopes, engine.project_id())?
        .into_iter()
        .map(|(task, record)| {
            let sidecar = record
                .get(crate::cloud::build::TRACKER_SIDECAR)
                .map(|value| serde_json::from_value::<TrackerSidecar>(value.clone()))
                .transpose()
                .map_err(invalid_record)?
                .unwrap_or_default();
            Ok((task, sidecar))
        })
        .collect::<Result<Vec<_>, CloudError>>()?;
    let mut saved = 0;
    for (task, sidecar) in records {
        let observed = engine
            .upsert_task_with_tracker(task, sidecar.links, sidecar.opt_ins)
            .map_err(|error| {
                let message = error.to_string();
                let store = match error {
                    crate::plan::engine::PlanMutationError::Store(error) => Some(error),
                    crate::plan::engine::PlanMutationError::Rejected(_) => None,
                };
                CloudError::Local {
                    saved,
                    message,
                    store,
                }
            })?;
        saved += usize::from(observed.value);
    }
    Ok(saved)
}

/// Commit signal changes, preserving a failure after any earlier saved records.
pub fn apply_signal_records(
    engine: &mut SignalEngine,
    envelopes: &[serde_json::Value],
) -> Result<usize, CloudError> {
    let records = local_records::<Signal>(envelopes, engine.project_id())?;
    let mut saved = 0;
    for (signal, _) in records {
        let observed = engine.upsert_signal(signal).map_err(|error| {
            let message = error.to_string();
            let store = match error {
                crate::signal::engine::SignalMutationError::Store(error) => Some(error),
                crate::signal::engine::SignalMutationError::Rejected(_) => None,
            };
            CloudError::Local {
                saved,
                message,
                store,
            }
        })?;
        saved += usize::from(observed.value);
    }
    Ok(saved)
}

/// Adopt the complete reasoning batch by immutable record identity.
pub fn apply_decision_records(
    engine: &mut ReasoningServer,
    envelopes: &[serde_json::Value],
) -> Result<usize, CloudError> {
    let notes = local_records::<Note>(envelopes, engine.project_id())?
        .into_iter()
        .map(|(step, _)| step)
        .collect();
    engine
        .adopt_notes(notes)
        .map(|observed| observed.value)
        .map_err(|error| CloudError::Local {
            saved: 0,
            message: error.error,
            store: error.store,
        })
}

pub async fn reconcile_decision(
    client: &CloudClient,
    engine: &mut ReasoningServer,
) -> Result<usize, CloudError> {
    let envelopes = fetch_family(client, "decision").await?;
    apply_decision_records(engine, &envelopes)
}

/// Attempt each family independently and retain every family's result.
pub async fn reconcile_all(
    client: &CloudClient,
    decision: &Arc<Mutex<ReasoningServer>>,
    plan: &Arc<Mutex<PlanEngine>>,
    signal: &Arc<Mutex<SignalEngine>>,
) -> (
    Result<usize, CloudError>,
    Result<usize, CloudError>,
    Result<usize, CloudError>,
) {
    let decision_result = match fetch_family(client, "decision").await {
        Ok(envelopes) => decision
            .lock()
            .map_err(invalid_record)
            .and_then(|mut engine| apply_decision_records(&mut engine, &envelopes)),
        Err(error) => Err(error),
    };
    let plan_result = match fetch_family(client, "plan").await {
        Ok(envelopes) => plan
            .lock()
            .map_err(invalid_record)
            .and_then(|mut engine| apply_plan_records(&mut engine, &envelopes)),
        Err(error) => Err(error),
    };
    let signal_result = match fetch_family(client, "signal").await {
        Ok(envelopes) => signal
            .lock()
            .map_err(invalid_record)
            .and_then(|mut engine| apply_signal_records(&mut engine, &envelopes)),
        Err(error) => Err(error),
    };
    (decision_result, plan_result, signal_result)
}

pub async fn reconcile_plan(
    client: &CloudClient,
    engine: &mut PlanEngine,
) -> Result<usize, CloudError> {
    let envelopes = fetch_family(client, "plan").await?;
    apply_plan_records(engine, &envelopes)
}

pub async fn reconcile_signals(
    client: &CloudClient,
    engine: &mut SignalEngine,
) -> Result<usize, CloudError> {
    let envelopes = fetch_family(client, "signal").await?;
    apply_signal_records(engine, &envelopes)
}

#[cfg(test)]
#[path = "pull.tests.rs"]
mod tests;
