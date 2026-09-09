use crate::detect;
use crate::error::{self, ResultExt};
use crate::memory;
use crate::output::OutputRenderer;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub fn cmd_learn(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let stack = detect::detect_stack(&dir).suggest()?;
    let analyzed = memory::analyze(&stack, &dir);

    let mem_path = memory::memory_path(&dir);
    let mut existing = memory::storage::load(&mem_path)
        .map_err(|e| error::MagistrError::Config(e.to_string()))
        .suggest()?;

    // If memory is empty, use the project name from analysis
    if existing.project_name.is_empty() {
        existing.project_name.clone_from(&analyzed.project_name);
    }

    let before_count = existing.len();
    memory::merge(&mut existing, &analyzed);
    let added = existing.len() - before_count;

    memory::storage::save(&existing, &mem_path)
        .map_err(|e| error::MagistrError::Config(e.to_string()))
        .suggest()?;

    if renderer.is_machine_readable() {
        let data = serde_json::json!({
            "path": mem_path.display().to_string(),
            "total_entries": existing.len(),
            "new_entries": added,
            "categories": existing.category_counts().iter()
                .map(|(cat, count)| serde_json::json!({
                    "category": cat.heading(),
                    "count": count,
                }))
                .collect::<Vec<_>>(),
        });
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| error::MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }

    eprintln!(
        "{} Analyzed project {}",
        fmt::green("✓"),
        fmt::bold(&stack.project_name)
    );
    eprintln!();

    if added > 0 {
        eprintln!("  Added {added} new entries to project memory:");
    } else {
        eprintln!("  No new entries — memory is already up to date.");
    }

    for (cat, count) in existing.category_counts() {
        eprintln!("    {}: {count} entries", fmt::cyan(&cat.to_string()));
    }

    eprintln!();
    eprintln!("  {}", fmt::dim(&format!("Memory: {}", mem_path.display())));
    eprintln!(
        "  {}",
        fmt::dim("Edit this file to refine what magistr knows about your project.")
    );

    Ok(())
}
