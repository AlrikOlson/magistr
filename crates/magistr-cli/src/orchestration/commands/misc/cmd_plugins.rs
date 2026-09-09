use crate::cli;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::plugin;
use crate::utils::format as fmt;
use color_eyre::eyre;

// ─── Plugins ────────────────────────────────────────────────────────────────

pub fn cmd_plugins(
    action: Option<cli::PluginAction>,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    let base_dir = plugin::plugins_dir().ok_or_else(|| {
        error::MagistrError::Config("could not determine config directory (HOME not set)".into())
    })?;

    match action.unwrap_or(cli::PluginAction::List) {
        cli::PluginAction::List => {
            let registry = plugin::PluginRegistry::load(&base_dir)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            if renderer.is_machine_readable() {
                let plugins: Vec<serde_json::Value> = registry
                    .list()
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "name": p.manifest.name,
                            "version": p.manifest.version,
                            "kind": p.manifest.kind.label(),
                            "description": p.manifest.description,
                            "source": p.source.to_string(),
                            "installed_at": p.installed_at,
                        })
                    })
                    .collect();
                renderer
                    .json(&mut std::io::stdout(), &serde_json::json!(plugins))
                    .map_err(|e| error::MagistrError::Plugin(e.to_string()))?;
                return Ok(());
            }

            if registry.is_empty() {
                eprintln!("{}", fmt::dim("No plugins installed."));
                eprintln!(
                    "{}",
                    fmt::dim("  Install one with: magistr plugins add <path-to-plugin-dir>")
                );
                return Ok(());
            }

            eprintln!("{}\n", fmt::bold("🔌 Installed Plugins"));

            for p in registry.list() {
                let desc = p
                    .manifest
                    .description
                    .as_deref()
                    .unwrap_or("no description");
                eprintln!(
                    "  {} {}  [{}]  {}",
                    fmt::cyan(&p.manifest.name),
                    fmt::dim(&format!("v{}", p.manifest.version)),
                    p.manifest.kind,
                    fmt::dim(desc),
                );
                eprintln!("    {}", fmt::dim(&format!("Source: {}", p.source)));
            }

            eprintln!(
                "\n  {}",
                fmt::dim(&format!("{} plugin(s) installed", registry.len()))
            );
        }

        cli::PluginAction::Add { path } => {
            let path = path
                .canonicalize()
                .map_err(|e| error::MagistrError::io(&path, e))
                .suggest()?;

            let mut registry = plugin::PluginRegistry::load(&base_dir)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            let installed = registry
                .add_local(&path)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            let name = installed.manifest.name.clone();
            let kind = installed.manifest.kind;

            registry
                .save()
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            if renderer.is_machine_readable() {
                let data = serde_json::json!({
                    "installed": name,
                    "kind": kind.label(),
                    "source": path.display().to_string(),
                });
                renderer
                    .json(&mut std::io::stdout(), &data)
                    .map_err(|e| error::MagistrError::Plugin(e.to_string()))?;
            } else {
                eprintln!(
                    "{} Installed plugin {} ({kind})",
                    fmt::green("✓"),
                    fmt::bold(&name)
                );
            }
        }

        cli::PluginAction::Remove { name } => {
            let mut registry = plugin::PluginRegistry::load(&base_dir)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            let removed = registry
                .remove(&name)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            registry
                .save()
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            if renderer.is_machine_readable() {
                let data = serde_json::json!({
                    "removed": removed.manifest.name,
                });
                renderer
                    .json(&mut std::io::stdout(), &data)
                    .map_err(|e| error::MagistrError::Plugin(e.to_string()))?;
            } else {
                eprintln!(
                    "{} Removed plugin {}",
                    fmt::green("✓"),
                    fmt::bold(&removed.manifest.name)
                );
            }
        }

        cli::PluginAction::Install {
            name,
            index,
            public_key,
        } => {
            let index_url = index.as_deref().unwrap_or(plugin::index::DEFAULT_INDEX_URL);

            if !renderer.is_machine_readable() {
                eprintln!("  {} Fetching plugin index...", fmt::dim("⟳"));
            }

            let registry_index = plugin::RegistryIndex::fetch(index_url)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            let entry = registry_index
                .find(&name)
                .ok_or_else(|| {
                    error::MagistrError::Config(format!(
                        "plugin '{name}' not found in registry index"
                    ))
                })
                .suggest()?
                .clone();

            if !renderer.is_machine_readable() {
                eprintln!(
                    "  {} Downloading {} v{}...",
                    fmt::dim("⟳"),
                    fmt::cyan(&entry.name),
                    entry.version
                );
            }

            let mut registry = plugin::PluginRegistry::load(&base_dir)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            let installed = registry
                .install_from_index(&entry, public_key.as_deref())
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            let inst_name = installed.manifest.name.clone();
            let inst_kind = installed.manifest.kind;
            let inst_version = installed.manifest.version.clone();

            registry
                .save()
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            if renderer.is_machine_readable() {
                let data = serde_json::json!({
                    "installed": inst_name,
                    "version": inst_version,
                    "kind": inst_kind.label(),
                    "source": format!("registry:{inst_name}@{inst_version}"),
                });
                renderer
                    .json(&mut std::io::stdout(), &data)
                    .map_err(|e| error::MagistrError::Plugin(e.to_string()))?;
            } else {
                eprintln!(
                    "{} Installed plugin {} v{} ({inst_kind}) from registry",
                    fmt::green("✓"),
                    fmt::bold(&inst_name),
                    inst_version
                );
            }
        }

        cli::PluginAction::Search { query, index } => {
            let index_url = index.as_deref().unwrap_or(plugin::index::DEFAULT_INDEX_URL);

            let registry_index = plugin::RegistryIndex::fetch(index_url)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            let plugins: Vec<&plugin::IndexEntry> = if let Some(q) = &query {
                let q_lower = q.to_lowercase();
                registry_index
                    .plugins
                    .iter()
                    .filter(|e| {
                        e.name.to_lowercase().contains(&q_lower)
                            || e.description
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&q_lower)
                    })
                    .collect()
            } else {
                registry_index.plugins.iter().collect()
            };

            if renderer.is_machine_readable() {
                let data: Vec<serde_json::Value> = plugins
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "name": e.name,
                            "version": e.version,
                            "kind": e.kind.label(),
                            "description": e.description,
                            "signed": e.signature.is_some(),
                        })
                    })
                    .collect();
                renderer
                    .json(&mut std::io::stdout(), &serde_json::json!(data))
                    .map_err(|e| error::MagistrError::Plugin(e.to_string()))?;
                return Ok(());
            }

            if plugins.is_empty() {
                eprintln!("{}", fmt::dim("No plugins found matching your query."));
                return Ok(());
            }

            eprintln!("{}\n", fmt::bold("📦 Available Plugins"));

            for e in &plugins {
                let desc = e.description.as_deref().unwrap_or("no description");
                let signed = if e.signature.is_some() {
                    " ✓signed"
                } else {
                    ""
                };
                eprintln!(
                    "  {} {}  [{}]  {}{}",
                    fmt::cyan(&e.name),
                    fmt::dim(&format!("v{}", e.version)),
                    e.kind,
                    fmt::dim(desc),
                    fmt::dim(signed),
                );
            }

            eprintln!(
                "\n  {}",
                fmt::dim(&format!("{} plugin(s) available", plugins.len()))
            );
        }

        cli::PluginAction::Validate { path } => {
            let path = path
                .canonicalize()
                .map_err(|e| error::MagistrError::io(&path, e))
                .suggest()?;

            let warnings = plugin::validate_plugin(&path)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            if renderer.is_machine_readable() {
                let data = serde_json::json!({
                    "valid": true,
                    "warnings": warnings,
                });
                renderer
                    .json(&mut std::io::stdout(), &data)
                    .map_err(|e| error::MagistrError::Plugin(e.to_string()))?;
                return Ok(());
            }

            let manifest = plugin::load_manifest(&path)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            eprintln!(
                "{} Plugin {} v{} ({}) is valid",
                fmt::green("✓"),
                fmt::bold(&manifest.name),
                manifest.version,
                manifest.kind
            );

            if !warnings.is_empty() {
                eprintln!();
                for w in &warnings {
                    eprintln!("  {} {w}", fmt::yellow("⚠"));
                }
            }
        }
    }

    Ok(())
}

// ─── Template vars ──────────────────────────────────────────────────────────

/// Build MCP permission entries for Claude Code `settings.json`.
///
/// Combines built-in servers (magistr, workbench-mcp) with user-selected servers
/// into formatted JSON array entries: `"mcp__servername"`.
pub(crate) fn build_mcp_permissions(selected_servers: &[String]) -> String {
    use crate::scaffold::registry;
    registry::BUILTIN_MCP_SERVERS
        .iter()
        .copied()
        .chain(selected_servers.iter().map(|s| s.as_str()))
        .map(|name| format!("\"mcp__{name}\""))
        .collect::<Vec<_>>()
        .join(",\n      ")
}
