//! `magistr archetypes` command — list configured archetypes.

use std::path::Path;

use color_eyre::eyre::{self, WrapErr};

/// List configured archetypes with language, framework, role, root path, and commands.
pub fn cmd_archetypes(dir: &Path, detail: bool, json_output: bool) -> eyre::Result<()> {
    let config = crate::validate::load_project_config(dir)
        .wrap_err("failed to load .magistr/project.json")?;

    let archetypes = config.effective_archetypes();

    if json_output {
        let json =
            serde_json::to_string_pretty(&archetypes).wrap_err("failed to serialize archetypes")?;
        println!("{json}");
        return Ok(());
    }

    if archetypes.is_empty() {
        println!("No archetypes configured.");
        return Ok(());
    }

    let is_synthesized = config.archetypes.is_empty();
    if is_synthesized {
        println!("Single-archetype project (synthesized from top-level config):\n");
    } else {
        println!(
            "{} archetype{} configured:\n",
            archetypes.len(),
            if archetypes.len() == 1 { "" } else { "s" }
        );
    }

    for arch in &archetypes {
        println!("  {} ({})", arch.id, arch.name);
        println!("    Language:  {}", arch.language);
        if let Some(fw) = &arch.framework {
            println!("    Framework: {fw}");
        }
        println!("    Root:      {}", arch.root.display());
        println!("    Role:      {}", arch.role);

        if detail {
            println!("    Commands:");
            println!("      check: {}", arch.quality_gate.check);
            println!("      test:  {}", arch.quality_gate.test);
            println!("      lint:  {}", arch.quality_gate.lint);
            println!(
                "    Format:    {} ({})",
                arch.format.command,
                arch.format.extensions.join(", ")
            );
            println!(
                "    Testing:   {} ({})",
                arch.testing.companion_pattern, arch.testing.test_dir
            );
            if !arch.source_patterns.is_empty() {
                println!("    Sources:   {}", arch.source_patterns.join(", "));
            }
            if let Some(naming) = &arch.naming_convention {
                println!("    Naming:    {naming}");
            }
            if let Some(model) = &arch.model_preference {
                println!("    Model:     {model}");
            }
        } else {
            println!("    Gates:     check | test | lint");
            println!(
                "    Format:    {}",
                if arch.format.command.is_empty() {
                    "(none)"
                } else {
                    &arch.format.command
                }
            );
        }
        println!();
    }

    if !config.relationships.is_empty() {
        println!("Relationships:");
        for rel in &config.relationships {
            println!("  {} --{:?}--> {}", rel.from, rel.kind, rel.to);
        }
        println!();
    }

    if let Some(topo) = &config.topology {
        println!("Topology: {topo:?}");
    }

    Ok(())
}
