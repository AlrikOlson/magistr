use super::{MapEntryKind, MapEntryPoint};
use std::collections::BTreeSet;

pub(super) fn entry_points(
    files: &[String],
    mut read: impl FnMut(&str) -> Option<String>,
) -> Vec<MapEntryPoint> {
    let mut entries = Vec::new();
    if files.iter().any(|path| path == "package.json")
        && let Some(manifest) = read("package.json")
            .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
    {
        if let Some(main) = manifest.get("main").and_then(serde_json::Value::as_str) {
            entries.push(entry(main, MapEntryKind::PackageMain, None));
        }
        match manifest.get("bin") {
            Some(serde_json::Value::String(path)) => entries.push(entry(
                path,
                MapEntryKind::PackageBin,
                manifest.get("name").and_then(serde_json::Value::as_str),
            )),
            Some(serde_json::Value::Object(bins)) => {
                for (name, path) in bins {
                    if let Some(path) = path.as_str() {
                        entries.push(entry(path, MapEntryKind::PackageBin, Some(name)));
                    }
                }
            }
            _ => {}
        }
    }
    if files.iter().any(|path| path == "Cargo.toml")
        && let Some(manifest) = read("Cargo.toml").and_then(|body| body.parse::<toml::Table>().ok())
        && let Some(bins) = manifest.get("bin").and_then(toml::Value::as_array)
    {
        for bin in bins {
            let name = bin.get("name").and_then(toml::Value::as_str);
            let path = bin
                .get("path")
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .or_else(|| name.map(|name| format!("src/bin/{name}.rs")));
            if let Some(path) = path {
                entries.push(entry(&path, MapEntryKind::CargoBin, name));
            }
        }
    }
    for path in files {
        if path
            .strip_prefix("src/main.")
            .is_some_and(|extension| !extension.is_empty() && !extension.contains(['/', '.']))
        {
            entries.push(entry(path, MapEntryKind::Convention, None));
        }
    }
    let mut seen = BTreeSet::new();
    entries.retain(|entry| seen.insert(entry.path.clone()));
    entries.truncate(12);
    entries
}

fn entry(path: &str, kind: MapEntryKind, name: Option<&str>) -> MapEntryPoint {
    MapEntryPoint {
        path: path.strip_prefix("./").unwrap_or(path).to_owned(),
        kind,
        name: name.map(str::to_owned),
    }
}
