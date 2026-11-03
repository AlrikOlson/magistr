use crate::types::ProjectConfig;

use super::CheckSpec;

/// Resolve `check` (`check` | `test` | `lint` | `all` | a custom check name)
/// against the project config. `Err` carries the user-facing message that
/// names the valid checks.
///
/// On a polyglot project the archetypes are the truth: each standard check
/// resolves to one spec per distinct (command, directory) across the
/// archetypes — so every stack's check runs, and a workspace-wide command
/// named by eight archetypes runs once instead of being raced against
/// itself. A project without archetypes resolves to the top-level commands
/// exactly as before.
///
/// # Errors
/// An unknown check name, with the valid names listed.
pub fn resolve_checks(config: &ProjectConfig, check: &str) -> Result<Vec<CheckSpec>, String> {
    match check {
        "check" | "test" | "lint" => Ok(standard_specs(config, check)),
        "all" => {
            let mut specs = Vec::new();
            for g in ["check", "test", "lint"] {
                specs.extend(standard_specs(config, g));
            }
            consolidate_checks(&mut specs);
            // Standard + *required* custom checks, as the tool's own
            // description has always claimed. A check registered
            // `required: false` is one the project expects to fail; `all`
            // paying for it bought a wasted run and a FAIL row that read
            // like a failed iteration. It stays runnable by its own name.
            specs.extend(
                config
                    .custom_checks
                    .iter()
                    .filter(|cg| cg.required)
                    .map(|cg| CheckSpec {
                        name: cg.name.clone(),
                        command: cg.command.clone(),
                        required: cg.required,
                    }),
            );
            Ok(specs)
        }
        other => config
            .custom_checks
            .iter()
            .find(|cg| cg.name == other)
            .map(|cg| {
                vec![CheckSpec {
                    name: cg.name.clone(),
                    command: cg.command.clone(),
                    required: cg.required,
                }]
            })
            .ok_or_else(|| {
                let mut valid: Vec<&str> = vec!["check", "test", "lint", "all"];
                valid.extend(config.custom_checks.iter().map(|cg| cg.name.as_str()));
                format!(
                    "Unknown check '{other}'. Valid checks: {}",
                    valid.join(", ")
                )
            }),
    }
}

/// The specs for one standard check (`check` | `test` | `lint`).
///
/// Without archetypes: the top-level command, named after the check. With
/// archetypes, three rules decide, all shared with `magistr_format`'s
/// format-archetype-cwd fix: (1) an archetype naming the project's own
/// top-level command is running the *project's* check and runs at the project
/// root, whichever subdirectory the archetype lives in; (2) one command in
/// one directory is one run, however many archetypes name it; (3) a genuinely
/// different command in a subdirectory archetype runs in that directory —
/// spelled `cd <dir> && <cmd>` so a record of the command is a record of
/// where it ran. Specs carrying an archetype's own command are named
/// `{check}:{archetype id}` so their check records stay distinguishable.
fn standard_specs(config: &ProjectConfig, check: &str) -> Vec<CheckSpec> {
    let qg = &config.checks;
    let top = match check {
        "check" => &qg.check,
        "test" => &qg.test,
        _ => &qg.lint,
    };
    let top_spec = || CheckSpec {
        name: check.to_owned(),
        command: top.clone(),
        required: true,
    };
    if config.archetypes.is_empty() {
        return vec![top_spec()];
    }

    let mut specs: Vec<CheckSpec> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for arch in &config.archetypes {
        let raw = match check {
            "check" => &arch.checks.check,
            "test" => &arch.checks.test,
            _ => &arch.checks.lint,
        };
        if raw.trim().is_empty() {
            specs.push(CheckSpec {
                name: format!("{check}:{}", arch.id),
                command: String::new(),
                required: true,
            });
            continue;
        }
        let root = arch.root.to_string_lossy();
        let is_project_command = raw == top;
        let dir = if is_project_command || root.is_empty() || root == "." {
            String::new()
        } else {
            root.to_string()
        };
        if !seen.insert((raw.clone(), dir.clone())) {
            continue;
        }
        let name = if is_project_command {
            check.to_owned()
        } else {
            format!("{check}:{}", arch.id)
        };
        let command = if dir.is_empty() {
            raw.clone()
        } else {
            format!("cd {} && {raw}", shell_escape::escape(dir.into()))
        };
        specs.push(CheckSpec {
            name,
            command,
            required: true,
        });
    }
    specs
}

/// Consolidate only commands whose Cargo targets and options prove coverage.
pub fn consolidate_checks(specs: &mut Vec<CheckSpec>) {
    // Clippy already type-checks the identical Cargo selection. Drop
    // only proven duplicates; different targets/features/cwds still run.
    let covered: Vec<_> = specs
        .iter()
        .filter(|spec| spec.name == "check" || spec.name.starts_with("check:"))
        .filter(|check| {
            specs
                .iter()
                .any(|lint| clippy_covers(&check.command, &lint.command))
        })
        .map(|spec| spec.name.clone())
        .collect();
    if !covered.is_empty() {
        specs.retain(|spec| !covered.contains(&spec.name));
        specs.sort_by_key(|spec| !spec.name.starts_with("lint"));
    }
}

/// Whether running this Clippy command also proves this exact check selection.
pub fn clippy_covers(check: &str, lint: &str) -> bool {
    // Splitting keeps a literal cd prefix as part of the coverage identity.
    // Unknown shell syntax refuses consolidation rather than guessing.
    fn selection(command: &str, verb: &str) -> Option<Vec<String>> {
        if command
            .chars()
            .any(|ch| matches!(ch, ';' | '|' | '\n' | '`' | '$' | '>' | '<'))
        {
            return None;
        }
        let mut words: Vec<_> = command.split_whitespace().map(str::to_owned).collect();
        let cargo = words.iter().position(|word| word == "cargo")?;
        if words.get(cargo + 1)? != verb {
            return None;
        }
        if words[..cargo].contains(&"cargo".into()) {
            return None;
        }
        if let Some(end) = words.iter().position(|word| word == "--") {
            words.truncate(end);
        }
        words[cargo + 1] = "verify".into();
        Some(words)
    }
    matches!((selection(check, "check"), selection(lint, "clippy")), (Some(a), Some(b)) if a == b)
}

#[cfg(test)]
#[path = "resolve_checks.tests.rs"]
mod tests;
