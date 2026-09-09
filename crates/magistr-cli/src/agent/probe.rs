//! Zero-turn ACP session probe — extracts provider capabilities without conversation.
//!
//! Used by the settings TUI to discover each provider's available models at launch.
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
/// Returns the config options with `Model` category, or an empty vec on failure.
async fn probe_runtime(runtime: &AgentRuntime) -> Vec<AcpConfigOption> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let transport_result =
        AcpTransport::new_with_args(runtime.path.clone(), runtime.args.clone(), Some(cwd));

    let mut transport = match transport_result {
        Ok(t) => t.session_mode(AcpSessionMode::New),
        Err(_) => return vec![],
    };

    // Connect — this does JSON-RPC initialize + session/new
    if transport.connect().await.is_err() {
        return vec![];
    }

    // Take the event receiver
    let Some(mut event_rx) = transport.read_events() else {
        let _ = transport.close().await;
        return vec![];
    };

    // Wait for the Initialized event (carries config_options)
    let config_options = timeout(PROBE_TIMEOUT, async {
        while let Some(event_result) = event_rx.recv().await {
            if let Ok(AcpEvent::Initialized { config_options, .. }) = event_result {
                return config_options;
            }
        }
        None
    })
    .await
    .unwrap_or(None);

    // Clean shutdown
    let _ = transport.close().await;

    // Convert to magistr types, filter to Model category
    match config_options {
        Some(opts) => {
            let all = config_options_from_acp(&opts);
            all.into_iter()
                .filter(|o| o.category == Some(ConfigOptionCategory::Model))
                .collect()
        }
        None => vec![],
    }
}

/// Probe all available agent runtimes for their model lists.
///
/// Returns a `ProviderModelSet` for each detected runtime. Probes run
/// concurrently with a per-probe timeout. Failed probes return empty model
/// lists with `probed: false`.
///
/// For Claude Code, model aliases (`default`, `sonnet`, `haiku`) are enriched
/// with full display names and subscription-aware descriptions via `claude auth status`.
pub async fn probe_all_providers() -> Vec<ProviderModelSet> {
    let runtimes = AgentRuntime::detect_all();
    if runtimes.is_empty() {
        return vec![];
    }

    // Probe all runtimes concurrently
    let mut handles = Vec::new();
    for rt in &runtimes {
        let rt_clone = rt.clone();
        handles.push(tokio::spawn(async move { probe_runtime(&rt_clone).await }));
    }

    let mut results = Vec::new();
    for (rt, handle) in runtimes.iter().zip(handles) {
        let config_opts = handle.await.unwrap_or_default();

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
