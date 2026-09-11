use serde::{Deserialize, Serialize};
use std::path::Path;

/// A manifest-derived command suggestion, never an execution authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedCommand {
    pub name: String,
    pub line: String,
    pub provenance: String,
}

impl DetectedCommand {
    #[must_use]
    pub fn detect(root: &Path) -> Vec<Self> {
        let mut commands = node_commands(root);
        if let Some(cargo) = super::cargo_layout::CargoLayout::read(root) {
            commands.extend(cargo.commands());
        }
        if root.join("go.mod").exists() {
            commands.extend([
                Self {
                    name: "go-test".into(),
                    line: "go test ./...".into(),
                    provenance: "go.mod".into(),
                },
                Self {
                    name: "go-build".into(),
                    line: "go build ./...".into(),
                    provenance: "go.mod".into(),
                },
            ]);
        }
        let ini = root.join("pytest.ini").exists();
        if ini
            || std::fs::read_to_string(root.join("pyproject.toml"))
                .is_ok_and(|body| body.contains("pytest"))
        {
            commands.push(Self {
                name: "pytest".into(),
                line: "pytest".into(),
                provenance: if ini { "pytest.ini" } else { "pyproject.toml" }.into(),
            });
        }
        let mut seen = std::collections::HashSet::new();
        commands.retain(|command| seen.insert(command.name.clone()));
        commands
    }

    /// Render suggestions for explicit init; this method writes no files.
    #[must_use]
    pub fn render_table(commands: &[Self]) -> String {
        let mut lines = vec![
            "# Seeded by `workbench-mcp init` from lockfile/manifest detection (SDD §9.6).".into(),
            "# Amend by hand: this table is run's allowlist and only a human may change it.".into(),
            "[commands]".into(),
        ];
        for command in commands {
            lines.push(format!(
                "{} = {} # {}",
                serde_json::Value::String(command.name.clone()),
                serde_json::Value::String(command.line.clone()),
                command.provenance
            ));
        }
        format!("{}\n", lines.join("\n"))
    }
}

fn node_commands(root: &Path) -> Vec<DetectedCommand> {
    let Some(package) = std::fs::read_to_string(root.join("package.json"))
        .ok()
        .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
    else {
        return vec![];
    };
    let (runner, provenance) = if root.join("bun.lock").exists() || root.join("bun.lockb").exists()
    {
        ("bun run", "bun.lock")
    } else if root.join("pnpm-lock.yaml").exists() {
        ("pnpm run", "pnpm-lock.yaml")
    } else if root.join("yarn.lock").exists() {
        ("yarn run", "yarn.lock")
    } else {
        ("npm run", "package-lock.json or default")
    };
    [
        "test",
        "build",
        "lint",
        "typecheck",
        "check",
        "format",
        "dev",
    ]
    .into_iter()
    .filter(|name| {
        package
            .get("scripts")
            .and_then(|scripts| scripts.get(name))
            .is_some_and(serde_json::Value::is_string)
    })
    .map(|name| DetectedCommand {
        name: name.into(),
        line: format!("{runner} {name}"),
        provenance: format!("package.json scripts.{name} + {provenance}"),
    })
    .collect()
}

#[cfg(test)]
#[path = "detected_command.tests.rs"]
mod tests;
