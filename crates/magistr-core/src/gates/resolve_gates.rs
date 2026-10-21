use crate::types::ProjectConfig;

use super::GateSpec;

/// Resolve `gate` (`check` | `test` | `lint` | `all` | a custom gate name)
/// against the project config. `Err` carries the user-facing message that
/// names the valid gates.
///
/// On a polyglot project the archetypes are the truth: each standard gate
/// resolves to one spec per distinct (command, directory) across the
/// archetypes — so every stack's gate runs, and a workspace-wide command
/// named by eight archetypes runs once instead of being raced against
/// itself. A project without archetypes resolves to the top-level commands
/// exactly as before.
///
/// # Errors
/// An unknown gate name, with the valid names listed.
pub fn resolve_gates(config: &ProjectConfig, gate: &str) -> Result<Vec<GateSpec>, String> {
    match gate {
        "check" | "test" | "lint" => Ok(standard_specs(config, gate)),
        "all" => {
            let mut specs = Vec::new();
            for g in ["check", "test", "lint"] {
                specs.extend(standard_specs(config, g));
            }
            consolidate_gates(&mut specs);
            // Standard + *required* custom gates, as the tool's own
            // description has always claimed. A gate registered
            // `required: false` is one the project expects to fail; `all`
            // paying for it bought a wasted run and a FAIL row that read
            // like a failed iteration. It stays runnable by its own name.
            specs.extend(
                config
                    .custom_gates
                    .iter()
                    .filter(|cg| cg.required)
                    .map(|cg| GateSpec {
                        name: cg.name.clone(),
                        command: cg.command.clone(),
                        required: cg.required,
                    }),
            );
            Ok(specs)
        }
        other => config
            .custom_gates
            .iter()
            .find(|cg| cg.name == other)
            .map(|cg| {
                vec![GateSpec {
                    name: cg.name.clone(),
                    command: cg.command.clone(),
                    required: cg.required,
                }]
            })
            .ok_or_else(|| {
                let mut valid: Vec<&str> = vec!["check", "test", "lint", "all"];
                valid.extend(config.custom_gates.iter().map(|cg| cg.name.as_str()));
                format!("Unknown gate '{other}'. Valid gates: {}", valid.join(", "))
            }),
    }
}

/// The specs for one standard gate (`check` | `test` | `lint`).
///
/// Without archetypes: the top-level command, named after the gate. With
/// archetypes, three rules decide, all shared with `magistr_format`'s
/// format-archetype-cwd fix: (1) an archetype naming the project's own
/// top-level command is running the *project's* gate and runs at the project
/// root, whichever subdirectory the archetype lives in; (2) one command in
/// one directory is one run, however many archetypes name it; (3) a genuinely
/// different command in a subdirectory archetype runs in that directory —
/// spelled `cd <dir> && <cmd>` so a record of the command is a record of
/// where it ran. Specs carrying an archetype's own command are named
/// `{gate}:{archetype id}` so their check records stay distinguishable.
fn standard_specs(config: &ProjectConfig, gate: &str) -> Vec<GateSpec> {
    let qg = &config.quality_gate;
    let top = match gate {
        "check" => &qg.check,
        "test" => &qg.test,
        _ => &qg.lint,
    };
    let top_spec = || GateSpec {
        name: gate.to_owned(),
        command: top.clone(),
        required: true,
    };
    if config.archetypes.is_empty() {
        return vec![top_spec()];
    }

    let mut specs: Vec<GateSpec> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for arch in &config.archetypes {
        let raw = match gate {
            "check" => &arch.quality_gate.check,
            "test" => &arch.quality_gate.test,
            _ => &arch.quality_gate.lint,
        };
        if raw.trim().is_empty() {
            specs.push(GateSpec {
                name: format!("{gate}:{}", arch.id),
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
            gate.to_owned()
        } else {
            format!("{gate}:{}", arch.id)
        };
        let command = if dir.is_empty() {
            raw.clone()
        } else {
            format!("cd {} && {raw}", shell_escape::escape(dir.into()))
        };
        specs.push(GateSpec {
            name,
            command,
            required: true,
        });
    }
    specs
}

/// Consolidate only commands whose Cargo targets and options prove coverage.
pub fn consolidate_gates(specs: &mut Vec<GateSpec>) {
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
#[path = "resolve_gates.tests.rs"]
mod tests;
