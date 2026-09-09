use crate::cli;
use crate::detect;
use crate::error::{self, ResultExt};
use crate::memory;
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub fn cmd_memory(
    dir: &std::path::Path,
    action: Option<cli::MemoryAction>,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let mem_path = memory::memory_path(&dir);

    match action.unwrap_or(cli::MemoryAction::Show) {
        cli::MemoryAction::Show => {
            let mem = memory::storage::load(&mem_path)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            if renderer.is_machine_readable() {
                let data = serde_json::json!({
                    "path": mem_path.display().to_string(),
                    "project_name": mem.project_name,
                    "total_entries": mem.len(),
                    "entries": mem.entries.iter().map(|e| serde_json::json!({
                        "category": e.category.heading(),
                        "text": e.text,
                        "added_at": e.added_at.map(|t| t.to_rfc3339()),
                    })).collect::<Vec<_>>(),
                });
                renderer
                    .json(&mut std::io::stdout(), &data)
                    .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
                return Ok(());
            }

            if mem.is_empty() {
                eprintln!(
                    "{}",
                    fmt::dim(
                        "No project memory found. Run `magistr learn` to analyze your project."
                    )
                );
                return Ok(());
            }

            eprintln!(
                "{}\n",
                fmt::bold(&format!("🧠 Project Memory — {}", mem.project_name))
            );

            for &cat in memory::MemoryCategory::ALL {
                let entries = mem.by_category(cat);
                if entries.is_empty() {
                    continue;
                }

                eprintln!("  {}", fmt::bold(cat.heading()));
                for entry in entries {
                    if let Some(ts) = entry.added_at {
                        let date = ts.format("%Y-%m-%d");
                        eprintln!("    {} {}", fmt::dim(&format!("[{date}]")), entry.text);
                    } else {
                        eprintln!("    - {}", entry.text);
                    }
                }
                eprintln!();
            }

            eprintln!(
                "  {}",
                fmt::dim(&format!(
                    "{} entries total — edit at {}",
                    mem.len(),
                    mem_path.display()
                ))
            );
        }

        cli::MemoryAction::Add { text, category } => {
            let cat = memory::MemoryCategory::from_heading(&category)
                .or_else(|| match category.to_lowercase().as_str() {
                    "patterns" => Some(memory::MemoryCategory::Patterns),
                    "architecture" => Some(memory::MemoryCategory::Architecture),
                    "conventions" => Some(memory::MemoryCategory::Conventions),
                    "mistakes" => Some(memory::MemoryCategory::Mistakes),
                    "observations" => Some(memory::MemoryCategory::Observations),
                    _ => None,
                })
                .ok_or_else(|| {
                    error::MagistrError::Config(format!(
                        "unknown category '{category}'. Use: patterns, architecture, conventions, mistakes, observations"
                    ))
                })
                .suggest()?;

            let mut mem = memory::storage::load(&mem_path)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            if mem.project_name.is_empty()
                && let Ok(stack) = detect::detect_stack(&dir)
            {
                mem.project_name = stack.project_name;
            }

            mem.add(memory::MemoryEntry::now(cat, &text));

            memory::storage::save(&mem, &mem_path)
                .map_err(|e| error::MagistrError::Config(e.to_string()))
                .suggest()?;

            if renderer.is_machine_readable() {
                let data = serde_json::json!({
                    "added": { "category": cat.heading(), "text": text },
                    "total_entries": mem.len(),
                });
                renderer
                    .json(&mut std::io::stdout(), &data)
                    .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
            } else {
                eprintln!(
                    "{} Added to {}: {text}",
                    fmt::green("✓"),
                    fmt::cyan(cat.heading()),
                );
            }
        }

        cli::MemoryAction::Reset => {
            if mem_path.is_file() {
                std::fs::remove_file(&mem_path)
                    .map_err(|e| error::MagistrError::io(&mem_path, e))
                    .suggest()?;
                eprintln!(
                    "{}",
                    fmt::success(&format!("Removed project memory at {}", mem_path.display()))
                );
            } else {
                eprintln!("{}", fmt::dim("No project memory file found."));
            }
        }
    }

    Ok(())
}
