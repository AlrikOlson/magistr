use crate::agent::Backend;
use crate::agent::probe::probe_all_providers;
use crate::error;
use crate::models;
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use color_eyre::eyre;

/// List the models each installed agent can actually run.
///
/// The provider probe is the source of truth: it reports exactly what the
/// agent advertises, so this command can never claim a model the agent would
/// reject. The built-in registry is only a pricing table — it annotates a row
/// when the advertised id resolves to a known entry, and stays silent when the
/// agent advertised a bare alias (`sonnet`, `opus`) whose concrete model only
/// the agent's own SDK knows.
pub async fn cmd_models(renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let user_file = models::load().map_err(|e| error::MagistrError::Config(e.to_string()))?;
    let registry = models::ModelRegistry::from_config(&user_file);
    let providers = probe_all_providers().await;

    if renderer.is_machine_readable() {
        return render_json(renderer, &registry, &providers);
    }
    render_text(&registry, &providers);
    Ok(())
}

fn render_json(
    renderer: &dyn OutputRenderer,
    registry: &models::ModelRegistry,
    providers: &[crate::agent::probe::ProviderModelSet],
) -> eyre::Result<()> {
    let data = build_models_json(registry, providers);
    renderer
        .json(&mut std::io::stdout(), &data)
        .map_err(|e| error::MagistrError::Model(e.to_string()))?;
    Ok(())
}

/// Build the `magistr models --json` payload.
///
/// Pure so it can be tested without spawning an agent: `providers` is whatever
/// the probe returned, and a model is priced only when its advertised id
/// resolves in the registry.
pub(crate) fn build_models_json(
    registry: &models::ModelRegistry,
    providers: &[crate::agent::probe::ProviderModelSet],
) -> serde_json::Value {
    let provider_json: Vec<serde_json::Value> = providers
        .iter()
        .map(|set| {
            let models: Vec<serde_json::Value> = set
                .models
                .iter()
                .map(|m| {
                    let priced = models::advertised_pricing(&m.id, registry);
                    serde_json::json!({
                        "id": m.id,
                        "name": m.name,
                        "cost_per_million_input": priced.map(|e| e.config.cost_per_million_input),
                        "cost_per_million_output": priced.map(|e| e.config.cost_per_million_output),
                    })
                })
                .collect();
            serde_json::json!({
                "provider": set.provider_id,
                "label": set.label,
                "probed": set.probed,
                "models": models,
            })
        })
        .collect();

    let pricing: Vec<serde_json::Value> = registry
        .all()
        .iter()
        .map(|entry| {
            serde_json::json!({
                "id": entry.config.id,
                "provider": entry.config.provider,
                "display_name": entry.config.display_name,
                "cost_per_million_input": entry.config.cost_per_million_input,
                "cost_per_million_output": entry.config.cost_per_million_output,
                "source": entry.source.label(),
            })
        })
        .collect();

    let backends: Vec<serde_json::Value> = Backend::all_compiled()
        .iter()
        .map(|b| {
            let caps = b.capabilities();
            serde_json::json!({
                "name": caps.name,
                "id": b.label(),
                "available": crate::agent::is_backend_available(*b),
                "streaming": caps.streaming,
                "session_resume": caps.session_resume,
                "model_selection": caps.model_selection,
                "mcp_support": caps.mcp_support,
                "permission_profiles": caps.permission_profiles,
            })
        })
        .collect();

    serde_json::json!({
        "providers": provider_json,
        "pricing_reference": pricing,
        "backends": backends,
    })
}

fn render_text(
    registry: &models::ModelRegistry,
    providers: &[crate::agent::probe::ProviderModelSet],
) {
    eprintln!("{}\n", fmt::bold("🤖 Models your agents can run"));

    if providers.is_empty() {
        eprintln!("  {}", fmt::dim("No agent runtimes detected."));
        eprintln!(
            "  {}",
            fmt::dim("Install Claude Code: curl -fsSL https://claude.ai/install.sh | bash")
        );
    }

    for set in providers {
        if !set.probed {
            eprintln!(
                "  {}  {}  {}",
                fmt::icon_fail(),
                fmt::bold(&set.label),
                fmt::dim("(not available)")
            );
            eprintln!();
            continue;
        }

        eprintln!("  {}  {}", fmt::icon_pass(), fmt::bold(&set.label));
        for model in &set.models {
            let price = match models::advertised_pricing(&model.id, registry) {
                Some(entry) => fmt::dim(&format!(
                    "  ${:.2}/M in  |  ${:.2}/M out",
                    entry.config.cost_per_million_input, entry.config.cost_per_million_output
                )),
                // A bare alias — only the agent's SDK knows what it resolves to.
                None => String::new(),
            };
            eprintln!("       {:<24} {}{}", model.id, model.name, price);
        }
        eprintln!();
    }

    render_pricing_reference(registry);
    render_backend_matrix();

    if let Some(config_path) = models::config::models_config_path() {
        eprintln!(
            "\n  {}",
            fmt::dim(&format!("Model config: {}", config_path.display()))
        );
    }
}

/// The built-in pricing table, labelled as reference rather than availability.
fn render_pricing_reference(registry: &models::ModelRegistry) {
    eprintln!("{}\n", fmt::bold("💲 Pricing reference"));
    if registry.is_empty() {
        eprintln!("  {}", fmt::dim("No models configured."));
        return;
    }
    for entry in registry.all() {
        let source_tag = match entry.source {
            models::ModelSource::Builtin => fmt::dim("(built-in)"),
            models::ModelSource::UserConfig => fmt::cyan("(user)"),
        };
        eprintln!(
            "  {}  {}  {source_tag}",
            fmt::bold(&entry.config.display_name),
            fmt::dim(&format!(
                "${:.2}/M in  |  ${:.2}/M out  |  id: {}",
                entry.config.cost_per_million_input,
                entry.config.cost_per_million_output,
                entry.config.id,
            ))
        );
    }
    eprintln!(
        "\n  {}",
        fmt::dim("Used to price usage and to route by cost — not a list of what is installed.")
    );
}

fn render_backend_matrix() {
    eprintln!("\n{}\n", fmt::bold("🔌 Backend Capabilities"));
    for backend in Backend::all_compiled() {
        let caps = backend.capabilities();
        let availability = if crate::agent::is_backend_available(backend) {
            fmt::icon_pass()
        } else {
            fmt::icon_fail()
        };
        eprintln!("  {availability}  {}", fmt::bold(caps.name));
        for (feature_name, supported) in caps.feature_list() {
            let icon = if supported { "✓" } else { "✗" };
            let formatted = if supported {
                fmt::green(icon)
            } else {
                fmt::dim(icon)
            };
            eprintln!("     {formatted}  {feature_name}");
        }
    }
}
