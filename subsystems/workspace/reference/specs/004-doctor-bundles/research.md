# Phase 0 Research: Workbench Doctor + Diagnostic Bundles

All unknowns from Technical Context resolved. Substrate pinned in think:135 (D1–D6); each decision below is grounded in a mechanism that already ships.

## R1 — CLI surface: extend the existing `doctor` dispatch branch

**Decision**: `workbench-mcp doctor [workspace] [--config key=value ...]` (full report), `workbench-mcp doctor bundle [workspace] [--out <path>] [--config ...]` (bundle), `workbench-mcp doctor config ...` unchanged. Implemented by widening the existing `process.argv[2] === "doctor"` branch in `src/main.ts` (today it hard-fails unless `argv[3] === "config"`).

**Rationale**: the verb family (`key rotate`, `init`, `doctor config`) and its exit-code convention (64 usage, 66 missing root, 65 unsupported-version) already exist; a new binary or subcommand framework would be invention. `doctor config` established the no-lock precedent this feature generalizes.

**Alternatives considered**: a separate `workbench-doctor` binary (rejected: SDD §6.1 is explicit — one binary); routing doctor through a running server via MCP (rejected: the gate scenario is precisely the case where the server may be unusable).

## R2 — Index probe: read-only SQLite open + metadata, never the corpus

**Decision**: probe `.workbench/index.db` by opening read-only (`bun:sqlite` `{ readonly: true }`): capture `PRAGMA user_version` (expected `INDEX_SCHEMA_VERSION = 1`), `PRAGMA quick_check` (bounded integrity), and the `meta` table (`schema_version`, `generation`) plus row counts of `files`/`symbols`. Open failure or quick_check failure ⇒ finding: store `index.db`, data class **cache**, remediation "delete the file; the next server start rebuilds it" (the exact `openVersionedDatabase` cache-class behavior — discarded and rebuilt). A missing file ⇒ the distinct healthy finding "no index yet (fresh workspace, Tier-0 answers only)". A `user_version` above the supported value ⇒ "newer than this binary" fail-closed finding (§10.2 vocabulary).

**Rationale**: mirrors `openVersionedDatabase`'s own probe semantics without importing its open-for-write behavior; a read-only open cannot heal, migrate, or clobber, which is what FR-005 requires. `quick_check` over `integrity_check` keeps SC-003's 5-second budget.

**Alternatives considered**: reusing `openVersionedDatabase` directly (rejected: it repairs — discards/rebuilds cache and quarantines durable stores — which violates read-only diagnosis); byte-sniffing the SQLite header only (rejected: misses torn pages that `quick_check` catches, which is the seeded-corruption class the gate exercises).

## R3 — Bundle format: tar.gz via a minimal in-tree ustar writer + `Bun.gzipSync`

**Decision**: the bundle is a gzip-compressed POSIX ustar archive written by a ~100-line in-tree writer (`src/doctor/tar.ts`): fixed 512-byte headers, name ≤ 100 chars (all members are flat `workbench-doctor/<member>.json|txt` names), correct checksum field, two zero blocks at end. Compression via `Bun.gzipSync`. Default output `workbench-doctor-bundle-<ISO-stamp>.tar.gz` in the invoking directory; `--out` overrides; an existing file is refused (exit 64).

**Rationale**: SDD §11.2 says "tarball"; the stack commitment forbids new runtime dependencies; ustar with short flat names avoids every hard part of tar (no pax extensions, no long names, no symlinks). Any `tar -xzf` opens it — US3's inspectability requirement.

**Alternatives considered**: a directory instead of an archive (rejected: SDD names a tarball, and one file is what a ticket attachment needs); zip (rejected: no built-in zip writer in Bun, and tar is the SDD's word); an npm tar package (rejected: zero-dependency commitment).

## R4 — Redaction: one choke point at bundle serialization

**Decision**: all bundle members pass through a single `redact()` at serialization time (`src/doctor/bundle.ts`): absolute paths outside the workspace root are replaced with `<outside-workspace>/…basename`; the workspace root prefix itself is replaced with `<workspace>` so machine identity leaks less; environment variable *values* never enter members (only key names and deciding layers, the existing `renderDoctorConfig` discipline); no member ever contains repo file bytes because no probe reads repo files — redaction is defense in depth on top of that structural guarantee. The privacy test seeds a sentinel string into workspace files and asserts its absence from every member.

**Rationale**: the constitution's redaction principle for envelopes ("exactly one choke point") transfers verbatim; a structural no-read guarantee plus a single choke point is auditable, scattered per-probe scrubbing is not.

**Alternatives considered**: allowlisting fields per member (rejected as the primary mechanism: fragile against future member additions; kept implicitly since members are typed structs, not free text).

## R5 — The gate scenario: remediation derived from bundle members alone

**Decision**: `test/doctor-bundle-gate.test.ts` seeds a workspace state dir with a real (empty-schema) index.db, overwrites it with garbage bytes, runs the bundle path, then un-tars the bundle and hands ONLY the extracted members to a resolver step that (a) locates the finding member, (b) asserts it names the store, the `cache` data class, and a remediation string, (c) executes that remediation mechanically (delete the named store file), and (d) verifies recovery by re-running the index probe (healthy "no index yet") — zero user-data loss asserted by checking `workspace.db` and `journal` stores untouched. The test never reads the live workspace between bundle creation and remediation derivation.

**Rationale**: SDD §11.3 wants the scenario *exercised*; encoding the support engineer as "code that sees only the bundle" makes the no-repo-access constraint structural rather than procedural.

**Alternatives considered**: a human-run quickstart only (rejected: the M6 gate must be CI-checkable; the quickstart documents the same flow for humans).

## R6 — Exit codes: findings are distinguishable

**Decision**: exit 0 = report/bundle produced, all probes healthy; exit 1 = report/bundle produced, at least one unhealthy finding; 64/65/66 keep their existing meanings for usage, config-from-the-future (doctor config path only), and missing root. The full report treats an `UnsupportedVersionError` from any store as a *finding* (exit 1), not an abort — `doctor config`'s existing 65 behavior is untouched (FR-009).

**Rationale**: scripts (and the gate test) need "ran and found problems" distinct from "could not run"; the existing convention already reserves 64/66.

**Alternatives considered**: always exit 0 (rejected: FR-008); a per-finding-class exit map (rejected: over-fits; the report carries the detail).

## R7 — "Recent engine crashes": quarantine strikes are the shipped evidence

**Decision**: the crashes section reads `.workbench/quarantine.jsonl` and reports each entry's `lastCrash`, `strikes`, `at`, and quarantined flag — labeled honestly as "crash evidence (from quarantine strikes)". A standalone crash ledger does not exist yet; the section says so and names the chunks that add richer sources (m6-tracing ring buffer, m6-crash-reports).

**Rationale**: FR-002 requires every section to name its source; inventing a new crash store would violate the no-new-stores scope and duplicate m6-tracing.

**Alternatives considered**: parsing supervisor stderr logs (none are persisted — nothing to read); adding a crash ledger now (rejected: m6-crash-reports' scope, and doctor must not write).

## R8 — Watcher health: reported honestly (corrected during build)

**Decision (as built)**: the environment section emits `watcher: watcher adapters present (fs.watch + polling) but not active in this build`. The original shape-phase claim ("no watcher subsystem exists") was wrong: `src/adapters/fswatch` ships `FsWatchAdapter`/`PollingWatcherAdapter` with a test suite, but nothing outside the module imports them — built, not wired. When the watcher is wired into the serving path, this line becomes a real probe.

**Rationale**: SDD §11.2 lists watcher health among doctor's probes; the truthful state is "adapters exist, not active", and doctor reports exactly that rather than fabricating a runtime probe over an unwired subsystem.

**Alternatives considered**: omitting the line (rejected: silence reads as an oversight); claiming "no watcher subsystem" (rejected during build: contradicted by `src/adapters/fswatch`).

## R9 — Environment probes: reuse `detectNetworkFs` and the native-binding presence check

**Decision**: filesystem type/network-fs detection calls the existing `detectNetworkFs(root, env)` (same function `openWorkspace` uses, without opening the workspace); engine selection reports whether the native `workbench-core.<platform>.node` binding resolves (the plugin-TCK precedent check) vs the wasm32 fallback; journal-mode consequence (WAL vs TRUNCATE) is derived from the detection, labeled as "expected mode" since no db is opened.

**Rationale**: both probes exist as shipped code paths; doctor must report what the server *would* decide without becoming a server.

**Alternatives considered**: opening the workspace to read the applied journal mode (rejected: `openWorkspace` takes the lock and can mutate — FR-005 forbids both).
