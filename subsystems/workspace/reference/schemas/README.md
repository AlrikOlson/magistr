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
| `baselines/envelope.v1.schema.json` | Frozen snapshot of the envelope at its v1 release. Never edited. `test/envelope-additive.test.ts` compares the live schema against it and fails any non-additive change (removed/changed fields, new required fields) — the SDD §4.2 rule that anything non-additive bumps `v`. |

Adding a tool means adding one file under `tools/` and re-running codegen. No
registry to edit; the directory listing is the registry.

## Status markers

Each schema carries `x-workbench-status` and `x-workbench-owned-by`:

- **`seed`** — placed by `chunk:m0-repo-scaffold` so the codegen pipeline is
  provable end to end. The shape follows the SDD but is **not normative**: the
  named owning chunk replaces it with the specified version, its evolution
  rules and its property tests.
- **`normative`** — the owning chunk has landed; changes follow the additive
  rule in SDD §4.2 (new optional fields only within a major).

Do not treat a `seed` schema as a settled contract.

`envelope.schema.json` is **normative** as of `chunk:m0-envelope-contract`:
success and failure are exclusive `oneOf` branches (an envelope never carries
both `error` and `results`), the truncation manifest requires all four of
`dropped`/`reason`/`narrow`/`cursor`, and evolution within v1 is additive-only,
enforced against `baselines/envelope.v1.schema.json`.
