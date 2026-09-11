# Phase 1 Data Model: Rust Move-Aware Files

No stored data changes. No cache-class or durable-class store is touched: the index, journal,
config and manifest formats are unchanged, and `Cargo.toml` edits are ordinary journaled
writes. What follows is the in-memory plan the Rust branch computes and hands to the existing
write batch.

## RustMovePlan (in-memory, per move)

```
RustMovePlan
  crateDir        string              crate-relative root ("" when the workspace root is the crate)
  rootFile        string | null       the crate root file the module tree hangs from
  oldModule       string[]            e.g. ["a","b"]   (crate::a::b)
  newModule       string[] | null     null when the move is not rewritable (see `skipped`)
  declaration     DeclarationChange | null
  pathRewrites    { path, count }[]   importer files (and the moved file) with rewritten paths
  manifest        { path, entries: number } | null
  unrewritten     { path, line, reason }[]   brace-group members, #[path], cross-crate, ...
  skipped         { reason } | null   non-leaf, declaration not found, same module path
  notes           string[]            rendered from the above; the envelope carries these
```

```
DeclarationChange
  kind     "rename" | "move" | "remove-only"
  from     { path, line, text }        the existing `mod old;` item (visibility/attrs kept)
  to       { path, text } | null       the new parent file and the item to insert; null when
                                       the destination parent module file does not exist
```

Rules:
- `newModule === null` ⇔ `skipped !== null`; then `declaration`, `pathRewrites` and `manifest`
  are empty and the move proceeds as today's plain file move.
- A `pathRewrites` entry never appears for a file with zero rewrites; an `unrewritten` entry
  always carries a 1-based line.
- `manifest` is present only when at least one target `path` entry named the moved file.
- The plan is derived from the same `files` set and `read` function `resolveMove` already uses;
  it reads nothing the generic move does not.

## Extended (additive): the move note

The generic move note keeps its shape for non-Rust moves, with "rust/" removed from its
parenthetical. A Rust move replaces it with the Rust note (module path before → after,
declaration file, counts, and the unrewritten/skipped statements), followed by the existing
`move: "<to>" already exists and will be overwritten` note when applicable.

## Unchanged

- `WriteOp`, `WriteReport`, the confirm token contract, `FilesInput`, the envelope.
- `schemas/tools/files.input.schema.json`: description text only (Rust joins the move list);
  no field changes.
