//! Fetch cloud records without holding engine locks, then apply each record
//! through its local persistence owner. Acknowledgements count committed
//! changes only; failures preserve the count already saved and typed write cause.

use std::sync::{Arc, Mutex};

use crate::cloud::client::{CloudClient, CloudError};
use crate::roadmap::RoadmapEngine;
use crate::roadmap::domain::{Chunk, TrackerLink, TrackerOptIn};
use crate::signal::SignalEngine;
use crate::signal::domain::Signal;
use crate::think::domain::ThinkStep;
use crate::think::engine::core::ReasoningServer;

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

/// Commit each chunk with its tracker state in one owner transaction.
pub fn apply_roadmap_records(
    engine: &mut RoadmapEngine,
    envelopes: &[serde_json::Value],
) -> Result<usize, CloudError> {
    let records = local_records::<Chunk>(envelopes, engine.project_id())?
        .into_iter()
        .map(|(chunk, record)| {
            let sidecar = record
                .get(crate::cloud::build::TRACKER_SIDECAR)
                .map(|value| serde_json::from_value::<TrackerSidecar>(value.clone()))
                .transpose()
                .map_err(invalid_record)?
                .unwrap_or_default();
            Ok((chunk, sidecar))
        })
        .collect::<Result<Vec<_>, CloudError>>()?;
    let mut saved = 0;
    for (chunk, sidecar) in records {
        let observed = engine
            .upsert_chunk_with_tracker(chunk, sidecar.links, sidecar.opt_ins)
            .map_err(|error| {
                let message = error.to_string();
                let store = match error {
                    crate::roadmap::engine::RoadmapMutationError::Store(error) => Some(error),
                    crate::roadmap::engine::RoadmapMutationError::Rejected(_) => None,
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
pub fn apply_think_records(
    engine: &mut ReasoningServer,
    envelopes: &[serde_json::Value],
) -> Result<usize, CloudError> {
    let steps = local_records::<ThinkStep>(envelopes, engine.project_id())?
        .into_iter()
        .map(|(step, _)| step)
        .collect();
    engine
        .adopt_steps(steps)
        .map(|observed| observed.value)
        .map_err(|error| CloudError::Local {
            saved: 0,
            message: error.error,
            store: error.store,
        })
}

pub async fn reconcile_think(
    client: &CloudClient,
    engine: &mut ReasoningServer,
) -> Result<usize, CloudError> {
    let envelopes = fetch_family(client, "think").await?;
    apply_think_records(engine, &envelopes)
}

/// Attempt each family independently and retain every family's result.
pub async fn reconcile_all(
    client: &CloudClient,
    think: &Arc<Mutex<ReasoningServer>>,
    roadmap: &Arc<Mutex<RoadmapEngine>>,
    signal: &Arc<Mutex<SignalEngine>>,
) -> (
    Result<usize, CloudError>,
    Result<usize, CloudError>,
    Result<usize, CloudError>,
) {
    let think_result = match fetch_family(client, "think").await {
        Ok(envelopes) => think
            .lock()
            .map_err(invalid_record)
            .and_then(|mut engine| apply_think_records(&mut engine, &envelopes)),
        Err(error) => Err(error),
    };
    let roadmap_result = match fetch_family(client, "roadmap").await {
        Ok(envelopes) => roadmap
            .lock()
            .map_err(invalid_record)
            .and_then(|mut engine| apply_roadmap_records(&mut engine, &envelopes)),
        Err(error) => Err(error),
    };
    let signal_result = match fetch_family(client, "signal").await {
        Ok(envelopes) => signal
            .lock()
            .map_err(invalid_record)
            .and_then(|mut engine| apply_signal_records(&mut engine, &envelopes)),
        Err(error) => Err(error),
    };
    (think_result, roadmap_result, signal_result)
}

pub async fn reconcile_roadmap(
    client: &CloudClient,
    engine: &mut RoadmapEngine,
) -> Result<usize, CloudError> {
    let envelopes = fetch_family(client, "roadmap").await?;
    apply_roadmap_records(engine, &envelopes)
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
