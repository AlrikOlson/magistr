# Phase 1 Data Model: Workbench Doctor + Diagnostic Bundles

No new persistent stores. Every entity below is in-memory during a doctor run, except the bundle file itself.

## DoctorFinding

An unhealthy (or noteworthy) condition discovered by a probe.

| Field | Type | Notes |
|---|---|---|
| `store` | string | What was probed: `index.db`, `quarantine.jsonl`, `plugin-health.json`, `plugins.json`, `config`, `environment`, or a plugin/store-specific label |
| `dataClass` | `"cache" \| "durable" \| "n/a"` | §10.2 vocabulary; drives the remediation template |
| `severity` | `"healthy" \| "info" \| "unhealthy"` | `unhealthy` findings flip the exit code to 1 |
| `symptom` | string | What was observed (e.g. `quick_check failed: database disk image is malformed`) |
| `remediation` | string | Present on every `unhealthy` finding (FR-007): cache ⇒ discard-and-rebuild path; durable ⇒ quarantine-intact recovery; version-from-the-future ⇒ upgrade |

Invariants: an `unhealthy` finding without `remediation` is a defect (asserted in tests); findings never contain repo file contents.

## DoctorSection

One probe's output.

| Field | Type | Notes |
|---|---|---|
| `id` | string | Stable member name: `environment`, `index`, `config`, `plugins`, `quarantine`, `crashes` |
| `source` | string | The on-disk artifact or mechanism read (FR-002), e.g. `.workbench/quarantine.jsonl` |
| `findings` | DoctorFinding[] | Healthy sections still carry an explicit `healthy` finding — absence of findings is a claim, never silence (US1 scenario 3) |
| `detail` | object | Section-specific structured payload (below) |

Section payloads:

- `environment`: `{ networkFs: {detected, method, fstype?}, expectedJournalMode, engine: {native, artifactKey}, watcher: string }`
- `index`: `{ present, userVersion?, schemaVersion?, generation?, fileCount?, symbolCount?, quickCheck? }` — `lastBuildError` was dropped as-built: the child keeps it in memory only, and doctor reports only what is recorded on disk
- `config`: the existing `renderDoctorConfig` text plus the structured `trace` (`key`, `value`, `decidedBy`, `pinnedBy?`) — reused, not forked
- `plugins`: `{ inventory: [{name, trust, source, dir?, manifestPresent?}], unhealthy: [{plugin, componentHash, reason, at}] }` — as-built: `trust` is the registry tier when present, else the source; `manifestPresent` (plugin.toml on disk) replaces the spec-phase `componentPresent`, which is what the inventory can honestly attest without parsing manifests
- `quarantine`: `{ entries: [{path, strikes, quarantined, lastCrash, at}], malformedLines: number }`
- `crashes`: `{ evidence: [{path, lastCrash, at}], note }` — derived from quarantine strikes (R7)

## DoctorReport

`{ workspaceRoot, generatedAt, toolVersion, sections: DoctorSection[], exitCode }` — rendered to stdout as sectioned text; the same object serializes into bundle members.

## BundleManifest (`manifest.json`, the bundle's own versioned contract)

| Field | Type | Notes |
|---|---|---|
| `bundleFormatVersion` | 1 | Versioned from day one; no preservation constraint before Magistr's public release |
| `toolVersion` | string | package.json version |
| `createdAt` | string | ISO timestamp |
| `workspaceLabel` | string | `<workspace>` placeholder identity, not the absolute path |
| `members` | string[] | Every member name in the archive |

## Bundle layout

```
workbench-doctor/
├── manifest.json
├── report.txt            # the human-readable report, verbatim
├── environment.json
├── index.json
├── config.txt            # renderDoctorConfig output (provenance text)
├── plugins.json
├── quarantine.json
└── crashes.json
```

All members pass the single `redact()` choke point (research R4). State transitions: none — doctor is stateless and read-only; the only produced artifact is the archive, refused if the path exists.
