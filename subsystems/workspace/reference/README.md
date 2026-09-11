# workbench-mcp

A ranked, verified file substrate for coding agents. An MCP server that replaces the
bash/grep/sed loop with eight typed tools.

## The same question, asked two ways

Here is what a coding agent gets back when it asks the shell where `resolveConfig` is used:

```
$ grep -rn "resolveConfig" src
src/main.ts:67:  resolveConfig,
src/main.ts:441:    const config = resolveConfig({ root: absRoot, sessionFlags: flags });
src/config/index.ts:18:  resolveConfig,
src/doctor/probes.ts:15:import { renderDoctorConfig, resolveConfig } from "../config";
src/doctor/probes.ts:254:    const config = resolveConfig({ root, env, sessionFlags });
src/protocol/workspace.ts:27:import { configNotes, type ResolvedConfig, resolveConfig } from "../config";
src/protocol/workspace.ts:238:    config = resolveConfig({ root, env, sessionFlags });
src/config/resolve.ts:270:export function resolveConfig(options: ResolveConfigOptions): ResolvedConfig {
```

Eight lines, in directory-walk order, the definition last. On a real repository it is four
hundred lines, and the agent picks one.

Here is the same question asked of workbench (`search`, `query: "resolveConfig"`, `path: "src/**"`,
trimmed here for width):

```json
{
  "v": 1, "ok": true, "freshness": "cold",
  "budget": { "requested": 1200, "used": 270 },
  "results": [
    { "file": "src/config/resolve.ts", "totalHits": 1, "hits": [
        { "line": 270, "excerpt": "export function resolveConfig(options: ResolveConfigOptions): ResolvedConfig {" } ] },
    { "file": "src/main.ts", "totalHits": 2, "hits": [
        { "line": 441, "excerpt": "    const config = resolveConfig({ root: absRoot, sessionFlags: flags });" },
        { "line": 67,  "excerpt": "  resolveConfig," } ] },
    { "file": "src/doctor/probes.ts", "totalHits": 2, "hits": [ "…" ] },
    { "file": "src/protocol/workspace.ts", "totalHits": 2, "hits": [ "…" ] },
    { "file": "src/config/index.ts", "totalHits": 1, "hits": [ "…" ] }
  ],
  "notes": ["mode:auto ran lexical; use mode:structural for AST patterns or mode:references for symbol edges"]
}
```

The definition is first. Hits are grouped by file, call sites rank above re-exports, and the
whole thing fit in 270 of the 1,200 tokens it was allowed. The `freshness` field says this came
from a cold start with no index yet. If the answer had been cut, a `truncation` block would say
how many hits were dropped and how to narrow the query. And every line number was checked against
the file's mtime and size a moment before it was sent.

## What this is

If you've watched a coding agent work, you've seen it grep, get four hundred lines back, and pick
the wrong one. The tools underneath were never the problem: ripgrep is excellent. The problem is
that a flat text dump is a bad interface for a language model, and so is blind byte substitution,
and so is finding out three turns later that a `sed` left a file that no longer parses.

Workbench keeps the engines and replaces the interface. It is an MCP server that exposes search,
read, edit, and the rest as typed tools whose output is ranked, budgeted, and verified before it
comes back. Underneath are ripgrep's regex engine, tree-sitter, ast-grep, and SQLite, wrapped in
one Rust crate and driven from a Bun process. Nothing about the engines is new. Everything about
what the agent sees is.

Four things follow from that, and the rest of this README is mostly about them:

- **Search ranks instead of dumping.** Definitions before uses, source before tests and vendor,
  grouped by file, trimmed to a token budget, never truncated silently.
- **Writes are verified in the same call.** An edit that would leave a file syntactically broken
  is rejected with the parser's message and the diff that was rolled back. Nothing half-applies.
- **There is no shell.** Project commands run through a declared allowlist. Ad-hoc batch work runs
  in a fuel-metered WASM sandbox. There is no escape hatch that is bash by another name.
- **It costs what grep costs.** No config file, no daemon, useful lexical answers within seconds
  of a cold start, and a crash in any subsystem degrades the answer instead of ending the server.

## The eight tools

| Tool | Replaces | What comes back |
|---|---|---|
| `search` | `grep`, `rg`, `find`, `ls`, glob | Ranked hits grouped by file; lexical, structural (ast-grep), references, or filename modes |
| `map` | `tree`, `ls -R`, the orientation grep in a new repo | A pruned directory tree with entry points, project commands, index health, and per-file symbol outlines |
| `read` | `cat`, `head`, `tail`, `sed -n` | A file, a line range, or a symbol by name, with line anchors and a content hash |
| `edit` | `sed -i`, `awk`, hand-written codemods | A verified diff: replace a body, insert around a symbol, rename, structural rewrite, or anchored text replacement |
| `files` | `mv`, `cp`, `touch`, `mkdir`, `rm` | Import-aware moves, journal-backed deletes, all undoable |
| `run` | running the tests, the build, the linter | Parsed failures and diagnostics as rows, from one declared command, no shell |
| `script` | a bash one-liner over many files | Typed Code Mode: reads execute live, writes queue into one reviewable diff |
| `undo` | `git checkout --` on your own last edit | The journal rewound one or more steps, with conflicts reported rather than forced |

Plus `workbench_status`, which takes no arguments and reports the workspace root, journal mode,
and supported envelope versions.

Every one of them returns the same envelope. That is covered in [Reading an envelope](#reading-an-envelope).

## Quick start

You need [Bun](https://bun.sh) 1.4 or newer and a Rust toolchain (the repo pins one in
`rust-toolchain.toml`) to build the native core for your platform. Prebuilt binaries are not
checked in yet.

```sh
git clone https://github.com/AlrikOlson/workbench-mcp.git
cd workbench-mcp
bun install
bun run build:core      # the native core; build:core:wasm builds the fallback instead
```

Then point your agent host at it. For Claude Code, that is an entry in `.mcp.json` in the
repository you want the agent to work on:

```json
{
  "mcpServers": {
    "workbench-mcp": {
      "command": "bun",
      "args": ["run", "/path/to/workbench-mcp/src/main.ts", "."]
    }
  }
}
```

The last argument is the workspace root. Everything the server does is scoped to it; paths that
resolve outside it are refused.

The first useful thing to do in a new workspace is let workbench seed the `run` allowlist from
whatever lockfile and manifest it finds:

```sh
bun run /path/to/workbench-mcp/src/main.ts init .
```

That writes a `[commands]` table into `workbench.toml`. From then on only a human edits that
table; an agent that asks for a command that is not in it gets `COMMAND_NOT_DECLARED` and a
pointer to you. The one in this repository looks like this:

```toml
[commands]
"test" = "bun test"
"typecheck" = "bun run typecheck"
"lint" = "bun run lint"
"clippy" = "cargo clippy --locked --workspace --all-targets -- -D warnings"
```

Finally, give the agent the routing skill. `skills/workbench-routing/SKILL.md` is a hand-written
document that maps shell habits to workbench tools, explains the envelope, and lists the next
action for every error code. For Claude Code, copy the `skills/workbench-routing/` directory into
`.claude/skills/`; for another harness, wherever that harness loads skills from. It is the
difference between an agent that has the tools and an agent that reaches for them.

If you want HTTP instead of stdio, `--http [port]` serves stateless Streamable HTTP on `/mcp`,
one replica per workspace. `workbench-mcp doctor` prints a health report, and `doctor bundle`
packs the same probes into a redacted tarball for a bug report (structure and symptoms, never
your file contents).

## The tools, one at a time

Each call below is complete and valid. The routing skill has a worked example per tool too, and a
CI check keeps those examples in agreement with the schemas.

### search

Four live modes, one envelope. `auto` runs lexical (regex over live file content, no index
needed) and re-ranks with structural classification once the index is warm, so it is the right
default. `lexical` pins it there. `structural` takes an ast-grep pattern with `$VAR` metavariables
and returns hits tagged with their node kind. `references` returns the callers, importers and
usages of a symbol from the index, each edge with a confidence. `files` is glob search in the
same shape.

```json
{ "query": "parseConfig", "mode": "references", "path": "src/**", "context": 2, "budget": 3000 }
```

Ranking demotes tests and vendored code rather than hiding them; `include: { tests: false }`
excludes them outright. `kind` filters structural hits by node kind, and the filter is a
substring, so `"function"` keeps both `function_item` and `function_signature_item` in Rust.

There is a `semantic` mode in the enum. It is rejected with a typed error. See
[What it will not do](#what-it-will-not-do).

### map

Orientation for a repository you have not seen. `skeleton` is the pruned directory tree with entry
points, declared project commands, index health and the plugin inventory; in a Cargo workspace it
adds one ranked row per crate. `outline` adds a compressed symbol outline per file once the index
is warm. `full` lifts the per-file symbol cap and lets the budget do the trimming.

```json
{ "path": ".", "depth": 2, "detail": "outline", "budget": 6000 }
```

### read

A whole file, a 1-based inclusive line range, or a symbol by address. Symbol reads resolve
through the index and accept either `UserService.authenticate` or `Config::load`; a bare member
name works when it is unambiguous. `surround` decides how much comes with it: `none` for exactly
the symbol, `enclosing` (the default) for its parent too, `imports` for the file's leading import
block as well.

```json
{ "file": "src/auth/service.ts", "symbol": "UserService.authenticate", "surround": "enclosing" }
```

The unit header carries a `contentHash`. Hand it to `edit` as `expectedHash` and a file that
changed under you comes back as `STALE_ANCHOR` instead of being clobbered.

### edit

Six operations, one pipeline. `replace_body`, `insert_after`, `insert_before` and `rename`
address code by symbol. `rewrite` is a repo-wide structural codemod: an ast-grep pattern, a
replacement pattern, a glob scope and a language. `replace_text` is an anchored literal
replacement for anything, source or not.

```json
{
  "op": "rewrite",
  "pattern": "console.log($ARGS)",
  "rewritePattern": "logger.debug($ARGS)",
  "path": "src/**",
  "lang": "ts",
  "dryRun": true
}
```

Every one of them runs the same way: take the workspace write lock, compute a dry-run diff,
pass the confirm gate, apply to a shadow copy, parse-verify it, format it, re-verify, journal
it, swap it into place atomically. A batch that would introduce a syntax error is rolled back
whole and returns `PARSE_REJECTED` with the parser's message and the rejected diff.

Single-file symbol and text edits apply on their own; parse-verify is the guard. Multi-file
operations (`rewrite`, `rename`, and `files` moves) and anything large stop at the confirm gate:
the envelope carries the aggregated diff and a `confirmToken`, and the agent re-issues the
identical call with `confirm` set to that token. If the host supports MCP elicitation, the server
can ask a human in-call instead, and `edit.confirm_mode` picks which. `dryRun: true` runs the
whole pipeline and applies nothing, which is worth doing liberally.

### files

`move`, `copy`, `create`, `mkdir`, `delete`. A move rewrites the files that import the moved
one and puts those rewrites in the same diff; in Rust it also updates the declaring `mod` item,
`crate::`/`super::`/`self::` paths, and Cargo target `path` entries, and the notes say what it
left for you. A delete is a journal-backed move into `.workbench/trash/`, so `undo` can bring it
back. Nothing is ever unlinked.

Binary files, including large PNG captures, stay byte-exact through copy, move, delete, and
undo after a server restart. They use the same locking, confirmation, journal, and atomic
write pipeline; parsing, formatting, and import rewriting apply only to text. Binary previews
and receipts contain byte counts and SHA-256 hashes, never the image payload. Text edits
refuse NUL-containing or invalid UTF-8 files instead of silently replacing their bytes.

```json
{ "op": "move", "from": "src/utils/date.ts", "to": "src/lib/date.ts", "dryRun": true }
```

### run

One declared command from the `[commands]` table, executed as a single argv. No shell is
involved, so shell operators in `args` are rejected rather than interpreted. What comes back is
rows, not text: a `report` row of test failures (name, file, assertion, trimmed trace) parsed
from bun, jest, vitest, pytest, cargo test, cargo nextest or go test; a `diagnostics` row of
rustc/clippy diagnostics with `file:line:column`, errors first; or an `output` row of head and
tail with a truncation manifest when nothing claimed the output.

```json
{ "command": "test", "args": ["--filter", "auth"], "timeout": 300 }
```

Asking `run` to do text or file munging answers `RUN_REDIRECTED` and points at the tool that
owns that job.

### script

The long tail of what bash was actually for: ad-hoc work over many files. The program runs in a
QuickJS guest inside a WASM sandbox, fuel-metered and memory-capped, with no ambient filesystem
and no network. It sees one global, `workspace`, with `search`, `read`, `glob`, `edit.queue` and
`files.queue`. Reads execute live. Writes are queued and come back as one dry-run diff behind
the same confirm gate every other write goes through.

```json
{
  "source": "const hits = workspace.search({ query: 'TODO', mode: 'lexical', path: 'src/**' });\nreturn hits.results.map((g) => g.file);",
  "dryRun": true
}
```

### undo

Rewinds the journal. `scope: "session"` only touches operations from your own session (set
`workbench/sessionId` in `_meta` so the server knows which those are); `scope: "workspace"`
crosses sessions. If someone changed a file since the operation you are undoing, `onConflict`
decides: `surface` reports it and stops, `force` restores the journaled content, `keepCurrent`
keeps their change and undoes the rest.

```json
{ "steps": 1, "scope": "session", "onConflict": "surface" }
```

## Reading an envelope

Every tool, including every plugin tool, returns one shape. It doubles as the MCP `outputSchema`.

On success:

- `results` — always ranked and grouped by file.
- `freshness` — `cold`, `warming` or `warm`: how warm the index was when this was answered.
- `budget` — `requested` and `used`, in estimated tokens. The default budget is 4,000.
- `notes` — degradations, progress, fidelity caveats. Worth reading.
- `truncation` — present only if results were cut, and then it always carries all four of
  `dropped`, `reason`, `narrow` (typed hints like `kind:function` or `path:src/**`) and
  `cursor` (to continue where it stopped).

On failure:

- `error.code` — one of the published codes.
- `error.recovery` — exactly one of `retry`, `narrow`, `confirm`, `escalate`, `abort`. Each code
  maps to exactly one recovery class, and that mapping is a schema test, not a convention.
- `error.message`, plus code-specific fields: `candidates` for an ambiguous symbol,
  `parserMessage` and `rejectedDiff` for a rejected write, `contentHash` for a stale anchor,
  `confirmToken` for a gated write, `backoffMs` for a busy engine.

Budgets are parameters, not limits to fight. Need more, raise `budget`; need less noise, narrow
`path` or `kind`. Because a cut is never silent, an agent never has to wonder whether it saw
everything.

The envelope carries `v`, its format version. Evolution within a major version is additive only
(new optional fields), and a test compares the live schema against a frozen v1 baseline to hold
that line. A client can pin what it accepts with `workbench/maxEnvelopeVersion` in `_meta`.

## How it works

### Two processes, one binary

`workbench-mcp` is the server: it speaks MCP, resolves config, holds the journal, and owns the
security boundary. `workbench-mcp --engine` is a supervised child of the same binary that loads
the native core and does every parse, structural match and index write. Same binary means
version skew between them is impossible. Separate process means a parser crash on a hostile
file costs one degraded answer, not the server. A file that repeatedly crashes the parser is
quarantined and reported by `doctor`; the rest of the workspace keeps working.

The core is one Rust crate wrapping tree-sitter, the `grep` crate (ripgrep's engine as a
library) and ast-grep, exposed through napi-rs. It is loaded native first; if that fails, the
engine child falls back to a `wasm32-wasip1` build of the same crate at runtime, and a startup
self-test exercises parse, search and rewrite before the server reports ready. The child is
spawned lazily on the first call that needs it.

### Freshness tiers

Tier 0 is the filesystem: lexical search and reads work the moment the server starts, with no
index. In the background the first workspace walk kicks off a Tier 1 build into
`.workbench/index.db`, a SQLite database with FTS5 for content and tables for files, symbols,
imports and references. A file watcher (native, or polling on a detected network filesystem)
keeps it current with debounced incremental updates. The envelope's `freshness` field says which
tier answered.

Only `references` and symbol-addressed `read`/`edit` actually need the warm index; until then
they answer `INDEX_UNAVAILABLE` with `recovery: retry` and build progress in `notes`. Nothing
blocks waiting for it, and nothing guesses. Whatever the tier, no line number leaves the server
without being checked against the file's current mtime and size.

The index is cache-class data. If a newer binary changes its schema, the old index is discarded
and rebuilt rather than misread. The journal and config are durable-class: they migrate forward
on open, are quarantined intact rather than deleted if unreadable, and fail closed if opened by
an older binary.

### The write path

There is exactly one way for bytes to reach a file. `edit`, `files`, `script` commits, `undo`,
plugin tools and the CLI all pass through the same write engine:

```
workspace lock → dry-run diff → confirm gate → shadow apply → parse-verify
             → format → re-verify → journal → atomic swap
```

Source files are parse-verified with tree-sitter; a new syntax error rolls the whole batch back.
Non-source files skip the parser but still get well-formedness checks, the journal and the
confirm gate. Formatting runs Biome or rustfmt when the project uses them, and can be turned off.
The journal is what makes `undo` possible and what makes a delete a move.

Text diff search is capped at 8 MiB of trace storage and one million search steps. If either
limit is reached, the receipt explicitly reports the fallback and shows a complete replacement
hunk in linear work. Binary changes use a compact summary without entering text diff search.
Journal schema v2 preserves existing v1 text history and prevents older readers from decoding
new binary history as UTF-8.

### The security boundary

Every path is resolved and checked against the workspace root; symlinks that escape it are
refused. Secrets are redacted at the envelope boundary before anything leaves the server.
Confirm tokens are minted per call with a per-workspace key (`workbench-mcp key rotate` replaces
it). The `[commands]` table is a protected path that no tool may write, which is what stops an
agent from adding a command to its own allowlist.

The server and the engine child never open an outbound network connection. Plugin installation
and crash-report upload live in the CLI and run only when a person runs them. Tracing writes one
line per request to `.workbench/trace.jsonl` on disk and never anywhere else; telemetry consent
defaults to `off`.

## Configuration

Workbench runs with no config file at all. When you want to change something, the file is
`workbench.toml` in the workspace root, and the keys are few:

| Key | Default | What it does |
|---|---|---|
| `[commands]` | seeded by `init` | The `run` allowlist. Human-edited only. |
| `budgets.default` | `4000` | Token budget when a call does not name one. |
| `edit.confirm_mode` | `"elicit"` | `elicit` asks a human in-call when the host supports it; `token` always uses the two-call `confirmToken` flow. |
| `edit.force_confirm` | `false` | Gate every write, including single-file edits. |
| `format.enabled` | `true` | Run the project's formatter after a verified write. |
| `security.redaction` | `true` | Redact secrets at the envelope boundary. |
| `security.protected_paths` | `[]` | Extra paths to declare off-limits to writes. Additive across layers. |
| `security.allow_sideload` | `true` | Allow plugins installed from outside the registry. |
| `telemetry.consent` | `"off"` | `off`, `crash`, or `full`. Stays on disk regardless. |
| `tracing.enabled` | `true` | Write the per-request flight recorder. |
| `watcher.mode` | `"auto"` | `auto`, `native`, `poll`, or `off`. |

Config resolves through five layers, lowest precedence first: built-in defaults, an org policy
file, a user file, the workspace `workbench.toml`, and per-session `--config key=value` flags.
Ordinary keys merge nearest-wins. Security keys only tighten: once a layer above the defaults
has decided one, a later layer can move it toward the safer value but never away, and an attempt
to loosen it is recorded as `CONFIG_POLICY_DENIED` naming the layer that pinned it. That is how
an organization can pin redaction on, or pin the `[commands]` table across a team, without
trusting every workspace file.

Unknown keys and unreadable layer files warn and are skipped; they never stop the server from
starting. `workbench-mcp doctor config` prints the resolved value of every key with the layer
that decided it, and takes no lock, so it runs beside a live server.

## Extending it

### Plugins

Third-party code runs inside a Wasmtime component-model sandbox, default-deny, with fuel and
memory limits from its manifest. There are five plugin surfaces, each a versioned WIT package
under `wit/`:

| Interface | Adds |
|---|---|
| `workbench:lang-pack` | A grammar, tag queries and file-class heuristics for a new language |
| `workbench:resolver` | Name-resolved references, rename and diagnostics — the upgrade from tree-sitter fidelity to LSP fidelity |
| `workbench:runner` | A structured-failure parser for a new test or build tool's output |
| `workbench:tool` | New MCP tools, under a mandatory `<publisher>.<pkg>/<tool>` namespace |
| `workbench:ranking` | Feature contributions to the search scorer, run in shadow mode until promoted with eval numbers |

A plugin is a directory with a `plugin.toml` and a component. The manifest names the publisher,
the package, the tools it exposes, the host imports it needs, and its limits:

```toml
[plugin]
publisher = "workbench"
pkg = "lsp-resolver"
version = "0.1.0"

[component]
path = "guest.wasm"

[[tools]]
name = "status"
description = "Reports the resolver plugin's build and protocol identity"

[capabilities]
imports = ["workbench:subprocess/subprocess@0.2.0"]

[limits]
fuel = 500000000
memory_bytes = 67108864
```

The first-party features are built on this same SDK rather than on private hooks: the LSP
resolver, a Gleam language pack, a Go test runner and a visibility ranking signal all live under
`plugins/` as ordinary plugins that go through the same admission checks and the same
conformance kit (`test/plugin-tck.test.ts`) a third-party plugin would. `examples/plugins/` has
the smallest conforming plugins, kept as readable WAT text. Plugin tools cannot shadow, wrap or
rename the eight core tools, and their writes queue into the same write engine.

Each WIT package carries its own semver stamp and evolves additively within a major. The
reasoning is that a public interface mistake is carried for years, so evolution is mechanical
rather than a judgment call.

### The routing skill

`skills/workbench-routing/SKILL.md` is the other half of the product. It is written by hand and
kept honest by `bun run lint:skill`, which fails CI whenever its routing table, its recovery table
or its worked examples disagree with the generated tool schemas or the error taxonomy in either
direction. The prose is opinion; the tables are contract. When the flight recorder shows a misuse
pattern (the same error five times in a row, a run of `RUN_REDIRECTED`), the fix is to revise the
row that should have prevented it.

## What it will not do

These are design decisions, not gaps waiting to be filled. Each one has a named trigger that
would reopen it, and the trigger is data rather than preference.

- **No general shell execution.** `run` executes declared commands only; `script` is a typed
  sandbox. There is no tool, at any trust tier, that takes a command string and hands it to `sh`.
- **No embeddings or vector search in v1.** Lexical, structural and references cover the
  load-bearing cases without the staleness and false-negative problems of a vector index. The
  `semantic` mode is reserved in the schema and rejected with a typed error so the API does not
  break when it lands. Reopens if more than 20% of zero-hit searches in dogfood traces are judged
  semantically answerable.
- **No graph database.** SQLite only. Reopens if the references latency gate is missed.
- **No LSP requirement.** Tree-sitter is the spine. Language servers are an optional, lazy,
  background upgrade delivered as a plugin; if one never becomes healthy, every tool still works
  at tree-sitter fidelity.
- **No mutation of the core tool surface.** Plugins add tools under their own namespace and
  nothing else.
- **No cloud, no code egress, no server-side network.** Indexing and search happen wherever the
  server runs. Nothing about your code leaves the machine unless you run a CLI command that says
  it will.
- **No multi-replica fleet in v1.** The HTTP profile is one replica per workspace.
- **No deprecated MCP surface.** No Sampling, Roots, Logging or HTTP+SSE. Long-running work uses
  the Tasks extension; in-call interaction uses elicitation.

## Developing

The project is Bun and TypeScript on top, one Rust crate underneath, with schemas as the source
of truth in between. Every MCP `inputSchema` and `outputSchema` is generated from `schemas/`;
nothing hand-writes a tool schema, and adding a tool means adding one JSON file there and
re-running codegen.

| Command | What it checks |
|---|---|
| `bun test` | The TypeScript suite: conformance on both transports, statelessness, the freshness fuzzer, migrations, the jail red-team, the plugin TCK, and per-tool behaviour |
| `bun run test:core` | The Rust crate's own tests |
| `bun run typecheck` | `tsc --noEmit` |
| `bun run lint` | Biome |
| `bun run lint:imports` | The port-layer import graph: tools may only reach engines through ports |
| `bun run lint:skill` | The routing skill agrees with the schemas and the error taxonomy |
| `bun run codegen:check` | `src/generated/` matches `schemas/` |
| `bun run bench:ipc` / `bench:index` / `bench:refs` | The recorded perf gates; `--root <path>` points a bench at a real repository |
| `bun run dogfood` | Drives the read path against this repository through the same `.mcp.json` an agent would use |
| `bun run eval:rank` | MRR and Hit@k for the ranking scorer; no ranking change merges without a before/after |

Where things live:

```
src/protocol/    MCP transport, server, Tasks, elicitation, tracing
src/tools/       the eight tools, the write engine, ranking, the plugin host
src/ports/       the interfaces tools are allowed to depend on
src/adapters/    engine IPC, SQLite journal, file watcher, formatters, workspace lock
src/jail/        path resolution and the security boundary
src/config/      the five-layer resolver and the key registry
src/doctor/      health probes, the report, the redacted bundle
crates/          workbench-core: tree-sitter, grep, ast-grep, the sandbox host
schemas/         the envelope, the error taxonomy, one input schema per tool
wit/             the plugin interfaces
plugins/         first-party plugins built on the public SDK
skills/          the routing skill
specs/           one Spec Kit feature per shipped chunk: spec, plan, research, tasks
bench/           recorded perf-gate runs, failures included
```

Work is spec-first. Each feature under `specs/` was shaped before it was built, and the
constitution in `.specify/memory/constitution.md` is what the gates enforce. If you want the
full reasoning behind any of the decisions above, `workbench-mcp-sdd.md` is the design document
and has a section number for every claim in this README.
