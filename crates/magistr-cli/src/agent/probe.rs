//! Zero-turn ACP session probe — inspect advertised models without sending a prompt.
//!
//! Explicit model previews and the model-list command share this discovery owner.
//! The probe connects to the agent, receives the `Initialized` event (which carries
//! `config_options` from `session/new`), then immediately closes — no prompt is sent.

use std::time::Duration;

use magistr_acp::{AcpEvent, AcpSessionMode, AcpTransport};
use tokio::time::timeout;

pub use super::probe_model::ProbeModel;
pub use super::provider_model_set::ProviderModelSet;

use super::message::{AcpConfigOption, ConfigOptionCategory};
use super::runtime::AgentRuntime;
use super::types::config_options_from_acp;

/// Maximum time to wait for the Initialized event from an agent.
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Probe a single agent runtime for its model list.
///
/// Spawns a zero-turn ACP session: `connect → Initialized → close`.
/// Returns the config options with `Model` category, or a visible discovery error.
async fn probe_runtime(
    runtime: &AgentRuntime,
    root: &std::path::Path,
) -> Result<Vec<AcpConfigOption>, String> {
    let mut transport = AcpTransport::new_with_args(
        runtime.path.clone(),
        runtime.args.clone(),
        Some(root.to_owned()),
    )
    .map_err(|error| error.to_string())?
    .session_mode(AcpSessionMode::New);
    let connected = timeout(PROBE_TIMEOUT, transport.connect())
        .await
        .map_err(|_| "The agent took too long to list its models.".to_owned())
        .and_then(|result| result.map_err(|error| error.to_string()));
    if let Err(error) = connected {
        let _ = transport.close().await;
        return Err(error);
    }
    probe_connected(transport).await
}

async fn probe_connected(mut transport: AcpTransport) -> Result<Vec<AcpConfigOption>, String> {
    let result = match transport.read_events() {
        Some(mut events) => timeout(PROBE_TIMEOUT, async {
            while let Some(event) = events.recv().await {
                match event {
                    Ok(AcpEvent::Initialized { config_options, .. }) => {
                        return Ok(config_options.unwrap_or_default());
                    }
                    Err(error) => return Err(error.to_string()),
                    _ => {}
                }
            }
            Err("The agent closed before listing its models.".into())
        })
        .await
        .map_err(|_| "The agent took too long to list its models.".to_owned())
        .and_then(|result| result),
        None => Err("The agent could not list its models.".into()),
    };
    let closed = transport.close().await.map_err(|error| error.to_string());
    let options = result?;
    closed?;
    Ok(config_options_from_acp(&options)
        .into_iter()
        .filter(|option| option.category == Some(ConfigOptionCategory::Model))
        .collect())
}

/// Inspect one selected agent's advertised models without a prompt or history write.
/// Project policy filters the offered values; admission validates the chosen value again.
pub async fn probe_models(
    inputs: &crate::config::inputs::ProjectInputs,
    provider: &str,
) -> Result<Vec<ProbeModel>, String> {
    let captured = inputs.clone();
    let provider = provider.to_owned();
    let (runtime, governance, root) = tokio::task::spawn_blocking(move || {
        let root = captured
            .project_root()
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let runtime = AgentRuntime::detect_for(&captured, Some(&provider))
            .map_err(|error| error.to_string())?;
        let governance = crate::team::TeamConfig::load(&root)
            .map_err(|error| error.to_string())?
            .map(|team| team.policy)
            .unwrap_or_default();
        Ok::<_, String>((runtime, governance, root))
    })
    .await
    .map_err(|error| error.to_string())??;
    Ok(probe_runtime(&runtime, &root)
        .await?
        .into_iter()
        .flat_map(|option| option.options)
        .filter(|value| governance.route_model(&value.value_id) == Some(value.value_id.as_str()))
        .map(|value| ProbeModel {
            id: value.value_id,
            name: value.name,
        })
        .collect())
}

/// Probe all available agent runtimes for their model lists.
///
/// Returns a `ProviderModelSet` for each detected runtime. Probes run
/// concurrently with a per-probe timeout. Failed probes return empty model
/// lists with `probed: false`.
///
/// Names and values remain exactly as the agent advertises them.
pub async fn probe_all_providers(
    inputs: &crate::config::inputs::ProjectInputs,
) -> Vec<ProviderModelSet> {
    let runtimes = AgentRuntime::detect_all_for(inputs);
    if runtimes.is_empty() {
        return vec![];
    }

    // Probe all runtimes concurrently
    let mut handles = Vec::new();
    for rt in &runtimes {
        let rt_clone = rt.clone();
        let root = inputs.project_root().to_owned();
        handles.push(tokio::spawn(async move {
            probe_runtime(&rt_clone, &root).await
        }));
    }

    let mut results = Vec::new();
    for (rt, handle) in runtimes.iter().zip(handles) {
        let config_opts = handle.await.ok().and_then(Result::ok).unwrap_or_default();

        let models: Vec<ProbeModel> = config_opts
            .iter()
            .flat_map(|opt| {
                opt.options.iter().map(|sv| ProbeModel {
                    id: sv.value_id.clone(),
                    name: sv.name.clone(),
                })
            })
            .collect();

        let probed = !models.is_empty();
        let provider_id = rt.provider_id().to_string();

        results.push(ProviderModelSet {
            provider_id,
            label: rt.provider_label().to_string(),
            models,
            probed,
        });
    }

    results
}

#[cfg(test)]
#[path = "probe.tests.rs"]
mod tests;
