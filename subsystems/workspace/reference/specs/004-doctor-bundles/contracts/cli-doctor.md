# Contract: `workbench-mcp doctor` CLI surface

The doctor verb family is a public CLI contract. Changes here follow Constitution V: additive within a major, anything else bumps.

## Verb grammar

```
workbench-mcp doctor [workspace] [--config key=value ...]
workbench-mcp doctor bundle [workspace] [--out <path>] [--config key=value ...]
workbench-mcp doctor config [workspace] [--config key=value ...]   # unchanged, pre-existing
```

- `workspace` defaults to `.`; resolved against the invoking directory.
- `--config` pairs feed the per-session layer of config resolution, exactly as the server and `doctor config` accept them.
- Unknown options: exit 64 with usage.
- `doctor` and `doctor bundle` take **no lock** and perform **no writes** under the workspace or `.workbench/` (the bundle file is the single output, refused if it already exists).

## Exit codes

| Code | Meaning |
|---|---|
| 0 | report/bundle produced; every probe healthy |
| 1 | report/bundle produced; at least one unhealthy finding |
| 64 | usage error (bad option, existing `--out` target) |
| 65 | `doctor config` only: a config layer from a newer binary (pre-existing behavior, unchanged) |
| 66 | workspace root does not exist |

The full report never exits 65: a from-the-future store is an `unhealthy` finding inside the report (exit 1).

## Report sections (stdout, in order)

1. `environment` — network-fs detection outcome + method, expected journal mode, engine selection (native binding vs wasm fallback), watcher status line.
2. `index` — presence, `user_version`, meta `schema_version` + `generation`, file/symbol counts, `quick_check` verdict, last build error if recorded.
3. `config` — the existing `renderDoctorConfig` rendering, embedded verbatim (per-key value ← deciding layer, pins, warnings).
4. `plugins` — inventory with trust tiers and component presence; unhealthy/breakered plugins with reason, component hash, and the recovery rule.
5. `quarantine` — every `quarantine.jsonl` entry (path, strikes, quarantined, lastCrash, at); malformed line count.
6. `crashes` — crash evidence derived from quarantine strikes, labeled with its source and the honest note about richer future sources.

Every section prints its `source:` line (FR-002). Every unhealthy finding prints `remediation:` (FR-007).

## Bundle contract

- Archive: gzip-compressed POSIX ustar; all members under the `workbench-doctor/` prefix; member set exactly as listed in [data-model.md](../data-model.md).
- `manifest.json` carries `bundleFormatVersion: 1`. Adding members or optional manifest fields is additive; removing/renaming members or changing member semantics bumps the major.
- Redaction invariants (testable): no repo file contents anywhere; absolute paths outside the workspace reduced to `<outside-workspace>/…`; workspace root rendered as `<workspace>`; no environment variable values.
- Default output path: `./workbench-doctor-bundle-<ISO-stamp>.tar.gz` in the invoking directory; `--out` overrides; existing target ⇒ exit 64, nothing written.

## Compatibility pins (tests)

- `doctor config` byte-for-byte behavior unchanged (FR-009).
- Server/state-dir untouched by any doctor invocation (before/after byte-identical, US4).
- Bundle unpacks with standard `tar -xzf` (ustar conformance).
