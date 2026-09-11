use super::{
    MapCargoSummary, MapDependency, MapDependent, MapRow, map_cargo_targets::cargo_targets,
};
use std::collections::{BTreeMap, BTreeSet};

/// Manifest-only Cargo model; live import facts are attached only when complete.
pub(super) struct CargoMap {
    kind: String,
    members: Vec<Member>,
    notes: Vec<String>,
}

struct Member {
    name: String,
    dir: String,
    use_name: String,
    manifest: toml::Table,
    files: Vec<String>,
    dependencies: Vec<MapDependency>,
    external_dependencies: usize,
}

struct RawDependency {
    key: String,
    kind: Option<String>,
    path: Option<String>,
    package: Option<String>,
    workspace: bool,
}

impl CargoMap {
    const MANIFEST_LIMIT: usize = 256;
    const ROW_LIMIT: usize = 24;
    const ENTRY_LIMIT: usize = 8;
    const FEATURE_LIMIT: usize = 16;

    pub fn read(files: &[String], mut read: impl FnMut(&str) -> Option<String>) -> Option<Self> {
        let present = files.iter().map(String::as_str).collect::<BTreeSet<_>>();
        if !present.contains("Cargo.toml") {
            return None;
        }
        let root = read("Cargo.toml")?.parse::<toml::Table>().ok()?;
        let workspace = root.get("workspace").and_then(toml::Value::as_table);
        let mut notes = Vec::new();
        let mut dirs = Vec::new();
        let mut seen = BTreeSet::new();
        if let Some(workspace) = workspace {
            let excluded = strings(workspace.get("exclude"))
                .into_iter()
                .map(|path| normalize(&path))
                .collect::<BTreeSet<_>>();
            let mut skipped = Vec::new();
            for raw in strings(workspace.get("members")) {
                let entry = normalize(&raw);
                let candidates =
                    if let Some(base) = entry.strip_suffix("/*").filter(|base| !has_glob(base)) {
                        files
                            .iter()
                            .filter_map(|path| {
                                let rest = path.strip_prefix(base)?.strip_prefix('/')?;
                                let child = rest.strip_suffix("/Cargo.toml")?;
                                if child.contains('/') || child.is_empty() {
                                    return None;
                                }
                                Some(format!("{base}/{child}"))
                            })
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect::<Vec<_>>()
                    } else if has_glob(&entry) {
                        skipped.push(raw);
                        continue;
                    } else if present.contains(format!("{entry}/Cargo.toml").as_str()) {
                        vec![entry]
                    } else {
                        Vec::new()
                    };
                for dir in candidates {
                    if !excluded.contains(&dir) && seen.insert(dir.clone()) {
                        dirs.push(dir);
                    }
                }
            }
            if !skipped.is_empty() {
                notes.push(format!(
                    "Cargo members {} not expanded (only <dir>/* globs are)",
                    skipped
                        .iter()
                        .map(|value| format!("`{value}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        let mut members = Vec::new();
        if let Some(member) = Member::new(".".into(), root.clone()) {
            members.push(member);
        }
        for dir in dirs.iter().take(Self::MANIFEST_LIMIT) {
            if let Some(manifest) =
                read(&format!("{dir}/Cargo.toml")).and_then(|body| body.parse::<toml::Table>().ok())
                && let Some(member) = Member::new(dir.clone(), manifest)
            {
                members.push(member);
            }
        }
        if dirs.len() > Self::MANIFEST_LIMIT {
            notes.push(format!(
                "{} Cargo member manifests beyond the {} read; those members are not shown",
                dirs.len() - Self::MANIFEST_LIMIT,
                Self::MANIFEST_LIMIT
            ));
        }
        if members.is_empty() {
            return None;
        }
        let member_dirs = members
            .iter()
            .map(|member| member.dir.clone())
            .collect::<Vec<_>>();
        for file in files {
            if let Some(owner) = owner(file, &member_dirs)
                && let Some(member) = members.iter_mut().find(|member| member.dir == owner)
            {
                member.files.push(if member.dir == "." {
                    file.clone()
                } else {
                    file[member.dir.len() + 1..].to_owned()
                });
            }
        }
        resolve_dependencies(&mut members, &root);
        Some(Self {
            kind: if workspace.is_some() {
                "workspace"
            } else {
                "crate"
            }
            .into(),
            members,
            notes,
        })
    }

    pub fn render(
        self,
        prefix: &str,
        imports: Option<&BTreeMap<String, Vec<String>>>,
    ) -> (MapCargoSummary, Vec<MapRow>, Vec<String>) {
        let mut dependents = BTreeMap::<&str, usize>::new();
        let mut edge_count = 0;
        for member in &self.members {
            for dep in &member.dependencies {
                *dependents.entry(&dep.name).or_default() += 1;
                edge_count += 1;
            }
        }
        let member_dirs = self
            .members
            .iter()
            .map(|member| member.dir.clone())
            .collect::<Vec<_>>();
        let mut unused = 0;
        let mut rows = self
            .members
            .iter()
            .map(|member| {
                let mut dependencies = member.dependencies.clone();
                if let Some(imports) = imports {
                    for dependency in &mut dependencies {
                        let use_name = self
                            .members
                            .iter()
                            .find(|candidate| candidate.name == dependency.name)
                            .map(|candidate| candidate.use_name.as_str())
                            .unwrap_or("");
                        let count = imports
                            .iter()
                            .filter(|(path, targets)| {
                                owner(path, &member_dirs) == Some(member.dir.as_str())
                                    && targets.iter().any(|target| {
                                        target == use_name
                                            || target
                                                .strip_prefix(use_name)
                                                .is_some_and(|suffix| suffix.starts_with("::"))
                                    })
                            })
                            .count();
                        dependency.files = Some(count);
                        if count == 0 {
                            unused += 1;
                        }
                    }
                }
                let files = member.files.iter().map(String::as_str).collect::<Vec<_>>();
                let (mut entry_points, roles) =
                    cargo_targets(&member.dir, &member.manifest, &files);
                let more_entry_points = positive_difference(entry_points.len(), Self::ENTRY_LIMIT);
                entry_points.truncate(Self::ENTRY_LIMIT);
                let mut features = member
                    .manifest
                    .get("features")
                    .and_then(toml::Value::as_table)
                    .map(|table| table.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default();
                if let Some(index) = features.iter().position(|name| name == "default") {
                    let default = features.remove(index);
                    features.insert(0, default);
                }
                let more_features = positive_difference(features.len(), Self::FEATURE_LIMIT);
                features.truncate(Self::FEATURE_LIMIT);
                MapRow::Crate {
                    path: member.dir.clone(),
                    name: member.name.clone(),
                    roles,
                    edition: member
                        .manifest
                        .get("package")
                        .and_then(|package| package.get("edition"))
                        .and_then(toml::Value::as_str)
                        .map(str::to_owned),
                    entry_points,
                    more_entry_points,
                    features,
                    more_features,
                    dependencies,
                    dependents: dependents
                        .get(member.name.as_str())
                        .copied()
                        .unwrap_or_default(),
                    external_dependencies: member.external_dependencies,
                }
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| match (left, right) {
            (
                MapRow::Crate {
                    name: left_name,
                    dependents: left_count,
                    ..
                },
                MapRow::Crate {
                    name: right_name,
                    dependents: right_count,
                    ..
                },
            ) => right_count
                .cmp(left_count)
                .then_with(|| left_name.cmp(right_name)),
            _ => std::cmp::Ordering::Equal,
        });
        let top = rows
            .iter()
            .take(5)
            .filter_map(|row| match row {
                MapRow::Crate {
                    name, dependents, ..
                } => Some(MapDependent {
                    name: name.clone(),
                    dependents: *dependents,
                }),
                _ => None,
            })
            .collect();
        let members = rows.len();
        rows.retain(|row| {
            prefix.is_empty()
                || row.path() == prefix
                || row
                    .path()
                    .strip_prefix(prefix)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        });
        let mut notes = self.notes;
        if rows.len() > Self::ROW_LIMIT {
            notes.push(format!("{} workspace members beyond the {} crate rows shown; map path:<member dir> lists a member", rows.len() - Self::ROW_LIMIT, Self::ROW_LIMIT));
        }
        rows.truncate(Self::ROW_LIMIT);
        let summary = MapCargoSummary {
            kind: self.kind,
            members,
            edges: edge_count,
            omitted: positive_difference(members, rows.len()),
            unused_deps: imports.map(|_| unused),
            top,
        };
        (summary, rows, notes)
    }
}

impl Member {
    fn new(dir: String, manifest: toml::Table) -> Option<Self> {
        let name = manifest.get("package")?.get("name")?.as_str()?.to_owned();
        let use_name = manifest
            .get("lib")
            .and_then(|lib| lib.get("name"))
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| name.replace('-', "_"));
        Some(Self {
            name,
            dir,
            use_name,
            manifest,
            files: Vec::new(),
            dependencies: Vec::new(),
            external_dependencies: 0,
        })
    }
}

fn resolve_dependencies(members: &mut [Member], root: &toml::Table) {
    let by_dir = members
        .iter()
        .map(|member| (member.dir.clone(), member.name.clone()))
        .collect::<BTreeMap<_, _>>();
    let names = members
        .iter()
        .map(|member| member.name.clone())
        .collect::<BTreeSet<_>>();
    let workspace_deps = root
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table);
    for member in members {
        let mut seen = BTreeSet::new();
        for dependency in dependencies(&member.manifest) {
            let target = if let Some(path) = &dependency.path {
                by_dir.get(&resolve(&member.dir, path)).cloned()
            } else if dependency.workspace && workspace_deps.is_some() {
                let declared = workspace_deps.and_then(|deps| deps.get(&dependency.key));
                if let Some(path) = declared
                    .and_then(|value| value.get("path"))
                    .and_then(toml::Value::as_str)
                {
                    by_dir.get(&resolve(".", path)).cloned()
                } else {
                    let name = declared
                        .and_then(|value| value.get("package"))
                        .and_then(toml::Value::as_str)
                        .unwrap_or(&dependency.key);
                    names.get(name).cloned()
                }
            } else {
                names
                    .get(dependency.package.as_ref().unwrap_or(&dependency.key))
                    .cloned()
            };
            let Some(name) = target else {
                member.external_dependencies += 1;
                continue;
            };
            if name == member.name || !seen.insert((name.clone(), dependency.kind.clone())) {
                continue;
            }
            member.dependencies.push(MapDependency {
                name,
                kind: dependency.kind,
                files: None,
            });
        }
    }
}

fn dependencies(manifest: &toml::Table) -> Vec<RawDependency> {
    let mut result = Vec::new();
    for (key, value) in manifest {
        if key == "target" {
            if let Some(targets) = value.as_table() {
                for target in targets.values().filter_map(toml::Value::as_table) {
                    result.extend(dependencies(target));
                }
            }
            continue;
        }
        let kind = match key.as_str() {
            "dependencies" => None,
            "dev-dependencies" => Some("dev".to_owned()),
            "build-dependencies" => Some("build".to_owned()),
            _ => continue,
        };
        if let Some(table) = value.as_table() {
            for (name, value) in table {
                result.push(RawDependency {
                    key: name.clone(),
                    kind: kind.clone(),
                    path: value
                        .get("path")
                        .and_then(toml::Value::as_str)
                        .map(str::to_owned),
                    package: value
                        .get("package")
                        .and_then(toml::Value::as_str)
                        .map(str::to_owned),
                    workspace: value.get("workspace").and_then(toml::Value::as_bool) == Some(true),
                });
            }
        }
    }
    result
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
fn has_glob(value: &str) -> bool {
    value.contains(['*', '?', '['])
}
fn normalize(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}
fn resolve(dir: &str, path: &str) -> String {
    let mut parts = if dir == "." {
        Vec::new()
    } else {
        dir.split('/').map(str::to_owned).collect::<Vec<_>>()
    };
    for part in normalize(path).split('/') {
        if part == ".." {
            parts.pop();
        } else if !part.is_empty() {
            parts.push(part.to_owned());
        }
    }
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}
fn owner<'a>(path: &str, dirs: &'a [String]) -> Option<&'a str> {
    dirs.iter()
        .filter(|dir| {
            dir.as_str() != "."
                && path
                    .strip_prefix(dir.as_str())
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
        .max_by_key(|dir| dir.len())
        .map(String::as_str)
        .or_else(|| {
            dirs.iter()
                .find(|dir| dir.as_str() == ".")
                .map(String::as_str)
        })
}
fn positive_difference(value: usize, kept: usize) -> Option<usize> {
    value.checked_sub(kept).filter(|difference| *difference > 0)
}

#[cfg(test)]
#[path = "map_cargo.tests.rs"]
mod tests;
