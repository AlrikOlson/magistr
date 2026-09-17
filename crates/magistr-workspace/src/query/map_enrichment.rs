use super::{MapDetail, MapImportGraph, MapImportModule, MapRow, MapSymbol, WorkspaceQueries};
use crate::{contracts::Freshness, worker::WorkerSymbol};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct MapEnrichment {
    pub freshness: Freshness,
    pub notes: Vec<String>,
    pub imports: Option<BTreeMap<String, Vec<String>>>,
    symbols: BTreeMap<String, Vec<WorkerSymbol>>,
}

impl MapEnrichment {
    const FILE_LIMIT: usize = 512;

    pub fn fetch(
        q: &WorkspaceQueries<'_>,
        files: &[String],
        visible: &[String],
        detail: MapDetail,
    ) -> Self {
        let mut result = Self {
            freshness: q.freshness(),
            notes: Vec::new(),
            imports: None,
            symbols: BTreeMap::new(),
        };
        let Some(index) = q.index else {
            if detail != MapDetail::Skeleton {
                result.notes.push("skeleton from the live filesystem; symbol outlines appear once the Tier-1 index is up".into());
            }
            return result;
        };
        let status = index.status();
        if status.freshness == Freshness::Cold {
            if detail != MapDetail::Skeleton {
                result.notes.push("skeleton from the live filesystem; symbol outlines appear once the Tier-1 index is up".into());
            }
            return result;
        }
        let mut graph_complete = true;
        let mut paths = match Self::graph_inventory(files) {
            Ok(inventory) => inventory,
            Err(note) => {
                graph_complete = false;
                result.notes.push(note);
                BTreeSet::new()
            }
        };
        if detail != MapDetail::Skeleton {
            paths.extend(visible.iter().take(Self::FILE_LIMIT).cloned());
            if visible.len() > Self::FILE_LIMIT {
                result.notes.push(format!("outlines cover the first {} of {} visible files; narrow path or lower depth for full coverage", Self::FILE_LIMIT, visible.len()));
            }
        }
        let mut imports = BTreeMap::new();
        let mut incomplete = false;
        for path in paths {
            match q.index_file(&path) {
                Ok(query) => {
                    result.freshness = worst(result.freshness, query.freshness);
                    // A single graph cannot mix generations. Rows already carry
                    // live hashes, but a moving cache cannot establish absence.
                    if query.generation != status.generation
                        || query.truncated
                        || query.rows.is_empty()
                    {
                        graph_complete = false;
                        incomplete = true;
                    }
                    result.notes.extend(query.notes);
                    for file in query.rows {
                        imports.insert(file.path.clone(), file.imports);
                        result.symbols.insert(file.path, file.symbols);
                    }
                }
                Err(error) => {
                    result.freshness = worst(result.freshness, Freshness::Cold);
                    graph_complete = false;
                    incomplete = true;
                    result.notes.push(format!(
                        "index enrichment unavailable for {path}: {}",
                        error.message
                    ));
                }
            }
        }
        let final_status = index.status();
        result.freshness = worst(result.freshness, final_status.freshness);
        if final_status.generation != status.generation {
            graph_complete = false;
            incomplete = true;
            result.freshness = worst(result.freshness, Freshness::Warming);
        }
        if incomplete {
            result.notes.push("import-graph summary and use counts unavailable: the index could not validate the complete source inventory".into());
        }
        if graph_complete {
            result.imports = Some(imports);
        }
        result
    }

    /// The import-summary ceiling, decided from the live inventory before any
    /// index read: the graph validates the complete inventory or nothing. Over
    /// the ceiling there is no inventory at all, so no fan-in/fan-out or use
    /// count can ever be computed from a subset.
    fn graph_inventory(files: &[String]) -> Result<BTreeSet<String>, String> {
        if files.len() > Self::FILE_LIMIT {
            return Err(format!(
                "import-graph summary and use counts unavailable: workspace has {} files, over the {}-file validation ceiling; no partial counts are reported",
                files.len(),
                Self::FILE_LIMIT
            ));
        }
        Ok(files.iter().cloned().collect())
    }

    pub fn attach(&self, rows: &mut [MapRow], detail: MapDetail) {
        if detail == MapDetail::Skeleton {
            return;
        }
        let cap = if detail == MapDetail::Full {
            usize::MAX
        } else {
            20
        };
        for row in rows {
            if let MapRow::File {
                path,
                symbols,
                dropped_symbols,
                ..
            } = row
                && let Some(extracted) = self.symbols.get(path)
            {
                *symbols = extracted
                    .iter()
                    .take(cap)
                    .map(|symbol| MapSymbol {
                        name: symbol.name.clone(),
                        kind: symbol.kind.clone(),
                        line: symbol.start_line,
                        doc: symbol.doc.clone(),
                    })
                    .collect();
                *dropped_symbols = extracted
                    .len()
                    .checked_sub(symbols.len())
                    .filter(|count| *count > 0);
            }
        }
    }

    pub fn graph(&self) -> Option<MapImportGraph> {
        let imports = self.imports.as_ref()?;
        let mut fan_in = BTreeMap::<String, usize>::new();
        let mut fan_out = BTreeMap::new();
        for (path, targets) in imports {
            if targets.is_empty() {
                continue;
            }
            fan_out.insert(path.clone(), targets.len());
            for target in targets {
                *fan_in.entry(target.clone()).or_default() += 1;
            }
        }
        if fan_in.is_empty() && fan_out.is_empty() {
            return None;
        }
        Some(MapImportGraph {
            fan_in: top(fan_in),
            fan_out: top(fan_out),
        })
    }
}

fn top(counts: BTreeMap<String, usize>) -> Vec<MapImportModule> {
    let mut rows = counts
        .into_iter()
        .map(|(module, count)| MapImportModule { module, count })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.module.cmp(&right.module))
    });
    rows.truncate(5);
    rows
}

fn worst(left: Freshness, right: Freshness) -> Freshness {
    match (left, right) {
        (Freshness::Cold, _) | (_, Freshness::Cold) => Freshness::Cold,
        (Freshness::Warming, _) | (_, Freshness::Warming) => Freshness::Warming,
        _ => Freshness::Warm,
    }
}

#[cfg(test)]
#[path = "map_enrichment.tests.rs"]
mod tests;
