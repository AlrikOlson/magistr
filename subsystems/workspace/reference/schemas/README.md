# `schemas/` — the single source of truth

Constitution principle V and SDD §4.2 both require that every MCP `inputSchema`
and `outputSchema` be **generated** from this directory. Nothing hand-writes a
tool schema in TypeScript; `src/generated/schemas.ts` is emitted by
`bun run codegen` and CI fails on drift (`bun run codegen:check`).

## Layout

| Path | Meaning |
|---|---|
| `envelope.schema.json` | The `ResponseEnvelope`, which is *every* tool's `outputSchema`. |
| `error-taxonomy.json` | The published error-code taxonomy (SDD §4.3): every code, its one recovery class, and the payload fields it requires. An **array** of entries, so a duplicated code is representable and `test/error-taxonomy.test.ts` can fail it — one-code-one-recovery is a schema test, not a convention. |
| `tools/<name>.input.schema.json` | One tool's `inputSchema`. The file stem is the tool name. |
| `baselines/envelope.v1.schema.json` | Imported envelope snapshot used by `test/envelope-additive.test.ts`. Magistr's pre-release policy supersedes the former frozen-baseline and mandatory version-bump rules: update or remove this baseline and its checks when the intended schema changes. |

Adding a tool means adding one file under `tools/` and re-running codegen. No
registry to edit; the directory listing is the registry.

## Status markers

Each schema carries `x-workbench-status` and `x-workbench-owned-by`:

- **`seed`** — placed by `chunk:m0-repo-scaffold` so the codegen pipeline is
  provable end to end. The shape follows the SDD but is **not normative**: the
  named owning chunk replaces it with the specified version, its evolution
  rules and its property tests.
- **`normative`** — the owning chunk has landed; this describes the current
  schema. Change it with current callers, generated output and tests under
  Magistr's pre-release policy; earlier schema versions impose no constraints.

Do not treat a `seed` schema as a settled contract.

`envelope.schema.json` is **normative** as of `chunk:m0-envelope-contract`:
success and failure are exclusive `oneOf` branches (an envelope never carries
both `error` and `results`), the truncation manifest requires all four of
`dropped`/`reason`/`narrow`/`cursor`. These are current properties, not preservation
requirements; update the schema, baseline and checks together when they change.
