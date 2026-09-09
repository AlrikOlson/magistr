//! `magistr preset list` — show all available presets with type and source.

use crate::config::project_preset::{self, PresetInfo};
use crate::config::user_preset;
use crate::output::OutputRenderer;
use crate::types::Language;
use crate::utils::format as fmt;
use color_eyre::eyre;

/// List all available presets: built-in, project-local, and user-global.
pub fn cmd_preset_list(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let mut all_presets: Vec<PresetInfo> = Vec::new();

    // Built-in language presets
    for lang in Language::ALL {
        all_presets.push(PresetInfo {
            name: lang.as_str().to_string(),
            source: "built-in".into(),
            layer_type: "language".into(),
            path: std::path::PathBuf::from(format!("presets/{}.json", lang.as_str())),
            description: Some(format!("{} language preset", lang.as_str())),
            extends: None,
        });
    }

    // Project-local presets
    let dir = if dir.as_os_str() == "." {
        std::env::current_dir().unwrap_or_else(|_| dir.to_path_buf())
    } else {
        dir.to_path_buf()
    };
    all_presets.extend(project_preset::list_project_presets(&dir));

    // User-global presets
    all_presets.extend(user_preset::list_user_preset_info());

    if renderer.is_machine_readable() {
        let json: Vec<serde_json::Value> = all_presets
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "source": p.source,
                    "layer_type": p.layer_type,
                    "path": p.path.display().to_string(),
                    "description": p.description,
                    "extends": p.extends,
                })
            })
            .collect();
        renderer
            .json(&mut std::io::stdout(), &serde_json::json!(json))
            .map_err(|e| eyre::eyre!(e))?;
        return Ok(());
    }

    if all_presets.is_empty() {
        eprintln!("{}", fmt::dim("No presets found."));
        return Ok(());
    }

    // Group by source
    let mut by_source: std::collections::BTreeMap<String, Vec<&PresetInfo>> =
        std::collections::BTreeMap::new();
    for p in &all_presets {
        by_source.entry(p.source.clone()).or_default().push(p);
    }

    for (source, presets) in &by_source {
        eprintln!("{}", fmt::bold(&format!("{source} presets:")));
        for p in presets {
            let extends_str = p
                .extends
                .as_deref()
                .map(|e| format!(" (extends: {e})"))
                .unwrap_or_default();
            let desc_str = p
                .description
                .as_deref()
                .map(|d| format!(" — {d}"))
                .unwrap_or_default();
            eprintln!(
                "  {} {} [{}]{extends_str}{desc_str}",
                fmt::green(&p.name),
                fmt::dim(&format!("({})", p.layer_type)),
                fmt::dim(&p.path.display().to_string()),
            );
        }
        eprintln!();
    }

    eprintln!(
        "{}",
        fmt::dim(&format!("Total: {} presets", all_presets.len()))
    );

    Ok(())
}
