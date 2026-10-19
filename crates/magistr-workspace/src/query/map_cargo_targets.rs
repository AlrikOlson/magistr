use super::{MapCrateRole, MapEntryKind, MapEntryPoint};
use std::collections::BTreeSet;

pub(super) fn cargo_targets(
    dir: &str,
    manifest: &toml::Table,
    files: &[&str],
) -> (Vec<MapEntryPoint>, Vec<MapCrateRole>) {
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for (key, value) in manifest {
        let Some(kind) = target_kind(key) else {
            continue;
        };
        let tables = match value {
            toml::Value::Table(table) => vec![table],
            toml::Value::Array(values) => values.iter().filter_map(toml::Value::as_table).collect(),
            _ => Vec::new(),
        };
        for table in tables {
            let name = table
                .get("name")
                .and_then(toml::Value::as_str)
                .map(str::to_owned);
            let path = table
                .get("path")
                .and_then(toml::Value::as_str)
                .map(normalize)
                .or_else(|| {
                    if kind == MapEntryKind::Lib {
                        Some("src/lib.rs".into())
                    } else {
                        name.as_ref().map(|name| {
                            format!(
                                "{}/{name}.rs",
                                if key == "bin" {
                                    "src/bin".into()
                                } else {
                                    format!("{key}s")
                                }
                            )
                        })
                    }
                });
            if let Some(path) = path {
                push(
                    &mut entries,
                    &mut seen,
                    MapEntryPoint {
                        path: ws_path(dir, &path),
                        kind,
                        name,
                    },
                );
            }
        }
    }
    let mut sorted = files.to_vec();
    sorted.sort();
    for path in sorted {
        if let Some((kind, name)) = convention(path) {
            push(
                &mut entries,
                &mut seen,
                MapEntryPoint {
                    path: ws_path(dir, path),
                    kind,
                    name,
                },
            );
        }
    }
    let mut roles = Vec::new();
    if entries.iter().any(|entry| entry.kind == MapEntryKind::Lib) {
        let is_proc_macro = manifest
            .get("lib")
            .and_then(|lib| lib.get("proc-macro"))
            .and_then(toml::Value::as_bool)
            == Some(true);
        roles.push(if is_proc_macro {
            MapCrateRole::ProcMacro
        } else {
            MapCrateRole::Lib
        });
    }
    if entries.iter().any(|entry| entry.kind == MapEntryKind::Bin) {
        roles.push(MapCrateRole::Bin);
    }
    if entries
        .iter()
        .any(|entry| entry.kind == MapEntryKind::Build)
    {
        roles.push(MapCrateRole::Build);
    }
    (entries, roles)
}

fn push(entries: &mut Vec<MapEntryPoint>, seen: &mut BTreeSet<String>, entry: MapEntryPoint) {
    if seen.insert(entry.path.clone()) {
        entries.push(entry);
    }
}

fn target_kind(key: &str) -> Option<MapEntryKind> {
    Some(match key {
        "lib" => MapEntryKind::Lib,
        "bin" => MapEntryKind::Bin,
        "example" => MapEntryKind::Example,
        "test" => MapEntryKind::Test,
        "bench" => MapEntryKind::Bench,
        _ => return None,
    })
}

fn convention(path: &str) -> Option<(MapEntryKind, Option<String>)> {
    match path {
        "src/lib.rs" => return Some((MapEntryKind::Lib, None)),
        "src/main.rs" => return Some((MapEntryKind::Bin, None)),
        "build.rs" => return Some((MapEntryKind::Build, None)),
        _ => {}
    }
    let (rest, kind) = if let Some(rest) = path.strip_prefix("src/bin/") {
        (rest, MapEntryKind::Bin)
    } else if let Some(rest) = path.strip_prefix("examples/") {
        (rest, MapEntryKind::Example)
    } else if let Some(rest) = path.strip_prefix("tests/") {
        (rest, MapEntryKind::Test)
    } else {
        (path.strip_prefix("benches/")?, MapEntryKind::Bench)
    };
    let name = rest
        .strip_suffix("/main.rs")
        .or_else(|| rest.strip_suffix(".rs"))?;
    if name.is_empty() || name.contains('/') {
        return None;
    }
    Some((kind, Some(name.to_owned())))
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn ws_path(dir: &str, path: &str) -> String {
    if dir == "." {
        path.into()
    } else {
        format!("{dir}/{path}")
    }
}
