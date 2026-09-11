# Contract: the skill drift check (`bun run lint:skill`)

The skill is prose; this contract is what makes it checkable. It binds the three machine-bound sections of `skills/workbench-routing/SKILL.md` to the repository's single sources of truth.

## Oracles

- Tool universe: the basenames of `schemas/tools/*.input.schema.json` (today: edit, files, map, read, run, script, search, undo) plus the allowlisted schema-less built-in `workbench_status`.
- Property names, `required`, and enum values: each tool's input schema as generated into `src/generated/schemas.ts` (same bytes, Constitution V).
- Error codes and recovery classes: `schemas/error-taxonomy.json`.

## Rules (each failure names the rule and the offending token)

| Rule | Direction | Fails when |
|---|---|---|
| R1 tool universe | skill→schema | a routing row or example names a tool outside the universe |
| R1 tool universe | schema→skill | a tool in the universe has no routing row or no worked example |
| R2 enum coverage | skill→schema | a `facet:value` token names a facet that is not an enum-typed property of that tool, or a value outside its enum |
| R2 enum coverage | schema→skill | an enum value of any enum-typed property of any tool appears in no routing row |
| R3 arguments | skill→schema | a backticked argument in *Arguments that matter* is not a property of the row's tool |
| R3 arguments | schema→skill | a `required` property of any tool appears in no routing row for that tool |
| R4 recovery | both | the recovery table is missing a taxonomy code, lists one twice, lists an unknown code, or pairs a code with a recovery class other than the taxonomy's |
| R5 examples | skill→schema | a ```` ```json workbench:<tool> ```` block is not JSON or fails validation against the tool's input schema (the schema-less built-in's example must be `{}`) |
| R6 copy | — | `.claude/skills/workbench-routing/SKILL.md` exists and differs from the canonical file |
| R7 hand-written / structure | — | the file carries a generated marker (an HTML comment containing `generated` or `do not edit`, the literal `DO NOT EDIT`, or `@generated` — narrow on purpose, so prose may say "the generated schemas"), the frontmatter `name` is not `workbench-routing`, a bound section or its table is missing, or a table's columns are not the contracted ones |

A tool with no worked example is reported under R1 (universe coverage), not R5; R5 is validation only. A routing row names exactly one tool in *Use* — a second backticked non-`facet:value` token in that cell is read as a second tool and fails R1, so Use-column prose must not backtick non-tool words.

## Invocation

```
bun run lint:skill                     # canonical skill + dogfood copy (when present)
bun run lint:skill <skill> [<copy>]    # other paths (tests)
```

Positional paths only. Every `--option` is refused with exit 2 — there is no fix, write, or quiet mode.

## Exit codes

- `0` — no violations.
- `1` — violations; each printed as `R<n> <where>: <message>`, sorted, deterministic.
- `2` — usage (any option, more than two arguments) or the skill file is missing.

## What the check never does

- It never writes, rewrites, or suggests replacement prose. There is no `--fix`.
- It never reads the running server or the trace file.
- It never inspects prose outside the three bound sections.

## Evolution

- Adding a tool, an enum value, a required argument, or a taxonomy code MUST make this check fail until the skill is revised by hand — that is the point.
- New rules are additive; renumbering existing rules bumps the contract's major (recorded in this file's header).
