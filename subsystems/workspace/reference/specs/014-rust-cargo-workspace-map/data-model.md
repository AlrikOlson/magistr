# Phase 1 Data Model: Cargo Workspace Map

No stored data changes. One additive port evolution (`PORT_VERSIONS.index` 8 → 9). No schema
shape change (`map` rows are envelope results; only the description sentence moves).

## Port (src/ports/index-port.ts) — additive

```
ImportGraphQuery
  limit?    number
  crates?   readonly CrateScope[]        NEW (v9): ask for per-crate use edges

CrateScope
  name      string                       the `use` name (hyphens already underscored)
  dir       string                       member directory, "." for the root package

ImportGraphResult
  fanIn, fanOut, freshness               unchanged
  crates?   readonly CrateImportEdge[]   NEW (v9): present iff the query asked and the adapter records imports

CrateImportEdge
  from      string                       importer member dir (longest prefix of the importing path)
  to        string                       the imported crate's use name
  files     number                       distinct importing files
```

Engine child (indexer.ts): `SELECT DISTINCT path, target FROM imports WHERE target = ?1 OR
target LIKE ?1 || '::%'` per name (bounded: ≤ 256 names), grouped by (longest member dir, name).
In-process adapter: never answers `crates`.

## map rows (src/tools/map.ts, src/tools/map-cargo.ts)

```
MapCrate
  kind                  "crate"
  path                  string                      member dir ("." for the root package)
  name                  string                      [package] name
  roles                 ("lib" | "bin" | "proc-macro" | "build")[]
  edition?              string
  entryPoints           MapCrateEntryPoint[]        ≤ 8, manifest targets first
  moreEntryPoints?      number
  features              string[]                    ≤ 16, "default" first
  moreFeatures?         number
  dependencies          MapCrateDependency[]        member deps only
  dependents            number
  externalDependencies  number

MapCrateEntryPoint
  path      string                                  workspace-relative
  kind      "lib" | "bin" | "example" | "test" | "bench" | "build"
  name?     string

MapCrateDependency
  name      string                                  the member's package name
  kind?     "dev" | "build"                         absent = normal
  files?    number                                  importing files, when the index answered

MapCargoSummary (overview.cargo)
  kind        "workspace" | "crate"
  members     number
  edges       number                                member → member Cargo edges
  omitted?    number                                members beyond MAP_CRATE_ROWS_MAX
  unusedDeps? number                                declared member deps with no use edge (index answered)
  top         { name: string; dependents: number }[]   ≤ 5

MapResult = MapOverview | MapCrate | MapEntry
row order: overview, crate rows (dependents desc, name), tree rows (breadth-first)

MAP_CRATE_ROWS_MAX = 24
MAP_CRATE_ENTRY_POINTS_MAX = 8
MAP_CRATE_FEATURES_MAX = 16
MAP_CRATE_MANIFESTS_MAX = 256
```

## Notes map may add

```
"use counts unavailable: the index could not answer per-crate import edges"   (throw / no field)
"N workspace members beyond the 24 shown; map path:<dir> lists a member"       (omitted)
"members entries `vendor/**` not expanded (only <dir>/* globs are)"            (skipped globs)
"M member manifests beyond the 256 read; those members are not shown"          (manifest cap)
```

## Golden

```
test/support/golden/map/cargo-workspace.expected.json   overview.cargo + crate rows of the fixture
```
