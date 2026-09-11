use super::{
    MapIndex, MapInput, MapPlugin, MapRow, WorkspaceQueries, budget, map_cargo::CargoMap,
    map_enrichment::MapEnrichment, map_entry_points::entry_points, map_tree::MapTree,
};
use crate::{
    contracts::{Freshness, WorkspaceResponse},
    file_content::FileContent,
    index::IndexStatus,
};

impl WorkspaceQueries<'_> {
    /// Live orientation with optional, revalidated native index enrichment.
    pub async fn map(&self, input: &MapInput) -> WorkspaceResponse<MapRow> {
        run(self, input)
    }
}

pub(super) fn run(q: &WorkspaceQueries<'_>, input: &MapInput) -> WorkspaceResponse<MapRow> {
    let admitted = match q.policy.check_read(input.path.replace('\\', "/")) {
        Ok(path) => path,
        Err(error) => return budget::failure(error),
    };
    let prefix = admitted
        .strip_prefix(&q.root)
        .expect("path admission keeps the workspace root")
        .to_string_lossy()
        .replace('\\', "/");
    let entries = match q.files() {
        Ok(entries) => entries,
        Err(error) => return budget::failure(error),
    };
    let files = entries
        .iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    let mut tree = MapTree::build(&entries, &prefix, input.depth.clamp(1, 16));
    let visible_paths = tree
        .rows
        .iter()
        .filter(|row| matches!(row, MapRow::File { .. }))
        .map(MapRow::path)
        .collect::<std::collections::BTreeSet<_>>();
    let visible = files
        .iter()
        .filter(|path| visible_paths.contains(path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let enrichment = MapEnrichment::fetch(q, &files, &visible, input.detail);
    enrichment.attach(&mut tree.rows, input.detail);
    let mut notes = enrichment.notes.clone();
    if tree.files == 0 && !prefix.is_empty() {
        notes.push(format!(
            "no files under {prefix:?}; is the path workspace-relative?"
        ));
    }
    let mut observed_freshness = enrichment.freshness;
    let mut read = |path: &str| match q.snapshot(path) {
        Ok(snapshot) => {
            if snapshot.freshness == Freshness::Cold {
                observed_freshness = Freshness::Cold;
            }
            match snapshot.content {
                FileContent::Text(text) => Some(text),
                FileContent::Binary(_) => None,
            }
        }
        Err(_) => None,
    };
    let detected = entry_points(&files, &mut read);
    let cargo = CargoMap::read(&files, &mut read)
        .map(|model| model.render(&prefix, enrichment.imports.as_ref()));
    let (cargo, mut crates) = if let Some((summary, rows, cargo_notes)) = cargo {
        notes.extend(cargo_notes);
        (Some(summary), rows)
    } else {
        (None, Vec::new())
    };
    let status = q.index.map(|index| index.status());
    let (observed_freshness, ttl_ms) = final_health(observed_freshness, status.as_ref());
    let index = status.as_ref().map(|status| MapIndex {
        tier: u8::from(status.freshness != Freshness::Cold),
        building: status.building,
        generation: status.generation,
        files_total: files.len(),
        files_indexed: status.files,
    });
    if tree.languages.get("rust").is_some_and(|count| *count > 0) && status.is_some() {
        notes.push("macro-generated items (derives, macro_rules!) are not in the symbol index: tree-sitter sees source, not expansion — use search mode:lexical or mode:references for them".into());
    }
    let plugins = match &q.state_dir {
        Some(state_dir) => {
            let (plugins, inventory_notes) = MapPlugin::inventory(state_dir);
            notes.extend(inventory_notes);
            plugins
        }
        None => {
            notes.push(
                "plugin inventory is unavailable in the native query library; no plugins are shown"
                    .into(),
            );
            Vec::new()
        }
    };
    // Repeated file validation notes describe one answer, not a log per file.
    let mut seen = std::collections::BTreeSet::new();
    notes.retain(|note| seen.insert(note.clone()));
    let overview = MapRow::Overview {
        path: if prefix.is_empty() {
            ".".into()
        } else {
            prefix
        },
        index,
        ttl_ms,
        files: tree.files,
        dirs: tree.dirs,
        languages: tree.languages,
        plugins,
        import_graph: enrichment.graph(),
        entry_points: detected,
        cargo,
    };
    let mut rows = vec![overview];
    rows.append(&mut crates);
    rows.extend(tree.rows);
    budget::success(
        rows,
        input.budget,
        observed_freshness,
        notes,
        |row| format!("path:{}", row.path()),
        vec![
            "lower depth".into(),
            "narrow path".into(),
            "detail: \"skeleton\"".into(),
            "raise budget".into(),
        ],
    )
}

fn final_health(observed: Freshness, status: Option<&IndexStatus>) -> (Freshness, u64) {
    let freshness = match (observed, status.map(|status| status.freshness)) {
        (Freshness::Cold, _) | (_, Some(Freshness::Cold) | None) => Freshness::Cold,
        (Freshness::Warming, _) | (_, Some(Freshness::Warming)) => Freshness::Warming,
        _ if status.is_some_and(|status| status.building) => Freshness::Warming,
        _ => Freshness::Warm,
    };
    (
        freshness,
        if freshness == Freshness::Warm {
            30_000
        } else {
            2_000
        },
    )
}

#[cfg(test)]
#[path = "map.tests.rs"]
mod tests;
