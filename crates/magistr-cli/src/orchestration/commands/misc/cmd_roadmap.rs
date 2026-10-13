use super::roadmap_native_render::{NATIVE_REQUIRED, render_next, render_show, show_json};
use super::*;
use crate::agent;
use crate::cli;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use color_eyre::eyre;

// ─── Roadmap ────────────────────────────────────────────────────────────────

pub async fn cmd_roadmap(
    action: Option<cli::RoadmapAction>,
    renderer: &dyn OutputRenderer,
    _env: &crate::env::MagistrEnv,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let default_dir = std::path::PathBuf::from(".");
    match action {
        None => cmd_roadmap_show(&default_dir, renderer),
        Some(cli::RoadmapAction::Show { ref dir }) => cmd_roadmap_show(dir, renderer),
        Some(cli::RoadmapAction::Next { ref dir }) => cmd_roadmap_next(dir, renderer),
        Some(cli::RoadmapAction::Status { ref dir }) => cmd_roadmap_status(dir, renderer),
        Some(cli::RoadmapAction::Export { dir }) => cmd_roadmap_export(&dir),
        Some(cli::RoadmapAction::Migrate { dir }) => cmd_roadmap_migrate(&dir),
        Some(cli::RoadmapAction::Generate { goal, dir, backend }) => {
            let backend = crate::commands::resolve_backend(backend, inputs)?;
            cmd_roadmap_generate(&goal, &dir, backend, inputs).await
        }
    }
}

/// Open the YAML roadmap store for a project directory.
fn open_roadmap_store(dir: &std::path::Path) -> eyre::Result<crate::roadmap::YamlStore> {
    let yaml_path = crate::roadmap::yaml_path(dir);
    crate::roadmap::YamlStore::load(&yaml_path).suggest()
}

fn canonical_dir(dir: &std::path::Path) -> eyre::Result<std::path::PathBuf> {
    dir.canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()
}

/// `show` on a think-and-ship project: the snapshot plus the pending chunks.
fn show_native(
    engines: &magistr_engine::NativeReader,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    let snapshot = engines.roadmap_snapshot();
    let chunks = engines.roadmap_chunks();
    if renderer.is_machine_readable() {
        renderer
            .json(&mut std::io::stdout(), &show_json(&snapshot, &chunks))
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
    } else {
        eprint!("{}", render_show(&snapshot, &chunks));
    }
    Ok(())
}

/// `next`: the chunk `roadmap_next` would hand an agent.
fn cmd_roadmap_next(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let dir = canonical_dir(dir)?;
    let Some(engines) = crate::roadmap::open_native_reader(&dir) else {
        eyre::bail!("{NATIVE_REQUIRED}");
    };
    let next = engines.roadmap_next();
    if renderer.is_machine_readable() {
        let data = next
            .as_ref()
            .map(|c| serde_json::to_value(c).unwrap_or(serde_json::Value::Null))
            .unwrap_or(serde_json::Value::Null);
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
    } else {
        eprint!("{}", render_next(next.as_ref()));
    }
    Ok(())
}

/// `status`: the bounded JSON `roadmap_status` answers over MCP, or the
/// human snapshot.
fn cmd_roadmap_status(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let dir = canonical_dir(dir)?;
    let Some(engines) = crate::roadmap::open_native_reader(&dir) else {
        eyre::bail!("{NATIVE_REQUIRED}");
    };
    if renderer.is_machine_readable() {
        renderer
            .json(&mut std::io::stdout(), &engines.roadmap_status())
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }
    show_native(&engines, renderer)
}

fn cmd_roadmap_show(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let dir = canonical_dir(dir)?;
    if let Some(engines) = crate::roadmap::open_native_reader(&dir) {
        return show_native(&engines, renderer);
    }

    let store = open_roadmap_store(&dir)?;
    let index = store.index();
    let stats = index.stats();
    let pct = stats.progress() * 100.0;

    if renderer.is_machine_readable() {
        let phase_data: Vec<serde_json::Value> = index
            .phases()
            .iter()
            .map(|p| {
                serde_json::json!({
                    "phase_id": p.id,
                    "name": p.name,
                    "status": p.status.to_string(),
                    "total": p.total_tasks(),
                    "checked": p.checked_tasks(),
                })
            })
            .collect();

        let data = serde_json::json!({
            "total_phases": stats.total_phases,
            "completed_phases": stats.completed_phases,
            "total_tasks": stats.total_tasks,
            "checked_tasks": stats.checked_tasks,
            "progress": format!("{pct:.0}%"),
            "phases": phase_data,
        });
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
    } else {
        eprintln!(
            "{}",
            fmt::info(&format!(
                "Roadmap: {}/{} items complete ({pct:.0}%)",
                stats.checked_tasks, stats.total_tasks
            ))
        );

        for phase in index.phases() {
            let checked = phase.checked_tasks();
            let total = phase.total_tasks();
            let status_icon = match phase.status {
                crate::roadmap::PhaseStatus::Done => fmt::green("✓"),
                crate::roadmap::PhaseStatus::InProgress => fmt::yellow("●"),
                _ => fmt::dim("○"),
            };
            let phase_label = format!("{}: {}", phase.id, phase.name);
            eprintln!("  {status_icon} {phase_label}  {checked}/{total}");
        }

        // Next unchecked items
        let next = index.next_unchecked(5);
        if !next.is_empty() {
            eprintln!("\n  Next items:");
            for (i, task_match) in next.iter().enumerate() {
                eprintln!(
                    "  {}. [{}] {}",
                    i + 1,
                    task_match.phase_name,
                    task_match.text
                );
            }
            let remaining = stats.total_tasks - stats.checked_tasks;
            if remaining > 5 {
                eprintln!("  ... and {} more", remaining - 5);
            }
        } else if stats.total_tasks > 0 {
            eprintln!("{}", fmt::success("All items complete!"));
        }
    }
    Ok(())
}

/// `export` (alias `sync`): regenerate ROADMAP.md from the roadmap store.
fn cmd_roadmap_export(dir: &std::path::Path) -> eyre::Result<()> {
    let dir = canonical_dir(dir)?;
    magistr_core::roadmap::ensure_file_export_allowed(&dir)?;
    if let Some(engines) = crate::roadmap::open_native_reader(&dir) {
        let md_path = dir.join("ROADMAP.md");
        let markdown = engines.roadmap_export("markdown");
        magistr_core::roadmap::ensure_file_export_allowed(&dir)?;
        std::fs::write(&md_path, markdown)
            .map_err(|e| error::MagistrError::io(&md_path, e))
            .suggest()?;
        let snapshot = engines.roadmap_snapshot();
        eprintln!(
            "{}",
            fmt::success(&format!(
                "Exported {} from think-and-ship ({}/{} chunks done)",
                md_path.display(),
                snapshot.progress.checked_tasks,
                snapshot.progress.total_tasks
            ))
        );
        return Ok(());
    }

    let store = open_roadmap_store(&dir)?;
    let md_path = store.export_markdown().suggest()?;

    let stats = store.index().stats();
    eprintln!(
        "{}",
        fmt::success(&format!(
            "Exported {} ({}/{} tasks done)",
            md_path.display(),
            stats.checked_tasks,
            stats.total_tasks
        ))
    );
    Ok(())
}

fn cmd_roadmap_migrate(dir: &std::path::Path) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    if crate::roadmap::backend_for(&dir).is_native() {
        eprintln!(
            "{}",
            fmt::warning(
                "This project runs the think-and-ship roadmap: ROADMAP.md is a generated view \
                 (`magistr roadmap export`), not a source to migrate. Import a legacy file \
                 with `think-and-ship import --file <ROADMAP.md|.magistr-roadmap.yml>`.",
            )
        );
        return Ok(());
    }

    let yaml_path = crate::roadmap::yaml_path(&dir);

    if yaml_path.exists() {
        eprintln!(
            "{}",
            fmt::warning(
                "YAML roadmap already exists at .magistr-roadmap.yml — no migration needed.",
            )
        );
        // Still patch project.json if it references the old spec file
        patch_project_json_spec_file(&dir);
        return Ok(());
    }

    // Check for legacy .magistr/roadmap.yaml
    let legacy_path = crate::roadmap::legacy_yaml_path(&dir);
    if legacy_path.exists() {
        eprintln!(
            "{}",
            fmt::info("Migrating .magistr/roadmap.yaml → .magistr-roadmap.yml")
        );

        let store = crate::roadmap::YamlStore::load(&legacy_path).suggest()?;
        let stats = store.index().stats();
        let mut migrated =
            crate::roadmap::YamlStore::from_roadmap(&yaml_path, store.index().roadmap().clone())
                .suggest()?;
        migrated.save().suggest()?;

        patch_project_json_spec_file(&dir);

        eprintln!(
            "{}",
            fmt::success(&format!(
                "Migration complete: {} phases, {}/{} tasks done.",
                stats.total_phases, stats.checked_tasks, stats.total_tasks,
            ))
        );
        return Ok(());
    }

    // Look for ROADMAP.md to migrate
    let md_path = dir.join("ROADMAP.md");
    if !md_path.exists() {
        eyre::bail!(
            "No ROADMAP.md found in {}. Nothing to migrate.",
            dir.display()
        );
    }

    eprintln!(
        "{}",
        fmt::info(&format!(
            "Migrating {} → .magistr-roadmap.yml",
            md_path.display()
        ))
    );

    let roadmap = crate::roadmap::parse_markdown_file(&md_path).suggest()?;
    let stats_before = roadmap.phases.iter().fold((0usize, 0usize), |(t, c), p| {
        (t + p.total_tasks(), c + p.checked_tasks())
    });

    let mut store = crate::roadmap::YamlStore::from_roadmap(&yaml_path, roadmap).suggest()?;
    store.save().suggest()?;

    patch_project_json_spec_file(&dir);

    eprintln!(
        "{}",
        fmt::success(&format!(
            "Migration complete: {} phases, {}/{} tasks done.",
            store.index().stats().total_phases,
            stats_before.1,
            stats_before.0,
        ))
    );
    Ok(())
}

/// Patch `spec.file` in `.magistr/project.json` from `"ROADMAP.md"` to `".magistr-roadmap.yml"`.
fn patch_project_json_spec_file(dir: &std::path::Path) {
    let Ok(config_path) = crate::paths::project_json(dir) else {
        return;
    };
    if !config_path.exists() {
        return;
    }
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return;
    };
    if !content.contains("\"ROADMAP.md\"") {
        return;
    }
    let patched = content.replace("\"ROADMAP.md\"", "\".magistr-roadmap.yml\"");
    if std::fs::write(&config_path, patched).is_ok() {
        eprintln!(
            "  {}",
            fmt::dim("Updated .magistr/project.json: spec.file → .magistr-roadmap.yml")
        );
    }
}

async fn cmd_roadmap_generate(
    goal: &str,
    dir: &std::path::Path,
    backend: agent::Backend,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = dir.canonicalize()?;
    let inputs = inputs.for_project(&dir);

    eprintln!("{}", fmt::info(&format!("Generating roadmap for: {goal}")));
    eprintln!(
        "  Target: the think-and-ship roadmap for this project (chunks authored via `roadmap_add_chunk`)"
    );

    let prompt = build_roadmap_generation_prompt(goal);

    let mut config = agent::AgentConfig::new(&prompt, &dir).with_inputs(&inputs);
    config = config.backend(backend);

    config = config.with_project_context(&dir);

    // The generator's session writes source the way every host's does:
    // through the admitted workspace, or not at all.
    let workspace =
        crate::commands::run::admit_workspace(&inputs, |line| eprintln!("{line}")).await;
    let mut session = agent::session::AgentSession::start(config, workspace.source_route())
        .await
        .suggest()?;

    eprintln!(
        "{}\n",
        fmt::info("Agent is researching and generating roadmap...")
    );

    loop {
        let mut turn_done = false;
        while let Some(msg_result) = session.next_message().await {
            match msg_result {
                Ok(msg) => match &msg {
                    crate::agent::message::AgentMessage::Assistant { message, .. } => {
                        for block in &message.content {
                            if let crate::agent::message::ContentBlock::Text { text } = block {
                                print!("{text}");
                            }
                        }
                    }
                    crate::agent::message::AgentMessage::Result {
                        is_error,
                        num_turns,
                        duration_ms,
                        total_cost_usd,
                        ..
                    } => {
                        let duration_secs = *duration_ms as f64 / 1000.0;
                        let cost_str = total_cost_usd
                            .map(|c| format!("${c:.4}"))
                            .unwrap_or_else(|| "unknown".into());
                        eprintln!();
                        if *is_error {
                            eprintln!("{}", fmt::error("Roadmap generation failed"));
                        } else {
                            eprintln!(
                                "{}",
                                fmt::success(&format!(
                                    "Roadmap generated — {num_turns} turns, {duration_secs:.1}s, cost: {cost_str}"
                                ))
                            );
                        }
                        turn_done = true;
                        break;
                    }
                    _ => {}
                },
                Err(e) => {
                    eprintln!("\n{}", fmt::error(&format!("Error: {e}")));
                    turn_done = true;
                    break;
                }
            }
        }
        if turn_done || session.next_message().await.is_none() {
            break;
        }
    }
    drop(session);
    if let Err(err) = workspace.shutdown().await {
        eprintln!("magistr: workspace shutdown: {err}");
    }

    eprintln!(
        "  Run {} to see it, {} to start executing it.",
        fmt::bold("magistr roadmap show"),
        fmt::bold("magistr next-loop")
    );

    Ok(())
}
