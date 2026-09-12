use super::{
    ReadInput, ReadRange, ReadSurround, ReadSymbolRef, read_symbols::SymbolAddress,
    source_observation::SourceObservation,
};
use crate::{
    contracts::{ErrorKind, Freshness, WorkspaceError},
    index::{IndexQuery, cache::IndexedFile},
    worker::WorkerSymbol,
};

/// Resolved source range and its least-fresh contributing observation.
pub(super) struct ReadSelection {
    pub range: ReadRange,
    pub symbol: Option<ReadSymbolRef>,
    pub enclosing: Option<ReadSymbolRef>,
    pub freshness: Freshness,
    pub notes: Vec<String>,
}

impl ReadSelection {
    pub fn resolve(
        snapshot: &SourceObservation,
        input: &ReadInput,
        lines: &[&str],
        outline: Option<IndexQuery<IndexedFile>>,
    ) -> Result<Self, (WorkspaceError, Vec<String>)> {
        let total = lines.len();
        let mut selection = Self {
            range: ReadRange {
                start: 1.min(total),
                end: total,
            },
            symbol: None,
            enclosing: None,
            freshness: snapshot.freshness,
            notes: Vec::new(),
        };
        if selection.freshness == Freshness::Cold {
            selection.notes.push("the file changed while being read (mtime+size mismatch, retried once); content is the most recent read, labeled cold".into());
        }
        if let Some(address) = &input.symbol {
            selection.select_symbol(snapshot, address, input.surround, lines, outline)?;
        } else if let Some(range) = input.lines {
            selection.select_lines(range, total);
        }
        if total == 0 {
            selection.range = ReadRange { start: 0, end: 0 };
            selection.notes.push("the file is empty".into());
        }
        Ok(selection)
    }

    fn select_symbol(
        &mut self,
        snapshot: &SourceObservation,
        address: &str,
        surround: ReadSurround,
        lines: &[&str],
        outline: Option<IndexQuery<IndexedFile>>,
    ) -> Result<(), (WorkspaceError, Vec<String>)> {
        let mut outline = outline.ok_or_else(|| unavailable("read with a symbol address requires the Tier-1 index; retry once the build completes, or address by lines"))?;
        if outline.truncated || outline.generation == 0 || outline.rows.is_empty() {
            return Err(unavailable(
                "the complete file outline is not available; retry after index refresh, or address by lines",
            ));
        }
        let indexed = outline.rows.iter_mut().find(|row| row.path == snapshot.path).ok_or_else(|| unavailable("the file outline is not available; retry after index refresh, or address by lines"))?;
        if indexed.content_hash != snapshot.content_hash {
            return Err(unavailable(
                "the outline and current file bytes differ; retry after index refresh, or address by lines",
            ));
        }
        self.freshness = worst_freshness(self.freshness, outline.freshness);
        self.notes.extend(outline.notes);
        let symbols = &mut indexed.symbols;
        symbols.sort_by_key(|row| (row.start_line, row.start_col));
        let resolved = SymbolAddress::resolve(symbols, address, &snapshot.path)?;
        self.range = SymbolAddress::extent(resolved, symbols, lines.len());
        self.symbol = Some(resolved.into());
        self.check_anchor(resolved, lines);
        if surround == ReadSurround::Enclosing {
            let symbol_end = resolved.end_line.max(resolved.start_line);
            let parent = symbols.iter().rfind(|row| {
                !std::ptr::eq(*row, resolved)
                    && row.start_line <= resolved.start_line
                    && row.end_line >= symbol_end
            });
            if let Some(parent) = parent {
                self.enclosing = Some(parent.into());
                self.range = ReadRange {
                    start: parent.start_line as usize,
                    end: (parent.end_line as usize).min(lines.len()),
                };
            }
        }
        Ok(())
    }

    fn check_anchor(&mut self, resolved: &WorkerSymbol, lines: &[&str]) {
        // Qualified Rust names identify symbols in the index; the declaration
        // contains the member token, just as with dotted reference addresses.
        let member = resolved
            .name
            .rsplit("::")
            .next()
            .unwrap_or(&resolved.name)
            .rsplit('.')
            .next()
            .unwrap_or(&resolved.name);
        let anchor = self
            .range
            .start
            .checked_sub(1)
            .and_then(|line| lines.get(line));
        if !anchor.is_some_and(|line| line.contains(member)) {
            self.freshness = Freshness::Cold;
            self.notes.push(format!("the symbol anchor (line {}) did not match the current content; line numbers may be approximate until the index catches up", self.range.start));
        }
    }

    fn select_lines(&mut self, input: ReadRange, total: usize) {
        let start = input.start.min(input.end).max(1);
        let end = input.start.max(input.end);
        if start != input.start || end != input.end {
            self.notes.push(
                "lines: start and end were swapped/clamped to a valid ascending range".into(),
            );
        }
        if start > total && total > 0 {
            self.notes.push(format!(
                "lines.start {start} is past the end of the file ({total} lines)"
            ));
        }
        if end > total {
            self.notes.push(format!(
                "lines.end clamped to the end of the file ({total} lines)"
            ));
        }
        self.range = ReadRange {
            start: start.min(total),
            end: end.min(total),
        };
    }
}

fn unavailable(message: &str) -> (WorkspaceError, Vec<String>) {
    (
        WorkspaceError::new(ErrorKind::IndexUnavailable, message),
        Vec::new(),
    )
}

fn worst_freshness(a: Freshness, b: Freshness) -> Freshness {
    match (a, b) {
        (Freshness::Cold, _) | (_, Freshness::Cold) => Freshness::Cold,
        (Freshness::Warming, _) | (_, Freshness::Warming) => Freshness::Warming,
        _ => Freshness::Warm,
    }
}
