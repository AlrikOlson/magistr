use super::plan_native_render::{NATIVE_REQUIRED, render_next, render_show, show_json};
use super::*;
use crate::agent;
use crate::cli;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use color_eyre::eyre;

// ─── Plan ────────────────────────────────────────────────────────────────

pub async fn cmd_plan(
    action: Option<cli::PlanAction>,
    renderer: &dyn OutputRenderer,
    _env: &crate::env::MagistrEnv,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let default_dir = std::path::PathBuf::from(".");
    match action {
        None => cmd_plan_show(&default_dir, renderer),
        Some(cli::PlanAction::Show { ref dir }) => cmd_plan_show(dir, renderer),
        Some(cli::PlanAction::Next { ref dir }) => cmd_plan_next(dir, renderer),
        Some(cli::PlanAction::Status { ref dir }) => cmd_plan_status(dir, renderer),
        Some(cli::PlanAction::Export { dir }) => cmd_plan_export(&dir),
        Some(cli::PlanAction::Generate { goal, dir, backend }) => {
            let backend = crate::commands::resolve_backend(backend, inputs)?;
            cmd_plan_generate(&goal, &dir, backend, inputs).await
        }
    }
}

/// Open the YAML plan store for a project directory.
fn open_plan_store(dir: &std::path::Path) -> eyre::Result<crate::plan::YamlStore> {
    let yaml_path = crate::plan::yaml_path(dir);
    crate::plan::YamlStore::load(&yaml_path).suggest()
}

fn canonical_dir(dir: &std::path::Path) -> eyre::Result<std::path::PathBuf> {
    dir.canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()
}

/// `show` on a magistr project: the snapshot plus the pending tasks.
fn show_native(
    engines: &magistr_engine::NativeReader,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    let tasks = engines.plan_tasks().map_err(|error| eyre::eyre!(error))?;
    let snapshot = magistr_engine::NativePlanSnapshot::from_tasks(engines.project_id(), &tasks);
    if renderer.is_machine_readable() {
        renderer
            .json(&mut std::io::stdout(), &show_json(&snapshot, &tasks))
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
    } else {
        eprint!("{}", render_show(&snapshot, &tasks));
    }
    Ok(())
}

/// `next`: the task `plan_next` would hand an agent.
fn cmd_plan_next(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let dir = canonical_dir(dir)?;
    let Some(engines) = crate::plan::open_native_reader(&dir)? else {
        eyre::bail!("{NATIVE_REQUIRED}");
    };
    let next = engines.plan_next().map_err(|error| eyre::eyre!(error))?;
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

/// `status`: the bounded JSON `plan_status` answers over MCP, or the
/// human snapshot.
fn cmd_plan_status(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let dir = canonical_dir(dir)?;
    let Some(engines) = crate::plan::open_native_reader(&dir)? else {
        eyre::bail!("{NATIVE_REQUIRED}");
    };
    if renderer.is_machine_readable() {
        renderer
            .json(
                &mut std::io::stdout(),
                &engines.plan_status().map_err(eyre::Error::msg)?,
            )
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }
    show_native(&engines, renderer)
}

fn cmd_plan_show(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let dir = canonical_dir(dir)?;
    if let Some(engines) = crate::plan::open_native_reader(&dir)? {
        return show_native(&engines, renderer);
    }

    let store = open_plan_store(&dir)?;
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
                "Plan: {}/{} items complete ({pct:.0}%)",
                stats.checked_tasks, stats.total_tasks
            ))
        );

        for phase in index.phases() {
            let checked = phase.checked_tasks();
            let total = phase.total_tasks();
            let status_icon = match phase.status {
                crate::plan::PhaseStatus::Done => fmt::green("✓"),
                crate::plan::PhaseStatus::InProgress => fmt::yellow("●"),
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

/// `export` (alias `sync`): regenerate PLAN.md from the plan store.
fn cmd_plan_export(dir: &std::path::Path) -> eyre::Result<()> {
    let dir = canonical_dir(dir)?;
    magistr_core::plan::ensure_file_export_allowed(&dir)?;
    if let Some(engines) = crate::plan::open_native_reader(&dir)? {
        let md_path = dir.join("PLAN.md");
        let markdown = engines.plan_export("markdown").map_err(eyre::Error::msg)?;
        let snapshot = engines.plan_snapshot().map_err(eyre::Error::msg)?;
        magistr_core::plan::ensure_file_export_allowed(&dir)?;
        std::fs::write(&md_path, markdown)
            .map_err(|e| error::MagistrError::io(&md_path, e))
            .suggest()?;
        eprintln!(
            "{}",
            fmt::success(&format!(
                "Exported {} from magistr ({}/{} tasks done)",
                md_path.display(),
                snapshot.progress.checked_tasks,
                snapshot.progress.total_tasks
            ))
        );
        return Ok(());
    }

    let store = open_plan_store(&dir)?;
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

async fn cmd_plan_generate(
    goal: &str,
    dir: &std::path::Path,
    backend: agent::Backend,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let dir = dir.canonicalize()?;
    let inputs = inputs.for_project(&dir);

    eprintln!("{}", fmt::info(&format!("Generating plan for: {goal}")));
    eprintln!("  Target: the magistr plan for this project (tasks authored via `plan_add_task`)");

    let prompt = build_plan_generation_prompt(goal);

    let mut config = agent::AgentConfig::new(&prompt, &dir).with_inputs(&inputs);
    config = config.backend(backend);

    config = config.with_project_context(&dir);

    // The generator's session writes source the way every host's does:
    // through the admitted workspace, or not at all.
    let workspace =
        crate::commands::start::admit_workspace(&inputs, |line| eprintln!("{line}")).await;
    let mut session = agent::session::AgentSession::start(config, workspace.source_route())
        .await
        .suggest()?;

    eprintln!(
        "{}\n",
        fmt::info("Agent is researching and generating plan...")
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
                            eprintln!("{}", fmt::error("Plan generation failed"));
                        } else {
                            eprintln!(
                                "{}",
                                fmt::success(&format!(
                                    "Plan generated — {num_turns} turns, {duration_secs:.1}s, cost: {cost_str}"
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
        fmt::bold("magistr plan show"),
        fmt::bold("magistr next-loop")
    );

    Ok(())
}
