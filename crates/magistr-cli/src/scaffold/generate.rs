use std::collections::HashMap;
use std::path::Path;

use crate::error::{MagistrError, Result};

use super::manifest::ScaffoldManifest;

/// Result of a generate/reinit operation, with per-file outcome tracking.
#[derive(Debug, Clone)]
pub struct GenerateResult {
    /// Files that were created (did not exist before).
    pub created: Vec<String>,
    /// Files that were updated (existed but content changed).
    pub updated: Vec<String>,
    /// Files that were unchanged (existed with identical content).
    pub unchanged: Vec<String>,
    /// Files that were preserved (user customizations kept).
    pub preserved: Vec<String>,
}

impl GenerateResult {
    /// All files that were written (created + updated).
    pub fn written(&self) -> Vec<String> {
        let mut all = self.created.clone();
        all.extend(self.updated.iter().cloned());
        all
    }

    /// All paths involved (for backward compat with code expecting Vec<String>).
    pub fn all_paths(&self) -> Vec<String> {
        let mut all = self.created.clone();
        all.extend(self.updated.iter().cloned());
        all.extend(self.unchanged.iter().cloned());
        all.extend(self.preserved.iter().cloned());
        all
    }
}

/// Generate the full `.claude/` scaffold in `dir`.
///
/// Uses diff-based updating: skips unchanged files, preserves user customizations
/// for `PreserveCustomized` files (detected via manifest hash comparison), and
/// always overwrites magistr-internal files.
///
/// Returns a `GenerateResult` with detailed per-file outcomes.
pub fn generate(
    dir: &Path,
    template_vars: &HashMap<String, String>,
    generated_overrides: &HashMap<String, String>,
) -> Result<Vec<String>> {
    let result = generate_with_result(dir, template_vars, generated_overrides)?;
    Ok(result.written())
}

/// Generate scaffold with detailed result tracking.
///
/// Same as `generate()` but returns a `GenerateResult` instead of just written paths.
pub fn generate_with_result(
    dir: &Path,
    template_vars: &HashMap<String, String>,
    generated_overrides: &HashMap<String, String>,
) -> Result<GenerateResult> {
    super::existing_config::ExistingConfig::validate(dir)?;

    // Host configuration stays portable; the native launcher discovers the
    // workspace and dependencies when the MCP client starts it.
    let mut vars = template_vars.clone();
    super::CodexSetup::prepare_vars(dir, &mut vars);
    vars.entry("project_root".to_string())
        .or_insert_with(|| ".".to_string());
    // Invalid explicit argv must fail before any scaffold writes.
    super::mcp_config::build(&vars)?;

    let codex = super::CodexSetup::plan(dir, &vars, false)?;
    let links = if super::CodexSetup::selected(&vars) {
        super::codex_skill_links::CodexSkillLinks::capture(dir)?
    } else {
        super::codex_skill_links::CodexSkillLinks::default()
    };
    let mut files = super::scaffold_files(&vars);

    // Apply AI-generated overrides: replace static template content with AI content.
    if !generated_overrides.is_empty() {
        let scaffold_paths: std::collections::HashSet<&str> =
            files.iter().map(|f| f.path).collect();
        let mut matched = 0usize;

        for f in &mut files {
            if let Some(content) = generated_overrides.get(f.path) {
                tracing::debug!(path = f.path, "Applied AI-generated file override");
                f.content = content.clone();
                matched += 1;
            }
        }

        for key in generated_overrides.keys() {
            if !scaffold_paths.contains(key.as_str()) {
                tracing::warn!(
                    path = key.as_str(),
                    "AI-generated override did not match any scaffold file path — content ignored"
                );
            }
        }

        tracing::debug!(
            total = generated_overrides.len(),
            matched,
            orphaned = generated_overrides.len() - matched,
            "Generated file override summary"
        );
    }

    // A vendored skill scaffolded into its own home repository is the
    // canonical file itself — byte for byte, no provenance marker.
    for f in &mut files {
        if let Some(content) = super::canonical_content(dir, f.path) {
            f.content = content;
        }
    }

    // All configuration and link preflights have passed. Detach legacy links
    // before creating directories or writing either host's generated files.
    links.migrate(dir, &files.iter().map(|f| f.path).collect::<Vec<_>>())?;

    // Collect all directories we need to create.
    let mut dirs: Vec<std::path::PathBuf> = files
        .iter()
        .filter_map(|f| dir.join(f.path).parent().map(Path::to_path_buf))
        .collect();
    dirs.sort();
    dirs.dedup();

    for d in &dirs {
        std::fs::create_dir_all(d).map_err(|e| MagistrError::io(d, e))?;
    }

    // Load the existing manifest for customization detection.
    let mut manifest = ScaffoldManifest::load(dir);

    let mut result = GenerateResult {
        created: Vec::new(),
        updated: Vec::new(),
        unchanged: Vec::new(),
        preserved: Vec::new(),
    };

    for f in &files {
        let full = dir.join(f.path);
        let existing = std::fs::read_to_string(&full).ok();

        let action = if links.covers(f.path) {
            super::FileAction::Overwrite
        } else {
            f.policy
                .action(f.path, existing.as_deref(), &f.content, &manifest)
        };
        match action {
            super::FileAction::Preserved => {
                result.preserved.push(f.path.to_owned());
                continue;
            }
            super::FileAction::Unchanged => {
                if f.executable {
                    super::CodexSetup::ensure_executable(&full)?;
                }
                result.unchanged.push(f.path.to_owned());
                super::CodexSetup::mark_file(&mut manifest, f.path);
                manifest.record(f.path, &f.content);
                continue;
            }
            super::FileAction::Create | super::FileAction::Overwrite => {}
        }

        // Write the file.
        std::fs::write(&full, &f.content).map_err(|e| MagistrError::io(&full, e))?;

        #[cfg(unix)]
        if f.executable {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&full, perms).map_err(|e| MagistrError::io(&full, e))?;
        }

        // Record the hash of what we just wrote.
        super::CodexSetup::mark_file(&mut manifest, f.path);
        manifest.record(f.path, &f.content);

        if existing.is_some() || links.covers(f.path) {
            result.updated.push(f.path.to_string());
        } else {
            result.created.push(f.path.to_string());
        }
    }

    // Keep the think-and-ship section of CLAUDE.md current. The file itself is
    // user-owned (NeverOverwrite); only the marked block between
    // `<!-- think-and-ship -->` markers is ours to write or refresh.
    match super::think_and_ship::upsert_claude_md_section(dir, &vars) {
        Ok(true) => {
            let claude = "CLAUDE.md".to_string();
            if !result.created.contains(&claude) && !result.updated.contains(&claude) {
                result.updated.push(claude);
            }
        }
        Ok(false) => {}
        Err(e) => tracing::warn!("could not refresh the think-and-ship section of CLAUDE.md: {e}"),
    }

    // Migration: remove rule files for retired navigation servers (replaced
    // by tools.md + the workbench-routing skill).
    for legacy in &[
        ".claude/rules/magistr-scope.md",
        ".cursor/rules/magistr-scope.mdc",
        ".claude/rules/iris-scope.md",
        ".claude/rules/iris-playbook.md",
        ".claude/rules/ministr-scope.md",
        ".claude/rules/ministr-playbook.md",
    ] {
        let path = dir.join(legacy);
        if path.is_file() {
            let _ = std::fs::remove_file(&path);
        }
    }

    // Merge magistr's MCP servers into .mcp.json (create or update, preserving user entries).
    super::merge_mcp_json(dir, &vars)?;
    super::strip_legacy_mcp_entries(dir)?;
    result.updated.push(".mcp.json".to_string());

    // Seed workbench `run`'s allowlist from the gates; a table the human
    // already declared is never touched.
    let seed = super::WorkbenchSeed::from_template_vars(&vars);
    if !seed.is_empty() {
        match seed.apply(dir) {
            Ok(true) => result.created.push("workbench.toml".to_string()),
            Ok(false) => {}
            Err(e) => tracing::warn!("could not seed workbench.toml: {e}"),
        }
    }

    // Merge magistr's patterns into .gitignore (create or update, preserving user entries).
    super::merge_gitignore(dir, &vars)?;
    result.updated.push(".gitignore".to_string());

    // Apply template plugin files (best-effort — plugin failures don't block scaffolding)
    let plugin_files = super::apply_template_plugins(dir, &vars);
    result.created.extend(plugin_files);

    // If the AI generated a roadmap.yaml, write it ONLY if one doesn't already exist.
    // An existing roadmap.yaml is user/tool-created data — never overwrite it.
    // A think-and-ship project (the scaffolded default) keeps its roadmap in
    // the engines — Generate mode authors chunks there — so no YAML is written.
    if !crate::roadmap::backend_for(dir).is_native()
        && let Some(yaml_content) = generated_overrides.get(".magistr/roadmap.yaml")
    {
        let yaml_path = crate::roadmap::yaml_path(dir);
        if !yaml_path.exists() {
            if let Some(parent) = yaml_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            if std::fs::write(&yaml_path, yaml_content).is_ok() {
                result.created.push(".magistr/roadmap.yaml".to_string());
                if let Ok(mut store) = crate::roadmap::YamlStore::load(&yaml_path) {
                    // save() regenerates ROADMAP.md from the YAML source
                    let _ = store.save();
                    if !result.created.contains(&"ROADMAP.md".to_string())
                        && !result.updated.contains(&"ROADMAP.md".to_string())
                    {
                        result.updated.push("ROADMAP.md".to_string());
                    }
                }
            }
        }
    }

    // Shared Codex components use the exact same preflighted plan as preview.
    for f in codex {
        match f.action {
            super::FileAction::Create | super::FileAction::Overwrite => {
                let full = dir.join(&f.path);
                if let Some(parent) = full.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
                }
                std::fs::write(&full, &f.new_content).map_err(|e| MagistrError::io(&full, e))?;
                super::CodexSetup::record(&mut manifest, &f)?;
                manifest.save(dir)?;
                if f.action == super::FileAction::Create {
                    result.created.push(f.path);
                } else {
                    result.updated.push(f.path);
                }
            }
            _ => result.unchanged.push(f.path),
        }
    }
    if super::CodexSetup::selected(&vars) {
        manifest.codex.get_or_insert_default().selected = true;
    }

    // Save the updated manifest.
    manifest.save(dir)?;

    Ok(result)
}

#[cfg(test)]
#[path = "codex_lifecycle.tests.rs"]
mod codex_tests;

#[cfg(test)]
#[path = "generate.tests.rs"]
mod tests;
