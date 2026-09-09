use std::collections::HashSet;
use std::path::Path;

/// Parsed manifest data extracted from project config files.
///
/// Fields are optional — parsing failures produce empty sets, not errors.
/// Detection functions use this for precise dependency/script lookups instead
/// of raw string-contains on file contents.
#[derive(Debug, Clone, Default)]
pub(crate) struct ManifestData {
    /// Combined dependencies (all sections).
    pub dependencies: HashSet<String>,
    /// npm scripts (package.json "scripts" section).
    pub scripts: HashSet<String>,
    /// Python tool sections present in pyproject.toml (e.g., "ruff", "pytest", "mypy").
    pub python_tools: HashSet<String>,
    /// Cargo features enabled in the default profile.
    pub cargo_features: HashSet<String>,
}

impl ManifestData {
    /// Check if a dependency exists (case-insensitive match on the name).
    pub fn has_dep(&self, name: &str) -> bool {
        self.dependencies.contains(name)
    }

    /// Check if a script exists by name.
    #[allow(dead_code)] // kept for future detection rules
    pub fn has_script(&self, name: &str) -> bool {
        self.scripts.contains(name)
    }

    /// Check if a Python tool section exists.
    pub fn has_python_tool(&self, name: &str) -> bool {
        self.python_tools.contains(name)
    }
}

/// Parse all relevant manifest files in a directory.
///
/// Tries package.json, Cargo.toml, pyproject.toml, go.mod, Gemfile, etc.
/// Never fails — returns an empty ManifestData if nothing can be parsed.
pub(crate) fn parse_manifests(dir: &Path) -> ManifestData {
    let mut data = ManifestData::default();

    parse_package_json(dir, &mut data);
    parse_cargo_toml(dir, &mut data);
    parse_pyproject_toml(dir, &mut data);
    parse_go_mod(dir, &mut data);
    parse_gemfile(dir, &mut data);
    parse_mix_exs(dir, &mut data);

    data
}

fn parse_package_json(dir: &Path, data: &mut ManifestData) {
    let path = dir.join("package.json");
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let json: serde_json::Value = match serde_json::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Extract dependencies from all dep sections
    for section in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        if let Some(deps) = json.get(section).and_then(|v| v.as_object()) {
            for key in deps.keys() {
                data.dependencies.insert(key.clone());
            }
        }
    }

    // Extract script names
    if let Some(scripts) = json.get("scripts").and_then(|v| v.as_object()) {
        for key in scripts.keys() {
            data.scripts.insert(key.clone());
        }
    }
}

fn parse_cargo_toml(dir: &Path, data: &mut ManifestData) {
    let path = dir.join("Cargo.toml");
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let toml: toml::Value = match toml::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Extract dependency names from [dependencies], [dev-dependencies], [build-dependencies]
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(deps) = toml.get(section).and_then(|v| v.as_table()) {
            for key in deps.keys() {
                data.dependencies.insert(key.clone());
            }
        }
    }

    // Extract default features
    if let Some(features) = toml.get("features").and_then(|v| v.as_table()) {
        for key in features.keys() {
            data.cargo_features.insert(key.clone());
        }
    }
}

fn parse_pyproject_toml(dir: &Path, data: &mut ManifestData) {
    let path = dir.join("pyproject.toml");
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let toml: toml::Value = match toml::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Extract dependencies from [project.dependencies]
    if let Some(deps) = toml
        .get("project")
        .and_then(|v| v.get("dependencies"))
        .and_then(|v| v.as_array())
    {
        for dep in deps {
            if let Some(s) = dep.as_str() {
                // PEP 508: "fastapi>=0.100" → "fastapi"
                let name = s
                    .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                    .next()
                    .unwrap_or(s)
                    .to_lowercase();
                if !name.is_empty() {
                    data.dependencies.insert(name);
                }
            }
        }
    }

    // Extract optional dependencies
    if let Some(optional) = toml
        .get("project")
        .and_then(|v| v.get("optional-dependencies"))
        .and_then(|v| v.as_table())
    {
        for deps in optional.values() {
            if let Some(arr) = deps.as_array() {
                for dep in arr {
                    if let Some(s) = dep.as_str() {
                        let name = s
                            .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                            .next()
                            .unwrap_or(s)
                            .to_lowercase();
                        if !name.is_empty() {
                            data.dependencies.insert(name);
                        }
                    }
                }
            }
        }
    }

    // Extract [tool.*] sections
    if let Some(tool) = toml.get("tool").and_then(|v| v.as_table()) {
        for key in tool.keys() {
            data.python_tools.insert(key.clone());
        }
    }
}

fn parse_go_mod(dir: &Path, data: &mut ManifestData) {
    let path = dir.join("go.mod");
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Simple line-by-line parsing of require block
    let mut in_require = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("require (") || trimmed == "require (" {
            in_require = true;
            continue;
        }
        if trimmed == ")" {
            in_require = false;
            continue;
        }
        if in_require {
            // "github.com/gin-gonic/gin v1.9.1" → extract last path segment
            if let Some(module) = trimmed.split_whitespace().next() {
                // Store both the full path and the last segment
                data.dependencies.insert(module.to_string());
                if let Some(last) = module.rsplit('/').next() {
                    data.dependencies.insert(last.to_string());
                }
            }
        }
        // Single-line require: `require github.com/foo/bar v1.0.0`
        if trimmed.starts_with("require ")
            && !trimmed.contains('(')
            && let Some(module) = trimmed
                .strip_prefix("require ")
                .and_then(|s| s.split_whitespace().next())
        {
            data.dependencies.insert(module.to_string());
            if let Some(last) = module.rsplit('/').next() {
                data.dependencies.insert(last.to_string());
            }
        }
    }
}

fn parse_gemfile(dir: &Path, data: &mut ManifestData) {
    let path = dir.join("Gemfile");
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Extract gem names from `gem "name"` or `gem 'name'` lines
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("gem ") {
            let rest = rest.trim();
            let name = if rest.starts_with('"') {
                rest.trim_start_matches('"').split('"').next()
            } else if rest.starts_with('\'') {
                rest.trim_start_matches('\'').split('\'').next()
            } else {
                None
            };
            if let Some(n) = name {
                data.dependencies.insert(n.to_string());
            }
        }
    }
}

fn parse_mix_exs(dir: &Path, data: &mut ManifestData) {
    let path = dir.join("mix.exs");
    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Extract dep atoms from `{:name, ...}` patterns
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(start) = trimmed.find("{:") {
            let rest = &trimmed[start + 2..];
            if let Some(end) = rest.find([',', '}']) {
                let name = &rest[..end];
                data.dependencies.insert(name.to_string());
            }
        }
    }
}

#[cfg(test)]
#[path = "config_parser.tests.rs"]
mod tests;
