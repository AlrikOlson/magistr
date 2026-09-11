# Phase 1 Data Model: Companion Routing Skill + CI Drift Check

No runtime storage. Two repository artifacts and one in-memory checker vocabulary.

## SKILL.md (the skill)

| Part | Form | Bound by the check? |
|---|---|---|
| Frontmatter | YAML: `name: workbench-routing`, `description: …` | name must be `workbench-routing`; no `generated` marker anywhere in the file |
| `## Routing table` | Markdown table, columns exactly `Instead of` \| `Use` \| `Arguments that matter` | yes — R1/R2/R3 |
| `## Recovery table` | Markdown table, columns exactly `Code` \| `Recovery` \| `What to do` | yes — R4 |
| `## Worked examples` | Fenced blocks ```` ```json workbench:<tool> ```` | yes — R1/R5 |
| Everything else | Prose (when not to route, envelopes, confirm flows, iterating from the trace, plugin discovery) | no |

### Token grammar inside bound cells

- Tool reference: a backticked tool name, e.g. `` `search` ``.
- Facet selection: a backticked `facet:value`, e.g. `` `mode:references` ``, `` `op:rewrite` ``, `` `detail:outline` ``, `` `scope:workspace` ``, `` `surround:imports` ``, `` `onConflict:force` ``. `facet` must be an enum-typed property of the row's tool; `value` one of its enum values. A facet may be marked rejected in prose (e.g. `` `mode:semantic` `` — rejected in v1) and still counts as routed.
- Argument reference: a backticked property name in *Arguments that matter*, e.g. `` `query` ``, `` `path` ``.
- A routing row names exactly one tool in *Use*; multi-tool habits use multiple rows.

## Checker vocabulary (`scripts/check-skill-drift.ts`)

```ts
interface ToolSurface { readonly name: string; readonly schema: JsonSchemaObject }        // from schemas/tools
interface TaxonomyEntry { readonly code: string; readonly recovery: string }             // from schemas/error-taxonomy.json
interface Violation { readonly rule: "R1"|"R2"|"R3"|"R4"|"R5"|"R6"|"R7"; readonly message: string; readonly where?: string }
function checkSkillDrift(input: { skill: string; tools: readonly ToolSurface[]; taxonomy: readonly TaxonomyEntry[]; copy?: string }): readonly Violation[]
```

- R1 tool universe (both directions) · R2 enum coverage (both directions) · R3 argument existence + required coverage · R4 taxonomy one-to-one · R5 example validity (ajv) + one example per tool · R6 dogfood copy byte-equal · R7 no `generated` marker, frontmatter name.
- The allowlisted schema-less built-in: `workbench_status` (routed and exampled; no schema to validate against — its example must be `{}`).

## Invariants (asserted in tests)

1. Over the committed SKILL.md, schemas, and taxonomy: zero violations.
2. Each synthetic drift (added enum value, unknown argument, wrong recovery class, invalid example, missing tool row, differing copy, generated marker) yields ≥1 violation naming its rule.
3. The checker's source contains no write API (`writeFile`, `writeFileSync`, `appendFile`, `Bun.write`) and no `--fix`/`--write` flag.
4. The checker is deterministic: same inputs → same violation list (pure function; sorted output).
