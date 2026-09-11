# Quickstart: Workbench Doctor + Diagnostic Bundles

Runnable validation scenarios proving the feature end to end. Contracts: [contracts/cli-doctor.md](contracts/cli-doctor.md). Entities: [data-model.md](data-model.md).

## Prerequisites

- `bun install` done; native binding built (`bun run build:core`) or the wasm fallback present.
- A scratch workspace: `mkdir -p /tmp/dr-ws && cd /tmp/dr-ws && echo 'x' > a.ts`.

## Scenario 1 — Healthy report (US2)

```
bun /path/to/repo/src/main.ts doctor /tmp/dr-ws
echo $?   # 0
```

Expect all six sections, each with a `source:` line; index section reports "no index yet" as a normal finding; watcher line reports no watcher subsystem; config section shows per-key `(defaults)` provenance.

## Scenario 2 — The M6 gate: corrupt index resolved from the bundle alone (US1)

```
# 1. Give the workspace a state dir with an index, then corrupt it:
mkdir -p /tmp/dr-ws/.workbench
# (run the server once, or copy any valid index.db) then:
printf 'garbage-not-sqlite' > /tmp/dr-ws/.workbench/index.db

# 2. Produce the bundle:
bun src/main.ts doctor bundle /tmp/dr-ws --out /tmp/bundle.tar.gz
echo $?   # 1 (findings present)

# 3. Support-engineer view — ONLY the bundle:
mkdir /tmp/support && tar -xzf /tmp/bundle.tar.gz -C /tmp/support
cat /tmp/support/workbench-doctor/index.json
# ⇒ store index.db, dataClass "cache", symptom (unreadable/quick_check),
#   remediation: delete .workbench/index.db; the next server start rebuilds it.

# 4. Apply the remediation and re-check:
rm /tmp/dr-ws/.workbench/index.db
bun src/main.ts doctor /tmp/dr-ws && echo healthy   # exit 0, "no index yet"
```

Automated as `test/doctor-bundle-gate.test.ts` (the release-gate test — the resolver step sees only extracted members).

## Scenario 3 — Privacy: the bundle is safe to share (US3)

```
echo 'SENTINEL-do-not-leak-9f3a' > /tmp/dr-ws/secret.ts
bun src/main.ts doctor bundle /tmp/dr-ws --out /tmp/b2.tar.gz
tar -xzOf /tmp/b2.tar.gz | grep -c 'SENTINEL-do-not-leak-9f3a'   # 0
```

Also verify: every member is readable text; absolute non-workspace paths appear as `<outside-workspace>/…`.

## Scenario 4 — Beside a live server, read-only (US4)

```
bun src/main.ts /tmp/dr-ws &          # server holds the lock
sleep 2
find /tmp/dr-ws/.workbench -type f -exec md5sum {} + | sort > /tmp/before
bun src/main.ts doctor /tmp/dr-ws     # completes; no lock wait
bun src/main.ts doctor bundle /tmp/dr-ws --out /tmp/b3.tar.gz
find /tmp/dr-ws/.workbench -type f -exec md5sum {} + | sort > /tmp/after
diff /tmp/before /tmp/after && echo untouched
```

## Scenario 5 — Quarantine + breakered plugin visibility (US2, chunk acceptance)

Seed `.workbench/quarantine.jsonl` with a `QuarantineEntry` line and `.workbench/plugin-health.json` with a breach record, then:

```
bun src/main.ts doctor /tmp/dr-ws | sed -n '/quarantine/,/crashes/p'
```

Expect the quarantined path with strikes and the plugin with its reason + "changed component clears the breaker" recovery line; both also present in the bundle members.

## Gates (run after implementation)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check
cargo test --manifest-path crates/workbench-core/Cargo.toml
```
