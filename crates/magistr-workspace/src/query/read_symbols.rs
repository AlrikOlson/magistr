use super::ReadRange;
use crate::{
    contracts::{ErrorKind, WorkspaceError},
    worker::WorkerSymbol,
};

/// Symbol addressing shared by read and future native editing.
pub(super) struct SymbolAddress;

impl SymbolAddress {
    pub(super) fn resolve<'a>(
        rows: &'a [WorkerSymbol],
        address: &str,
        file: &str,
    ) -> Result<&'a WorkerSymbol, (WorkspaceError, Vec<String>)> {
        let candidates = Self::candidates(rows, address);
        match candidates.as_slice() {
            [] => {
                let mut known = Vec::new();
                for row in rows {
                    if !known.contains(&row.name.as_str()) {
                        known.push(row.name.as_str());
                        if known.len() == 8 {
                            break;
                        }
                    }
                }
                let notes = if known.is_empty() {
                    Vec::new()
                } else {
                    vec![format!(
                        "symbols in this file include: {}",
                        known.join(", ")
                    )]
                };
                Err((
                    WorkspaceError::new(
                        ErrorKind::SymbolNotFound,
                        format!(
                            "no symbol {address:?} in {file}; revise the address or read by lines"
                        ),
                    ),
                    notes,
                ))
            }
            [symbol] => Ok(symbol),
            _ => Err((
                WorkspaceError::new(
                    ErrorKind::AmbiguousSymbol {
                        candidates: candidates
                            .iter()
                            .map(|row| {
                                format!(
                                    "{} ({}) line {}",
                                    Self::qualified_name(row),
                                    row.kind,
                                    row.start_line
                                )
                            })
                            .collect(),
                    },
                    format!(
                        "{address:?} matches {} symbols in {file}; pick one via lines: {{start, end}}",
                        candidates.len()
                    ),
                ),
                Vec::new(),
            )),
        }
    }

    fn candidates<'a>(rows: &'a [WorkerSymbol], address: &str) -> Vec<&'a WorkerSymbol> {
        let mut candidates: Vec<_> = rows.iter().filter(|row| row.name == address).collect();
        if candidates.is_empty() {
            let segments: Vec<_> = address
                .split("::")
                .flat_map(|part| part.split('.'))
                .filter(|part| !part.is_empty())
                .collect();
            let member = segments.last().copied().unwrap_or(address);
            let parent = segments.len().checked_sub(2).map(|i| segments[i]);
            candidates = rows.iter().filter(|row| row.name == member).collect();
            if candidates.is_empty() {
                let rust_suffix = format!("::{member}");
                let dotted_suffix = format!(".{member}");
                candidates = rows
                    .iter()
                    .filter(|row| {
                        row.name.ends_with(&rust_suffix) || row.name.ends_with(&dotted_suffix)
                    })
                    .collect();
            }
            if candidates.is_empty() {
                let lowered = member.to_lowercase();
                candidates = rows
                    .iter()
                    .filter(|row| row.name.to_lowercase() == lowered)
                    .collect();
            }
            if let Some(parent) = parent.filter(|_| candidates.len() > 1) {
                let parents: Vec<_> = rows.iter().filter(|row| row.name == parent).collect();
                let contained: Vec<_> = candidates
                    .iter()
                    .copied()
                    .filter(|row| {
                        parents.iter().any(|parent| {
                            parent.start_line < row.start_line
                                && (parent.end_line < parent.start_line
                                    || parent.end_line >= row.start_line)
                        })
                    })
                    .collect();
                if !contained.is_empty() {
                    candidates = contained;
                }
            }
        }
        candidates
    }

    pub(super) fn extent(symbol: &WorkerSymbol, rows: &[WorkerSymbol], total: usize) -> ReadRange {
        let start = symbol.start_line as usize;
        let end = if symbol.end_line >= symbol.start_line {
            symbol.end_line as usize
        } else {
            rows.iter()
                .find(|row| row.start_line > symbol.start_line)
                .map_or(total, |next| next.start_line as usize - 1)
        };
        ReadRange {
            start,
            end: end.min(total),
        }
    }

    fn qualified_name(symbol: &WorkerSymbol) -> String {
        if let Some(detail) = &symbol.detail
            && !detail.is_empty()
            && let Some((owner, member)) = symbol.name.split_once("::")
        {
            return format!("<{owner} as {detail}>::{member}");
        }
        symbol.name.clone()
    }
}
