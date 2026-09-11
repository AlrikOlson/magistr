use super::DetectedCommand;
use std::collections::BTreeMap;
use std::path::Path;

pub(super) struct CargoLayout {
    workspace: bool,
    members: BTreeMap<String, String>,
    skipped: Vec<String>,
    nextest: bool,
    clippy: bool,
}

impl CargoLayout {
    pub(super) fn read(root: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
        let manifest = content.parse::<toml::Table>().unwrap_or_default();
        let workspace = manifest.get("workspace").and_then(toml::Value::as_table);
        let toolchain = std::fs::read_to_string(root.join("rust-toolchain.toml"))
            .ok()
            .and_then(|body| body.parse::<toml::Table>().ok());
        let clippy = toolchain
            .as_ref()
            .and_then(|doc| doc.get("toolchain"))
            .and_then(|table| table.get("components"))
            .and_then(toml::Value::as_array)
            .is_some_and(|components| {
                components
                    .iter()
                    .any(|value| value.as_str() == Some("clippy"))
            });
        let mut layout = Self {
            workspace: workspace.is_some(),
            members: BTreeMap::new(),
            skipped: vec![],
            nextest: root.join(".config/nextest.toml").exists(),
            clippy,
        };
        if let Some(workspace) = workspace {
            layout.read_members(root, &manifest, workspace);
        }
        Some(layout)
    }

    fn read_members(&mut self, root: &Path, manifest: &toml::Table, workspace: &toml::Table) {
        let excluded = strings(workspace.get("exclude"));
        if let Some(name) = package_name(manifest) {
            self.members.insert(name, ".".into());
        }
        for raw in strings(workspace.get("members")) {
            let entry = normalize(&raw);
            let candidates = if let Some(dir) = entry.strip_suffix("/*")
                && !has_glob(dir)
            {
                let mut children = std::fs::read_dir(root.join(dir))
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .filter_map(|child| child.file_name().into_string().ok())
                    .collect::<Vec<_>>();
                children.sort();
                children
                    .into_iter()
                    .map(|name| format!("{dir}/{name}"))
                    .collect::<Vec<_>>()
            } else if has_glob(&entry) {
                self.skipped.push(raw);
                continue;
            } else {
                vec![entry]
            };
            for candidate in candidates {
                if excluded
                    .iter()
                    .any(|exclude| normalize(exclude) == candidate)
                {
                    continue;
                }
                let member = std::fs::read_to_string(root.join(&candidate).join("Cargo.toml"))
                    .ok()
                    .and_then(|body| body.parse::<toml::Table>().ok());
                if let Some(name) = member.as_ref().and_then(package_name) {
                    self.members.entry(name).or_insert(candidate);
                }
            }
        }
    }

    pub(super) fn commands(&self) -> Vec<DetectedCommand> {
        let scope = if self.workspace { " --workspace" } else { "" };
        let manifest = if self.workspace {
            "Cargo.toml [workspace]"
        } else {
            "Cargo.toml"
        };
        let unexpanded = if self.skipped.is_empty() {
            String::new()
        } else {
            format!(
                "; members {} not expanded (only <dir>/* globs are)",
                self.skipped
                    .iter()
                    .map(|entry| format!("`{entry}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let mut commands = vec![
            detected(
                "cargo-test",
                if self.workspace {
                    "cargo test --workspace --no-fail-fast".into()
                } else {
                    "cargo test".into()
                },
                format!("{manifest}{unexpanded}"),
            ),
            detected(
                "cargo-build",
                format!("cargo build{scope}"),
                manifest.into(),
            ),
        ];
        if self.nextest {
            commands.push(detected(
                "cargo-nextest",
                format!("cargo nextest run{scope} --no-fail-fast"),
                ".config/nextest.toml".into(),
            ));
        }
        if self.clippy {
            commands.push(detected(
                "cargo-clippy",
                format!("cargo clippy{scope} --all-targets"),
                "rust-toolchain.toml components".into(),
            ));
        }
        for (index, (name, dir)) in self.members.iter().take(32).enumerate() {
            let note = if index == 31 && self.members.len() > 32 {
                format!(
                    "; {} more members not seeded (cap 32, add by hand)",
                    self.members.len() - 32
                )
            } else {
                String::new()
            };
            commands.push(detected(
                &format!("cargo-test-{name}"),
                format!("cargo test -p {name}"),
                format!("{dir}/Cargo.toml [package]{note}"),
            ));
        }
        commands
    }
}

fn detected(name: &str, line: String, provenance: String) -> DetectedCommand {
    DetectedCommand {
        name: name.into(),
        line,
        provenance,
    }
}

fn strings(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn package_name(doc: &toml::Table) -> Option<String> {
    doc.get("package")?.get("name")?.as_str().map(str::to_owned)
}

fn normalize(entry: &str) -> String {
    entry.replace('\\', "/").trim_end_matches('/').to_owned()
}
fn has_glob(entry: &str) -> bool {
    entry.contains(['*', '?', '['])
}
