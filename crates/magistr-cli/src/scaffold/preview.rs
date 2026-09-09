use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;

use super::{FileAction, FilePreview, ScaffoldPreview};

/// Preview what `generate()` would do without writing any files.
///
/// Returns a `ScaffoldPreview` describing each file's action (create, overwrite,
/// unchanged, or preserved) along with unified diffs for modified files.
pub fn preview(
    dir: &Path,
    template_vars: &HashMap<String, String>,
    generated_overrides: &HashMap<String, String>,
) -> Result<ScaffoldPreview> {
    super::existing_config::ExistingConfig::validate(dir)?;

    let mut vars = template_vars.clone();
    super::CodexSetup::prepare_vars(dir, &mut vars);
    vars.entry("project_root".to_string())
        .or_insert_with(|| ".".to_string());
    super::mcp_config::build(&vars)?;

    let codex = super::CodexSetup::plan(dir, &vars, false)?;
    let ownership_context = if super::CodexSetup::selected(&vars) {
        Some(super::preview_context::PreviewContext::capture(dir)?)
    } else {
        None
    };
    let links = if super::CodexSetup::selected(&vars) {
        super::codex_skill_links::CodexSkillLinks::capture(dir)?
    } else {
        super::codex_skill_links::CodexSkillLinks::default()
    };
    let mut files = super::scaffold_files(&vars);
    for f in &mut files {
        if let Some(content) = generated_overrides.get(f.path) {
            f.content = content.clone();
        }
    }
    // The same canonical-repo override generate applies.
    for f in &mut files {
        if let Some(content) = super::canonical_content(dir, f.path) {
            f.content = content;
        }
    }
    let manifest = super::manifest::ScaffoldManifest::load(dir);
    let mut previews = Vec::with_capacity(files.len());

    for f in &files {
        let full = dir.join(f.path);
        let existing = if links.covers(f.path) {
            links.read(dir, f.path)?
        } else {
            std::fs::read_to_string(&full).ok()
        };

        let action = if links.covers(f.path) {
            FileAction::Overwrite
        } else {
            f.policy
                .action(f.path, existing.as_deref(), &f.content, &manifest)
        };
        let new_content = if action == FileAction::Preserved {
            existing.as_deref().unwrap_or(&f.content).to_owned()
        } else {
            f.content.clone()
        };

        let diff = if let Some(description) = links.description(f.path) {
            Some(format!(
                "{description}{}",
                super::unified_diff(f.path, existing.as_deref().unwrap_or_default(), &f.content)
            ))
        } else if action == FileAction::Overwrite {
            existing
                .as_deref()
                .map(|old| super::unified_diff(f.path, old, &f.content))
        } else {
            None
        };

        previews.push(FilePreview {
            path: f.path.to_string(),
            action,
            new_content,
            existing_content: existing,
            ownership_context: if super::CodexSetup::owned_path(f.path) {
                ownership_context.clone()
            } else {
                None
            },
            diff,
        });
    }

    // Add .mcp.json preview (merge-aware).
    let mcp_path = dir.join(".mcp.json");
    let existing_mcp = std::fs::read_to_string(&mcp_path).ok();
    let (merged_content, mcp_action) = super::preview_mcp_json(dir, &vars)?;

    let mcp_diff = if mcp_action == FileAction::Overwrite {
        existing_mcp
            .as_deref()
            .map(|old| super::unified_diff(".mcp.json", old, &merged_content))
    } else {
        None
    };

    previews.push(FilePreview {
        path: ".mcp.json".to_string(),
        action: mcp_action,
        new_content: merged_content,
        existing_content: existing_mcp,
        ownership_context: None,
        diff: mcp_diff,
    });

    // Add .gitignore preview (merge-aware).
    let gi_path = dir.join(".gitignore");
    let existing_gi = std::fs::read_to_string(&gi_path).ok();
    let (gi_content, gi_action) = super::preview_gitignore(dir, &vars)?;

    let gi_diff = if gi_action == FileAction::Overwrite {
        existing_gi
            .as_deref()
            .map(|old| super::unified_diff(".gitignore", old, &gi_content))
    } else {
        None
    };

    previews.push(FilePreview {
        path: ".gitignore".to_string(),
        action: gi_action,
        new_content: gi_content,
        existing_content: existing_gi,
        ownership_context: None,
        diff: gi_diff,
    });

    previews.extend(codex);
    Ok(ScaffoldPreview { files: previews })
}
